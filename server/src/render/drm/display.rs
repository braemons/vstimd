use std::os::fd::{AsFd, BorrowedFd};

use drm_sys::control::{Device as ControlDevice, connector};

/// DRM device wrapper — satisfies the `drm` crate's `AsFd` requirement.
struct Card(std::fs::File);

impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

// Blanket impls: all modesetting ops are provided by the traits using `as_fd`.
impl drm_sys::Device for Card {}
impl ControlDevice for Card {}

/// Information about the first connected display, gathered from KMS.
///
/// The `file` handle must be kept alive until `vkAcquireDrmDisplayEXT` is
/// called; after that Vulkan owns the display and the file can be dropped.
pub struct DisplayInfo {
    pub file: std::fs::File,
    /// Raw DRM connector ID, passed to `vkGetDrmDisplayEXT`.
    pub connector_id: u32,
    pub width: u32,
    pub height: u32,
}

/// Open the first available DRM card that has a connected display, and return
/// the display's geometry. Tries `/dev/dri/card0` through `card3`.
pub fn find_display() -> Result<DisplayInfo, String> {
    let card = (0..4)
        .find_map(|i| {
            let path = format!("/dev/dri/card{i}");
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
                .ok()
                .map(Card)
        })
        .ok_or_else(|| {
            "no DRM card found at /dev/dri/card0..3 — is the 'video' group set?".to_string()
        })?;

    let res = card
        .resource_handles()
        .map_err(|e| format!("DRM resource_handles() failed: {e}"))?;

    for &conn_handle in res.connectors() {
        let conn = card
            .get_connector(conn_handle, false)
            .map_err(|e| format!("get_connector failed: {e}"))?;

        if conn.state() != connector::State::Connected {
            continue;
        }

        let Some(mode) = conn.modes().first() else {
            continue;
        };

        let (hdisplay, vdisplay) = mode.size();
        let connector_id = {
            use drm_sys::control::ResourceHandle as _;
            conn_handle.as_raw()
        };

        // Consume the Card, extracting the inner File so the fd stays open.
        let file = card.0;
        return Ok(DisplayInfo {
            file,
            connector_id,
            width: hdisplay as u32,
            height: vdisplay as u32,
        });
    }

    Err("no connected display found on any DRM connector".to_string())
}
