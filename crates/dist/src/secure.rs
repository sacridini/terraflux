//! A coordinator other computers reach, safely, from a short code: whoever has the code may submit batches
//! and work, nobody else; nobody on the way reads or changes what travels.
//!
//! - **The code** (`K7QM-2XPA-9DTE`: 12 characters of Crockford's base 32, 60 bits) is shown by the computer
//!   that shares and typed on those that help. A key is made from it slowly (PBKDF2-HMAC-SHA256, 100 000
//!   rounds), so that what goes on the network does not give the code away.
//! - **TLS** with a certificate the coordinator makes for itself (kept in its folder). A client accepts any
//!   certificate at first, then asks the coordinator to prove it knows the code: an HMAC of the certificate it
//!   presented, made with the key. A coordinator that does not know the code, or someone in between with a
//!   certificate of their own, cannot. The certificate is then pinned: the connection made again must present
//!   it.
//! - **Authentication:** once the coordinator proved itself, every call carries a token made from the key.

use std::fmt;
use std::sync::{Arc, Mutex};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, WebPkiSupportedAlgorithms};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use sha2::{Digest, Sha256};

use crate::{Error, Result};

/// The characters of a code: Crockford's base 32 (no I, L, O or U).
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const LENGTH: usize = 12;

/// HMAC-SHA256.
pub fn hmac(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        k[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let pad = |b: u8| k.iter().map(|x| x ^ b).collect::<Vec<u8>>();
    let inner = Sha256::new().chain_update(pad(0x36)).chain_update(message).finalize();
    Sha256::new().chain_update(pad(0x5c)).chain_update(inner).finalize().into()
}

/// PBKDF2-HMAC-SHA256, one block (the HMAC's padded keys hashed once).
fn pbkdf2(password: &[u8], salt: &[u8], rounds: u32) -> [u8; 32] {
    let mut k = [0u8; 64];
    if password.len() > 64 {
        k[..32].copy_from_slice(&Sha256::digest(password));
    } else {
        k[..password.len()].copy_from_slice(password);
    }
    let inner = Sha256::new().chain_update(k.map(|x| x ^ 0x36));
    let outer = Sha256::new().chain_update(k.map(|x| x ^ 0x5c));
    let mac = |m: &[u8]| -> [u8; 32] {
        let i = inner.clone().chain_update(m).finalize();
        outer.clone().chain_update(i).finalize().into()
    };
    let mut s = salt.to_vec();
    s.extend_from_slice(&1u32.to_be_bytes());
    let mut u = mac(&s);
    let mut out = u;
    for _ in 1..rounds {
        u = mac(&u);
        for (o, x) in out.iter_mut().zip(&u) {
            *o ^= x;
        }
    }
    out
}

/// Bytes of the system's random generator.
fn random<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    provider().secure_random.fill(&mut b).expect("the system's random generator");
    b
}

/// A secret of 16 random bytes, in hexadecimal.
pub fn secret() -> String {
    hex::encode(random::<16>())
}

/// Two byte strings the same, in a time that does not tell where they differ.
pub fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

/// A pairing code, and the key made from it.
#[derive(Clone, PartialEq, Eq)]
pub struct Code {
    chars: String,
    key: [u8; 32],
}

impl fmt::Debug for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Code({self})")
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c = &self.chars;
        write!(f, "{}-{}-{}", &c[..4], &c[4..8], &c[8..])
    }
}

impl Code {
    /// A new code, at random.
    pub fn generate() -> Code {
        let r: [u8; LENGTH] = random();
        let chars: String = r.iter().map(|b| ALPHABET[(b % 32) as usize] as char).collect();
        Code::from_chars(chars)
    }

    /// A code as typed: any case, with or without dashes and spaces; O read as 0, I and L as 1.
    pub fn parse(text: &str) -> Result<Code> {
        let chars: String = text
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '-')
            .map(|c| match c.to_ascii_uppercase() {
                'O' => '0',
                'I' | 'L' => '1',
                c => c,
            })
            .collect();
        if chars.len() != LENGTH || !chars.bytes().all(|b| ALPHABET.contains(&b)) {
            return Err(Error::Other(format!("{text:?} is not a code (12 letters and digits, as K7QM-2XPA-9DTE)")));
        }
        Ok(Code::from_chars(chars))
    }

    fn from_chars(chars: String) -> Code {
        let key = pbkdf2(chars.as_bytes(), b"terraflux pairing code", 100_000);
        Code { chars, key }
    }

    /// The token every call carries.
    pub fn token(&self) -> String {
        hex::encode(hmac(&self.key, b"terraflux token"))
    }

    /// The coordinator's proof that it knows the code, for its certificate.
    pub fn proof(&self, fingerprint: &[u8]) -> Vec<u8> {
        let mut m = b"terraflux coordinator ".to_vec();
        m.extend_from_slice(fingerprint);
        hmac(&self.key, &m).to_vec()
    }

    /// What a coordinator of this code says on the network to be found (it does not give the code away).
    pub fn beacon(&self) -> String {
        hex::encode(&hmac(&self.key, b"terraflux beacon")[..12])
    }
}

/// A coordinator's TLS certificate and key, made once and kept in its folder.
pub struct Identity {
    pub cert: Vec<u8>,
    key: Vec<u8>,
}

