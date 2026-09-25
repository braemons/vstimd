//! Telling the network this rig has a display: `_vstimd._tcp`.
//!
//! Advertised by the daemon itself, as statemachined, mousewheeld and triald
//! advertise theirs, so the record exists exactly while the server does and
//! says the ports it actually bound. It replaces a static Avahi service file
//! rendered at boot: that one had to be edited by hand whenever a rig config
//! moved a port, and it went on advertising a server that had stopped.
//!
//! The SRV port is the web port, where a braemons console loads
//! `/elements/vstimd.js` — the same meaning the SRV port has for every daemon
//! in the family. The ZMQ ports are TXT records (`zmq_port`, `event_port`), as
//! triald puts its gRPC port in `grpc_port`. With the web surface off, the SRV
//! port is the ZMQ command port and there is no `elements`.
//!
//! Two identifiers, for two questions:
//!
//! - **`id`** — this daemon on this box, `sha256("vstimd:" + machine-id)`.
//! - **`rig`** — the box, `sha256("braemons:" + machine-id)`, the *same* string
//!   in every braemons daemon's record on it, which is what lets a console show
//!   the display, the wheel and the state machine as one rig.
//!
//! **Advertising is never fatal.** A server whose network is down still drives
//! a display that somebody may reach by IP.

use std::collections::HashMap;
use std::path::Path;

use sha2::{Digest, Sha256};

pub const SERVICE_TYPE: &str = "_vstimd._tcp.local.";
pub const MACHINE_ID_PATH: &str = "/etc/machine-id";

/// The ports this server bound, as the record reports them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdvertisedPorts {
    pub zmq_port: u16,
    /// `None` with `--no-events`.
    pub event_port: Option<u16>,
    /// `None` with the web surface off.
    pub web_port: Option<u16>,
}

impl AdvertisedPorts {
    /// The SRV port: the panels where there are panels, else the commands.
    pub fn service_port(&self) -> u16 {
        self.web_port.unwrap_or(self.zmq_port)
    }
}

/// Sixteen hex digits of `sha256(salt + machine-id)`.
///
/// Falls back to the hostname where there is no machine-id — a container, a
/// developer's checkout: weaker, and better than a value that changes every
/// restart.
pub fn salted_machine_identifier(salt: &str, machine_id_path: &Path) -> String {
    let seed = std::fs::read_to_string(machine_id_path)
        .map(|text| text.trim().to_string())
        .unwrap_or_default();
    let seed = if seed.is_empty() { hostname() } else { seed };
    let digest = Sha256::digest(format!("{salt}{seed}").as_bytes());
    digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()[..16]
        .to_string()
}

/// What a console can act on without opening a connection first.
pub fn text_records_for(
    daemon_identifier: &str,
    rig_identifier: &str,
    ports: AdvertisedPorts,
    version: &str,
) -> HashMap<String, String> {
    let mut records: HashMap<String, String> = [
        ("id", daemon_identifier.to_string()),
        ("rig", rig_identifier.to_string()),
        ("version", version.to_string()),
        ("port", ports.service_port().to_string()),
        ("zmq_port", ports.zmq_port.to_string()),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_string(), value))
    .collect();
    if ports.web_port.is_some() {
        records.insert("elements".into(), "/elements/vstimd.js".into());
    }
    if let Some(event_port) = ports.event_port {
        records.insert("event_port".into(), event_port.to_string());
    }
    records
}

/// This box's name, as somebody standing next to the rig would say it.
pub fn hostname() -> String {
    sysinfo::System::host_name().unwrap_or_else(|| "vstimd".into())
}

fn short_hostname() -> String {
    hostname().split('.').next().unwrap_or_default().to_string()
}

/// One `_vstimd._tcp` registration, withdrawn when the server stops.
pub struct MdnsServiceAdvertisement {
    ports: AdvertisedPorts,
    version: String,
    daemon: Option<mdns_sd::ServiceDaemon>,
    fullname: Option<String>,
}

impl MdnsServiceAdvertisement {
    pub fn new(ports: AdvertisedPorts, version: &str) -> Self {
        Self {
            ports,
            version: version.to_string(),
            daemon: None,
            fullname: None,
        }
    }

