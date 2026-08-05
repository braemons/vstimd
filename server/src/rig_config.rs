/// Machine-specific configuration loaded at startup from `rig-config.toml`.
///
/// Unlike `stim-config` (scene + named VTL lines, changed per experiment),
/// `rig-config` describes the physical rig and changes only when the hardware
/// is reconfigured:
///
///   - VTL shared-memory parameters (shm name, bank counts, vblank trigger bit)
///   - Display preferences for DRM/console mode (resolution, refresh rate)
///   - Thread scheduling options (CPU affinity, real-time priorities)
///
/// Default path: `/etc/braemons/vstimd-rig-config.toml`
/// Override with the `--rig-config` flag.
///
/// If the file is absent vstimd falls back to built-in defaults and logs a
/// notice — useful for development machines without a full rig setup.
use crate::render::RenderTargetPref;
use crate::render::system_info::ClockSource;
use crate::vtl_state::VtlBit;

pub const DEFAULT_PATH: &str = "/etc/braemons/vstimd-rig-config.toml";
const EXAMPLES_DIR: &str = "/usr/share/braemons/vstimd/";

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct RigConfig {
    #[serde(default)]
    pub vtl: VtlRigConfig,
    #[serde(default)]
    pub display: DisplayRigConfig,
    #[serde(default)]
    pub scheduling: SchedulingRigConfig,
    #[serde(default)]
    pub web: WebRigConfig,
    #[serde(default)]
    pub startup: StartupRigConfig,
}

/// What (if anything) vstimd loads into the scene at startup, and whether it
/// saves the scene back out on quit — so a rig can boot into a known
/// configuration with no client attached.
///
/// Overridden by the `--config <path>` CLI flag: if that is given, it wins and
/// `load_config` is ignored.
#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct StartupRigConfig {
    /// Named config (in the `--config-dir`) to load at boot. The literal
    /// `"last"` resolves to the auto-saved last-session slot (see
    /// `save_on_quit`). Omit — or set to the empty string — to start with an
    /// empty scene.
    #[serde(default, deserialize_with = "deserialize_startup_load")]
    pub load_config: Option<StartupLoad>,
    /// Save the current scene to the last-session slot on graceful shutdown, so
    /// the next boot can restore it via `load_config = "last"`. Default: false.
    #[serde(default)]
    pub save_on_quit: bool,
}

/// The resolved target of `[startup] load_config`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartupLoad {
    /// Load a specific named config from the config directory.
    Named(String),
    /// Load the auto-saved last-session config. Missing on a first boot (or
    /// when `save_on_quit` has never run) — treated as a no-op, not an error.
    LastSession,
}

/// Deserializes `[startup] load_config`. The literal `"last"` (any case) maps
/// to [`StartupLoad::LastSession`]; the empty string maps to `None` (empty
/// scene); anything else is a named config.
fn deserialize_startup_load<'de, D>(deserializer: D) -> Result<Option<StartupLoad>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize as _;
    let s = String::deserialize(deserializer)?;
    Ok(match s.trim() {
        "" => None,
        last if last.eq_ignore_ascii_case("last") => Some(StartupLoad::LastSession),
        name => Some(StartupLoad::Named(name.to_string())),
    })
}

/// Embedded web control surface (HTTP + WebSocket) settings.
///
/// The web server can also be compiled out entirely via the `web` Cargo feature
/// (on by default). When the feature is disabled these fields are ignored.
/// CLI flags (`--no-web`, `--web-port`) override these values.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebRigConfig {
    /// Whether to start the web control surface. Default: true.
    #[serde(default = "WebRigConfig::default_enabled")]
    pub enabled: bool,
    /// HTTP/WebSocket port. Default: 8080.
    #[serde(default = "WebRigConfig::default_port")]
    pub port: u16,
}

impl WebRigConfig {
    fn default_enabled() -> bool { true }
    fn default_port() -> u16 { 8080 }
}

