//! Machines rented in the cloud, set up by the script a provider runs when it starts one (cloud-init's user
//! data, which every large provider takes: AWS, Google Cloud, Azure, DigitalOcean, Hetzner, OVH...):
//!
//! - **a server:** Terraflux installed (the Linux package of this version, in `/opt/terraflux`, the command in
//!   `/usr/local/bin`); then saved as a server reached by SSH ([`crate::remote`]: `terraflux cluster add`, or the
//!   application's Computers pane), its coordinator and workers started for each batch;
//! - **more machines working for it:** Terraflux installed and a worker kept running as a service (started
//!   again if it stops, and with the machine), reaching the server's coordinator by its address on the provider's
//!   network and its code.

/// The Linux package of a version of Terraflux, as the releases carry it (`tools/make-linux-package.py`).
pub fn package_url(version: &str) -> String {
    format!("https://github.com/sacridini/terraflux/releases/download/v{version}/terraflux-{version}-linux-x64.tar.gz")
}

/// What a machine is for.
#[derive(Debug, Clone, PartialEq)]
pub enum Role {
    /// Terraflux installed, nothing started (a server batches are run on by SSH).
    Server,
    /// A worker kept running for a coordinator (`host:port`) of a code.
    Worker { coordinator: String, code: String, threads: Option<usize> },
}

/// The cloud-init user data of a machine: Terraflux from the package at `url`, and what the role asks.
pub fn user_data(role: &Role, url: &str) -> String {
    let mut s = String::from(
        "#cloud-config\n# Terraflux: paste as the machine's \"user data\" (cloud-init) when it is created.\npackage_update: true\n\
         packages: [curl, ca-certificates]\n",
    );
    if let Role::Worker { coordinator, code, threads } = role {
        let threads = threads.map(|t| format!(" --threads {t}")).unwrap_or_default();
        s += &format!(
            "write_files:\n  - path: /etc/systemd/system/terraflux-worker.service\n    content: |\n      [Unit]\n      \
             Description=Terraflux worker\n      After=network-online.target\n      Wants=network-online.target\n      \
             [Service]\n      ExecStart=/usr/local/bin/terraflux worker --coordinator {coordinator} --code {code} --cache \
             /var/cache/terraflux --name %H{threads}\n      Restart=always\n      RestartSec=10\n      Nice=5\n      \
             [Install]\n      WantedBy=multi-user.target\n"
        );
    }
    s += &format!(
        "runcmd:\n  - mkdir -p /opt\n  - curl -fsSL {url} -o /tmp/terraflux.tar.gz\n  - tar -xzf /tmp/terraflux.tar.gz -C /opt\n  \
         - rm -rf /opt/terraflux && mv /opt/terraflux-*-linux-* /opt/terraflux\n  - ln -sf /opt/terraflux/terraflux \
         /usr/local/bin/terraflux\n"
    );
    if matches!(role, Role::Worker { .. }) {
        s += "  - mkdir -p /var/cache/terraflux\n  - systemctl daemon-reload\n  - systemctl enable --now terraflux-worker\n";
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn servers_and_workers_are_set_up() {
        let url = package_url("1.2.3");
        assert!(url.ends_with("/v1.2.3/terraflux-1.2.3-linux-x64.tar.gz"));
        let server = user_data(&Role::Server, &url);
        assert!(server.starts_with("#cloud-config\n"));
        assert!(server.contains(&url) && server.contains("ln -sf /opt/terraflux/terraflux /usr/local/bin/terraflux"));
        assert!(!server.contains("terraflux-worker"));
        let worker = user_data(
            &Role::Worker { coordinator: "10.0.0.5:23456".into(), code: "K7QM-2XPA-9DTE".into(), threads: Some(8) },
            &url,
        );
        assert!(worker.contains("worker --coordinator 10.0.0.5:23456 --code K7QM-2XPA-9DTE"));
        assert!(worker.contains("--threads 8") && worker.contains("systemctl enable --now terraflux-worker"));
        // YAML: every line of the unit indented under its content
        let unit: Vec<&str> = worker.lines().skip_while(|l| !l.contains("content: |")).skip(1).take(11).collect();
        assert!(unit.iter().all(|l| l.starts_with("      ")), "{unit:?}");
    }
}
