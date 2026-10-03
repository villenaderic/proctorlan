//! Optional mDNS advertisement (`_proctorlan._tcp.local.`). Convenience only: manual IP:port
//! entry always works. The advertisement carries the app name and version, never a session code.

use std::net::Ipv4Addr;

use mdns_sd::{ServiceDaemon, ServiceInfo};

use crate::config;

pub struct Advertiser {
    daemon: ServiceDaemon,
}

impl Advertiser {
    /// Returns `None` (and logs) if the platform refuses multicast; callers carry on without it.
    pub fn start(ip: Ipv4Addr, port: u16) -> Option<Self> {
        if ip.is_loopback() {
            return None; // nothing on the LAN could reach it anyway
        }
        let result = (|| -> Result<ServiceDaemon, mdns_sd::Error> {
            let daemon = ServiceDaemon::new()?;
            let host = format!("{}.local.", config::APP_NAME.to_lowercase());
            let props = [("app", config::APP_NAME), ("version", env!("CARGO_PKG_VERSION"))];
            let info = ServiceInfo::new(config::MDNS_SERVICE_TYPE, config::APP_NAME, &host, ip.to_string().as_str(), port, &props[..])?;
            daemon.register(info)?;
            Ok(daemon)
        })();
        match result {
            Ok(daemon) => Some(Self { daemon }),
            Err(e) => {
                tracing::warn!(error = %e, "mDNS advertisement unavailable; students can still join by IP and port");
                None
            }
        }
    }
}

impl Drop for Advertiser {
    fn drop(&mut self) {
        let _ = self.daemon.shutdown();
    }
}