impl Default for WebRigConfig {
    fn default() -> Self {
        Self { enabled: Self::default_enabled(), port: Self::default_port() }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VtlRigConfig {
    /// POSIX shared-memory name for the VTL segment (must start with `/`).
    #[serde(default = "VtlRigConfig::default_shm_name")]
    pub shm_name: String,
    /// Number of 64-bit input banks (1–4).  Each bank holds 64 input lines.
    /// 1 is sufficient for up to 64 physical trigger inputs.
    #[serde(default = "VtlRigConfig::default_input_banks")]
    pub num_input_banks: u32,
    /// Number of 64-bit output banks (1–4).
    #[serde(default = "VtlRigConfig::default_output_banks")]
    pub num_output_banks: u32,
    /// Output bit pulsed HIGH at the start of each frame (immediately after the
    /// vblank wait) and LOW once the GPU work is submitted.  The pulse width is
    /// vstimd's per-frame compute time.  Omit to disable.
    ///
    /// Choose a bit not used by any gpiochip-daqd output line so there is no
    /// conflict.  Bit 63 on bank 0 is a safe default.
    ///
    /// Addressed by (bank, bit) only — the vblank line is always an output, so
    /// no kind is specified in the rig config.
    pub vblank: Option<VblankBit>,
}

/// A (bank, bit) address for the vblank output line in the rig config.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VblankBit {
    pub bank: usize,
    pub bit:  u8,
}

impl VblankBit {
    /// The resolved output-directed [`VtlBit`].
    pub fn to_vtl_bit(self) -> VtlBit {
        VtlBit { bank: self.bank, bit: self.bit, kind: vtl::VtlKind::Output }
    }
}

impl VtlRigConfig {
    fn default_shm_name() -> String  { "/vstimd_vtl".into() }
    fn default_input_banks() -> u32  { 1 }
    fn default_output_banks() -> u32 { 1 }
}

impl Default for VtlRigConfig {
    fn default() -> Self {
        Self {
            shm_name:         Self::default_shm_name(),
            num_input_banks:  Self::default_input_banks(),
            num_output_banks: Self::default_output_banks(),
            vblank:           None,
        }
    }
}

/// Preferred display mode for DRM/console output.
///
/// All fields are optional and independently filter the modes `VK_KHR_display`
/// reports for the connected display; a field left `None` is not filtered on.
/// If no reported mode matches, vstimd logs a warning and falls back to
/// auto-select. Omit all fields to always auto-select (highest refresh rate,
/// then highest resolution as a tie-break — Vulkan does not expose the
/// display's EDID-preferred-mode flag).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayRigConfig {
    /// Forces a specific render backend, bypassing the `DISPLAY`/
    /// `WAYLAND_DISPLAY`-based auto-detection. `"auto"` (default — also used
    /// if the key is omitted) auto-detects: desktop session → winit,
    /// bare console → DRM. Other values: `"drm"`, `"desktop"`, `"null"`,
    /// `"evdi"`.
    ///
    /// Lets a headless rig (e.g. booted via the `vstimd.target` systemd
    /// unit, with no `--evdi`/`--null` flag in `ExecStart=`) pick a
    /// non-default backend — most usefully `"evdi"`, so a DisplayLink
    /// output is used without editing the systemd unit. The `--null` and
    /// `--evdi` CLI flags still take priority over this when given.
    #[serde(default, deserialize_with = "deserialize_backend_pref")]
    pub backend: Option<RenderTargetPref>,
    /// Preferred horizontal resolution (pixels).
    pub width: Option<u32>,
    /// Preferred vertical resolution (pixels).
    pub height: Option<u32>,
    /// Preferred refresh rate (Hz), e.g. `60.0` or `144.0`.
    pub refresh_hz: Option<f64>,
    /// Scale factor applied to the egui overlay UI, independent of the OS/window
    /// DPI scale. Useful on high-DPI displays (e.g. 4K) where the overlay text
    /// and controls would otherwise be unreadably small — and required in DRM
    /// mode, which has no OS-reported scale factor at all. Applies to both
    /// desktop and DRM render targets. Overridable with `--overlay-scale`.
    #[serde(default = "DisplayRigConfig::default_overlay_scale")]
    pub overlay_scale: f32,
    /// Forces a specific vblank clock source for DRM/console mode, bypassing
    /// auto-detection. `"auto"` (default — also used if the key is omitted)
    /// tries `DRM_IOCTL_WAIT_VBLANK` first, falling back to
    /// `VK_EXT_display_control`, then `VK_KHR_present_wait`, then
    /// GPU-completion timestamps. Other values: `"drm_vblank"`,
    /// `"vk_display_control"`, `"present_wait"`, `"gpu_completion"`.
    ///
    /// Auto-detection can't reliably predict every GPU/driver combination —
    /// on some hardware a clock source that passes an initial check still
    /// fails once the render loop is running. Set this to pin a specific
    /// source instead of probing: vstimd will use exactly that source, or
    /// fail loudly at startup with an actionable error if it isn't
    /// available, rather than silently trying alternatives. `"display_timing"`
    /// is not a valid choice here (it's not wired up as a selectable clock in
    /// DRM mode) and is rejected at startup.
    #[serde(default, deserialize_with = "deserialize_clock_pref")]
    pub clock: Option<ClockSource>,
}

