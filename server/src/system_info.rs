use crate::render::RenderTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClockSource {
    /// DRM kernel scanout event (`drmWaitVBlank`) — most accurate.
    DrmVblank,
    /// `VK_EXT_display_control` vblank fence — accurate, equivalent to DrmVblank.
    VkDisplayControl,
    /// `VK_KHR_present_wait` — accurate, GPU-side.
    PresentWait,
    /// `VK_GOOGLE_display_timing` — approximate.
    DisplayTiming,
    /// Fallback: `Instant::now()` after GPU fence — inaccurate.
    GpuCompletion,
}

impl ClockSource {
    pub fn as_str(self) -> &'static str {
        match self {
            ClockSource::DrmVblank => "DRM_IOCTL_WAIT_VBLANK",
            ClockSource::VkDisplayControl => "VK_EXT_display_control",
            ClockSource::PresentWait => "VK_KHR_present_wait",
            ClockSource::DisplayTiming => "VK_GOOGLE_display_timing",
            ClockSource::GpuCompletion => "GPU-completion (inaccurate)",
        }
    }

    /// Parses a clock-source preference, as used in rig-config's
    /// `[display] clock` key and the `--preferred-clock-source` CLI flag.
    /// `"auto"` (case-insensitive) means "auto-detect" (`None`); any other
    /// value must name a variant in snake_case (e.g. `"drm_vblank"`,
    /// `"vk_display_control"`, `"present_wait"`, `"gpu_completion"`).
    pub fn parse_pref(s: &str) -> Result<Option<Self>, String> {
        if s.eq_ignore_ascii_case("auto") {
            return Ok(None);
        }
        use serde::Deserialize;
        use serde::de::IntoDeserializer;
        ClockSource::deserialize(IntoDeserializer::<serde::de::value::Error>::into_deserializer(
            s,
        ))
        .map(Some)
        .map_err(|e| {
            format!(
                "invalid clock source {s:?} ({e}) — expected \"auto\", \"drm_vblank\", \
                 \"vk_display_control\", \"present_wait\", or \"gpu_completion\""
            )
        })
    }
}

/// Static host facts collected in `main` before any Vulkan initialisation.
pub struct HostInfo {
    pub hardware_model: String,
    pub hostname: String,
    pub local_ip: String,
    pub zmq_port: u16,
    /// Render-thread affinity/priority actually in effect (see [`crate::process::sched`]).
    pub sched: crate::process::sched::SchedStatus,
}

pub struct SystemInfo {
    pub host: HostInfo,
    pub gpu_name: String,
    pub backend: RenderTarget,
    pub supports_wireframe: bool,
    pub clock_source: ClockSource,
}

/// Detect the hardware platform by reading device-tree or DMI sysfs.
/// Returns a human-readable model string, e.g. "NVIDIA Jetson Orin Nano …"
/// or "Raspberry Pi 5 Model B Rev 1.0".  Falls back to "unknown".
pub fn query_hardware_model() -> String {
    // ARM/RISC-V boards expose their model via the device-tree.
    // The file is null-terminated; take the first null-delimited token.
    if let Ok(raw) = std::fs::read("/proc/device-tree/model") {
        let s = raw
            .split(|&b| b == 0)
            .next()
            .and_then(|b| std::str::from_utf8(b).ok())
            .unwrap_or("")
            .trim();
        if !s.is_empty() {
            return s.to_owned();
        }
    }
    // x86 / UEFI systems expose the product name via DMI.
    if let Ok(s) = std::fs::read_to_string("/sys/devices/virtual/dmi/id/product_name") {
        let s = s.trim();
        if !s.is_empty() && s != "To Be Filled By O.E.M." {
            return s.to_owned();
        }
    }
    "unknown".to_owned()
}

/// Resolve the default-route local IP by connecting a UDP socket (no packets sent).
pub fn query_local_ip() -> String {
    (|| -> Option<String> {
        let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
        s.connect("8.8.8.8:80").ok()?;
        Some(s.local_addr().ok()?.ip().to_string())
    })()
    .unwrap_or_else(|| "unknown".to_owned())
}

pub fn query_hostname() -> String {
    std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".to_owned())
}