impl Identity {
    pub fn load_or_create(dir: &std::path::Path) -> Result<Identity> {
        let (cp, kp) = (dir.join("tls-cert.der"), dir.join("tls-key.der"));
        if let (Ok(cert), Ok(key)) = (std::fs::read(&cp), std::fs::read(&kp)) {
            return Ok(Identity { cert, key });
        }
        let made = rcgen::generate_simple_self_signed(vec!["terraflux".to_string()])
            .map_err(|e| Error::Other(format!("a certificate: {e}")))?;
        let id = Identity { cert: made.cert.der().to_vec(), key: made.key_pair.serialize_der() };
        std::fs::create_dir_all(dir)?;
        std::fs::write(&kp, &id.key)?;
        std::fs::write(&cp, &id.cert)?;
        Ok(id)
    }

    pub fn fingerprint(&self) -> [u8; 32] {
        Sha256::digest(&self.cert).into()
    }

    /// The server's TLS settings (HTTP/2).
    pub fn server_config(&self) -> Result<rustls::ServerConfig> {
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.key.clone()));
        let mut c = rustls::ServerConfig::builder_with_provider(provider())
            .with_safe_default_protocol_versions()
            .map_err(|e| Error::Other(e.to_string()))?
            .with_no_client_auth()
            .with_single_cert(vec![CertificateDer::from(self.cert.clone())], key)
            .map_err(|e| Error::Other(e.to_string()))?;
        c.alpn_protocols = vec![b"h2".to_vec()];
        Ok(c)
    }
}

fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// The client's check of the coordinator's certificate: any at first (the coordinator then proves it knows the
/// code for it), then only the one pinned.
#[derive(Debug)]
pub struct Pinned {
    pin: Mutex<Option<[u8; 32]>>,
    seen: Mutex<Option<[u8; 32]>>,
    algorithms: WebPkiSupportedAlgorithms,
}

impl Pinned {
    pub fn new() -> Arc<Pinned> {
        Arc::new(Pinned {
            pin: Mutex::new(None),
            seen: Mutex::new(None),
            algorithms: provider().signature_verification_algorithms,
        })
    }

    /// The fingerprint of the certificate of the last connection.
    pub fn seen(&self) -> Option<[u8; 32]> {
        *self.seen.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn pin(&self, fingerprint: [u8; 32]) {
        *self.pin.lock().unwrap_or_else(|e| e.into_inner()) = Some(fingerprint);
    }

    /// The client's TLS settings (HTTP/2) with this check.
    pub fn client_config(self: &Arc<Self>) -> Result<rustls::ClientConfig> {
        let mut c = rustls::ClientConfig::builder_with_provider(provider())
            .with_safe_default_protocol_versions()
            .map_err(|e| Error::Other(e.to_string()))?
            .dangerous()
            .with_custom_certificate_verifier(self.clone())
            .with_no_client_auth();
        c.alpn_protocols = vec![b"h2".to_vec()];
        Ok(c)
    }
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        let fp: [u8; 32] = Sha256::digest(end_entity.as_ref()).into();
        *self.seen.lock().unwrap_or_else(|e| e.into_inner()) = Some(fp);
        match *self.pin.lock().unwrap_or_else(|e| e.into_inner()) {
            Some(p) if p != fp => Err(rustls::Error::General("another coordinator than the one paired".into())),
            _ => Ok(ServerCertVerified::assertion()),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algorithms.supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_and_pbkdf2_are_the_standards() {
        // RFC 4231, test case 2
        assert_eq!(
            hex::encode(hmac(b"Jefe", b"what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        // RFC 7914, PBKDF2-HMAC-SHA256 ("passwd", "salt", 1 round), its first 32 bytes
        assert_eq!(
            hex::encode(pbkdf2(b"passwd", b"salt", 1)),
            "55ac046e56e3089fec1691c22544b605f94185216dde0465e68b9d57c20dacbc"
        );
    }

    #[test]
    fn codes_are_typed_as_people_type() {
        let c = Code::generate();
        let shown = c.to_string();
        assert_eq!(shown.len(), 14);
        assert_eq!(Code::parse(&shown).unwrap(), c);
        assert_eq!(Code::parse(&shown.to_lowercase().replace('-', " ")).unwrap(), c);
        let a = Code::parse("K7QM-2XPA-9DTE").unwrap();
        assert_eq!(Code::parse("k7qm2xpa9dte").unwrap(), a);
        assert_eq!(Code::parse("K7QM-2XPA-9DTE").unwrap().token(), a.token());
        assert_eq!(Code::parse("10QM-2XPA-9DTE").unwrap(), Code::parse("ioqm-2xpa-9dte").unwrap());
        assert!(Code::parse("K7QM-2XPA").is_err());
        assert!(Code::parse("K7QM-2XPA-9DTU").is_err(), "no U");
        assert_ne!(Code::generate(), Code::generate());
        assert_ne!(a.token(), Code::parse("K7QM-2XPA-9DTF").unwrap().token());
        assert!(same(&a.proof(b"x"), &a.proof(b"x")) && !same(&a.proof(b"x"), &a.proof(b"y")));
    }
}