/// Deserializes the `[display] clock` key: the literal string `"auto"` maps
/// to `None` (the auto-detecting path); any other value is parsed as a
/// `ClockSource` variant name. Lets the TOML file say `clock = "auto"`
/// explicitly rather than only supporting auto-detection via omitting the
/// key entirely.
fn deserialize_clock_pref<'de, D>(deserializer: D) -> Result<Option<ClockSource>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize as _;
    use serde::de::Error as _;

    let s = String::deserialize(deserializer)?;
    ClockSource::parse_pref(&s).map_err(D::Error::custom)
}

/// Deserializes the `[display] backend` key — same `"auto"` → `None`
/// convention as `deserialize_clock_pref` above.
fn deserialize_backend_pref<'de, D>(deserializer: D) -> Result<Option<RenderTargetPref>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize as _;
    use serde::de::Error as _;

    let s = String::deserialize(deserializer)?;
    RenderTargetPref::parse_pref(&s).map_err(D::Error::custom)
}

impl DisplayRigConfig {
    fn default_overlay_scale() -> f32 { 1.0 }
}

impl Default for DisplayRigConfig {
    fn default() -> Self {
        Self {
            backend: None,
            width: None,
            height: None,
            refresh_hz: None,
            overlay_scale: Self::default_overlay_scale(),
            clock: None,
        }
    }
}

/// Thread scheduling options for vstimd.
///
/// Both are opt-in and applied to the render/vblank thread only — see
/// [`crate::sched`].
#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct SchedulingRigConfig {
    /// CPU core to pin the render/vblank thread to.  Unpinned by default —
    /// pinning is a rig-specific decision, so it is never applied implicitly.
    pub render_cpu_core: Option<usize>,
    /// `SCHED_FIFO` priority (1–99) for the render/vblank thread.  Defaults to
    /// [`crate::sched::DEFAULT_RENDER_RT_PRIO`] when omitted; set `0` to stay
    /// on `SCHED_OTHER`.  Needs `CAP_SYS_NICE`; warns and continues without it.
    pub render_rt_prio: Option<i32>,
}

/// Load a rig-config from `path`.  Returns `Ok(RigConfig::default())` if the
/// file does not exist (non-fatal), or an error if the file exists but is
/// malformed.
pub fn load(path: &str) -> anyhow::Result<RigConfig> {
    match std::fs::read_to_string(path) {
        Ok(raw) => {
            let cfg: RigConfig = toml::from_str(&raw)
                .map_err(|e| anyhow::anyhow!("rig-config {path}: {e}"))?;
            log::info!("vstimd: rig-config loaded from {path}");
            Ok(cfg)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            log::info!(
                "vstimd: rig-config not found at {path} — using built-in defaults. \
                 Copy a board example from {EXAMPLES_DIR} to customise."
            );
            Ok(RigConfig::default())
        }
        Err(e) => Err(anyhow::anyhow!("rig-config {path}: {e}")),
    }
}
