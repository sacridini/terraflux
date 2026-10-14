//! Connections to a coordinator: plain (a coordinator of this computer, or a network one trusts), or with a
//! pairing code (TLS, the coordinator's proof checked and its certificate pinned, every call with the token: see
//! [`crate::secure`]); and the TLS connections a coordinator with a code accepts.

use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use hyper_util::rt::TokioIo;
use rustls::pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
use tokio_rustls::{TlsAcceptor, TlsConnector};
use tonic::metadata::{Ascii, MetadataValue};
use tonic::service::Interceptor;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::{Channel, Endpoint};
use tonic::{Request, Status};

use crate::coordinator::MAX_MESSAGE;
use crate::proto::Empty;
use crate::proto::coordinator_client::CoordinatorClient;
use crate::secure::{Code, Pinned, same};
use crate::{Error, Result};

/// The token of the calls, if any.
#[derive(Clone, Default)]
pub struct Auth(Option<MetadataValue<Ascii>>);

impl Interceptor for Auth {
    fn call(&mut self, mut r: Request<()>) -> std::result::Result<Request<()>, Status> {
        if let Some(t) = &self.0 {
            r.metadata_mut().insert("authorization", t.clone());
        }
        Ok(r)
    }
}

/// A connection to a coordinator.
pub type Conn = CoordinatorClient<InterceptedService<Channel, Auth>>;

/// `host:port` of an address given with or without its scheme.
pub fn host_port(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    rest.trim_end_matches('/').to_string()
}

fn client(channel: Channel, auth: Auth) -> Conn {
    CoordinatorClient::with_interceptor(channel, auth)
        .max_decoding_message_size(MAX_MESSAGE)
        .max_encoding_message_size(MAX_MESSAGE)
}

/// Connect to a coordinator at `url` (`http://host:port`, or `host:port`): plain, or with its pairing code.
pub async fn connect(url: &str, code: Option<&Code>) -> Result<Conn> {
    let at = host_port(url);
    let Some(code) = code else {
        let channel = Endpoint::from_shared(format!("http://{at}"))
            .map_err(|e| Error::Other(format!("{url}: {e}")))?
            .connect()
            .await?;
        return Ok(client(channel, Auth::default()));
    };
    let pinned = Pinned::new();
    let tls = TlsConnector::from(Arc::new(pinned.client_config()?));
    let target = at.clone();
    let connector = tower::service_fn(move |_: tonic::codegen::http::Uri| {
        let (tls, target) = (tls.clone(), target.clone());
        async move {
            let tcp = TcpStream::connect(&target).await?;
            tcp.set_nodelay(true)?;
            let name = ServerName::try_from("terraflux").expect("a name");
            let s = tls.connect(name, tcp).await?;
            Ok::<_, std::io::Error>(TokioIo::new(s))
        }
    });
    let channel = Endpoint::from_shared(format!("http://{at}"))
        .map_err(|e| Error::Other(format!("{url}: {e}")))?
        .connect_with_connector(connector)
        .await?;
    // the coordinator proves it knows the code before the token goes
    let hello = client(channel.clone(), Auth::default()).hello(Empty {}).await?.into_inner();
    let seen = pinned.seen().ok_or_else(|| Error::Other("no certificate".into()))?;
    if !same(&hello.proof, &code.proof(&seen)) {
        return Err(Error::Other(format!("the coordinator at {at} does not know this code")));
    }
    pinned.pin(seen);
    let token: MetadataValue<Ascii> =
        format!("Bearer {}", code.token()).parse().map_err(|_| Error::Other("a token".into()))?;
    Ok(client(channel, Auth(Some(token))))
}

/// A TLS connection a coordinator accepted.
pub struct Tls(pub tokio_rustls::server::TlsStream<TcpStream>);

impl tonic::transport::server::Connected for Tls {
    type ConnectInfo = ();
    fn connect_info(&self) -> Self::ConnectInfo {}
}

impl AsyncRead for Tls {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl AsyncWrite for Tls {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}

/// The TLS connections to a listener, each handshake on its own (a slow one holds no other back).
pub fn accept(
    listener: tokio::net::TcpListener,
    config: rustls::ServerConfig,
) -> tokio_stream::wrappers::ReceiverStream<std::io::Result<Tls>> {
    let acceptor = TlsAcceptor::from(Arc::new(config));
    let (tx, rx) = tokio::sync::mpsc::channel(16);
    tokio::spawn(async move {
        loop {
            let Ok((tcp, _)) = listener.accept().await else { continue };
            let _ = tcp.set_nodelay(true);
            if tx.is_closed() {
                return;
            }
            let (acceptor, tx) = (acceptor.clone(), tx.clone());
            tokio::spawn(async move {
                let handshake = tokio::time::timeout(std::time::Duration::from_secs(10), acceptor.accept(tcp));
                if let Ok(Ok(s)) = handshake.await {
                    let _ = tx.send(Ok(Tls(s))).await;
                }
            });
        }
    });
    tokio_stream::wrappers::ReceiverStream::new(rx)
}
