//! evdi DRM node detection.
//!
//! DisplayLink docks show up as their own `evdi`-driven DRM nodes
//! (`/dev/dri/cardN`), separate from the real GPU/display-controller nodes —
//! on a Raspberry Pi 5 alongside `v3d` (compute GPU, no connectors) and
//! `vc4-drm` (the real HDMI controller). Finding and opening an evdi node
//! needs no compositor; only *driving* it (the rest of `render/evdi/`) does.

use drm::Device as DrmDevice;
use drm::control::Device as CtrlDevice;
use drm::control::connector;
use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::os::fd::{AsFd, BorrowedFd};

pub struct Card(pub File);

impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl DrmDevice for Card {}
impl CtrlDevice for Card {}

/// An `evdi` DRM node with at least one connected connector.
pub struct EvdiNode {
    pub path: String,
    pub card: Card,
    pub connector: connector::Handle,
}

/// Walk `/dev/dri/card0..7`, return the first `evdi`-driven node that
/// reports a connected connector.
pub fn find_connected_evdi() -> Option<EvdiNode> {
    for n in 0..8u32 {
        let path = format!("/dev/dri/card{n}");
        let file = match OpenOptions::new().read(true).write(true).open(&path) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let card = Card(file);

        let driver = match card.get_driver() {
            Ok(d) => d,
            Err(_) => continue,
        };
        if driver.name() != OsStr::new("evdi") {
            continue;
        }

        let res = match card.resource_handles() {
            Ok(r) => r,
            Err(_) => continue,
        };

        let connected = res.connectors().iter().find_map(|&conn_h| {
            let conn = card.get_connector(conn_h, false).ok()?;
            (conn.state() == connector::State::Connected).then_some(conn_h)
        });

        match connected {
            Some(connector) => {
                log::info!("vstimd: evdi node at {path} has a connected connector");
                return Some(EvdiNode { path, card, connector });
            }
            None => log::debug!("vstimd: evdi node at {path} has no connected connector"),
        }
    }

    None
}