    /// The record, as the responder wants it. Separate so a test can read it.
    pub fn build_service_info(&self) -> Result<mdns_sd::ServiceInfo, String> {
        let machine_id = Path::new(MACHINE_ID_PATH);
        let records = text_records_for(
            &salted_machine_identifier("vstimd:", machine_id),
            &salted_machine_identifier("braemons:", machine_id),
            self.ports,
            &self.version,
        );
        let host = short_hostname();
        mdns_sd::ServiceInfo::new(
            SERVICE_TYPE,
            &host,
            &format!("{host}.local."),
            "",
            self.ports.service_port(),
            records,
        )
        .map(|info| info.enable_addr_auto())
        .map_err(|problem| problem.to_string())
    }

    /// Register. Returns whether it worked; never fails the server.
    pub fn start(&mut self) -> bool {
        let registered = (|| {
            let info = self.build_service_info()?;
            let daemon = mdns_sd::ServiceDaemon::new().map_err(|problem| problem.to_string())?;
            let fullname = info.get_fullname().to_string();
            daemon
                .register(info)
                .map_err(|problem| problem.to_string())?;
            Ok::<_, String>((daemon, fullname))
        })();
        match registered {
            Ok((daemon, fullname)) => {
                log::info!("mDNS: advertising {fullname}");
                self.daemon = Some(daemon);
                self.fullname = Some(fullname);
                true
            }
            Err(problem) => {
                log::warn!(
                    "mDNS: not advertising ({problem}). Commands are still served; a console \
                     will need this rig's address by hand."
                );
                false
            }
        }
    }

    /// Withdraw the record, then close. Never fails.
    pub fn stop(&mut self) {
        if let (Some(daemon), Some(fullname)) = (&self.daemon, &self.fullname) {
            let _ = daemon.unregister(fullname);
        }
        if let Some(daemon) = self.daemon.take() {
            let _ = daemon.shutdown();
        }
        self.fullname = None;
    }
}

impl Drop for MdnsServiceAdvertisement {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PORTS: AdvertisedPorts = AdvertisedPorts {
        zmq_port: 5555,
        event_port: Some(5556),
        web_port: Some(8080),
    };

    #[test]
    fn the_rig_identifier_is_the_familys_and_the_daemon_one_is_its_own() {
        let directory = std::env::temp_dir().join(format!("vstimd-mdns-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let machine_id = directory.join("machine-id");
        std::fs::write(&machine_id, "dc4f4b06f2d84f6b9e2a7b0c1d2e3f40\n").unwrap();

        let rig = salted_machine_identifier("braemons:", &machine_id);
        assert_eq!(rig.len(), 16);
        assert_ne!(rig, salted_machine_identifier("vstimd:", &machine_id));
        assert!(!rig.contains("dc4f4b06"));
        // statemachined's `id` for this machine-id, from its own test: proof
        // that the salting is the one the family uses.
        assert_eq!(
            salted_machine_identifier("statemachined:", &machine_id),
            "0b60208b6da842d7"
        );
        std::fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn the_srv_port_is_the_panels_and_the_zmq_ports_are_text_records() {
        let records = text_records_for("abc", "def", PORTS, "1.2.3");
        assert_eq!(PORTS.service_port(), 8080);
        assert_eq!(records["port"], "8080");
        assert_eq!(records["elements"], "/elements/vstimd.js");
        assert_eq!(records["zmq_port"], "5555");
        assert_eq!(records["event_port"], "5556");
        assert_eq!(records["rig"], "def");
    }

    #[test]
    fn without_the_web_surface_the_srv_port_is_the_command_port() {
        let ports = AdvertisedPorts {
            web_port: None,
            event_port: None,
            ..PORTS
        };
        let records = text_records_for("abc", "def", ports, "1.2.3");
        assert_eq!(ports.service_port(), 5555);
        assert_eq!(records["port"], "5555");
        assert!(!records.contains_key("elements"));
        assert!(!records.contains_key("event_port"));
    }
}
