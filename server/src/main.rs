use std::sync::{Arc, Mutex, RwLock};

#[cfg(target_os = "linux")]
use vstimd::render::drm::DrmBackend;
use vstimd::render::{
    BackendData, ClockSource, DisplayModePref, HostInfo, NullBackend, RenderTarget,
    RenderTargetPref, WindowMode,
};
use vstimd::render::{query_hardware_model, query_hostname, query_local_ip};
use vstimd::render::winit_vk::WinitBackend;
use vstimd::rig_config;
use vstimd::scene::SceneState;
use vstimd::vtl_state::VtlState;

fn main() {
    let args = parse_args();

    let default_level = if args.verbose { "debug" } else { "info" };
    let server_start = std::time::Instant::now();
    let env_logger =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(default_level))
            .build();
    let log_buffer = vstimd::log_buffer::install(env_logger, server_start);

    log::info!(
        "vstimd v{} (built {})",
        env!("VSTIMD_VERSION"),
        env!("VSTIMD_BUILD_DATE"),
    );
    let host_info = HostInfo {
        hardware_model: query_hardware_model(),
        hostname: query_hostname(),
        local_ip: query_local_ip(),
        zmq_port: vstimd::ipc::DEFAULT_ZMQ_PORT,
    };
    log::info!("vstimd: hardware: {}", host_info.hardware_model);

    // Load rig-config (hardware-specific settings). Non-fatal if absent; logs
    // whether it actually found+parsed a file at `args.rig_config` (which
    // defaults to rig_config::DEFAULT_PATH, /etc/braemons/vstimd-rig-config.toml)
    // or fell back to built-in defaults.
    let rig = rig_config::load(&args.rig_config).unwrap_or_else(|e| {
        log::error!("vstimd: {e}");
        std::process::exit(1);
    });
    if let Some(w) = rig.display.width {
        log::info!(
            "vstimd: rig display preference: {w}×{}@{}Hz (DRM mode only)",
            rig.display.height.unwrap_or(0),
            rig.display.refresh_hz.unwrap_or(0.0),
        );
    }

    // Resolve the render target: an explicit --null/--evdi CLI flag wins;
    // otherwise rig-config's [display] backend; otherwise DISPLAY-env
    // auto-detection. Deferred to here (rather than parse_args()) because
    // rig-config isn't loaded yet when arguments are parsed.
    let has_display = std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok();
    let render_target = resolve_render_target(
        args.render_target,
        rig.display.backend,
        args.window_mode,
        has_display,
    );
    if args.explicit_windowed && render_target == RenderTarget::Drm {
        eprintln!(
            "vstimd: --windowed requires a desktop session \
             (DISPLAY or WAYLAND_DISPLAY must be set, or rig-config's \
             [display] backend must not be \"drm\"). \
             DRM/console mode does not support windowed output."
        );
        std::process::exit(1);
    }
    log::info!("vstimd: render target: {:?}", render_target);

    let config_dir = resolve_config_dir(args.config_dir.clone());
    log::info!("vstimd: config dir: {}", config_dir.display());
    let scene = Arc::new(RwLock::new(SceneState::new_with_config_dir(
        config_dir.clone(),
    )));

    // Seed display metrics from rig-config so headless/null mode reports a
    // screen size and refresh rate (the GPU backends overwrite `screen_size`
    // with the real swapchain extent in render_frame). Without this, the web
    // UI map has no aspect ratio under `--null`.
    {
        let mut s = scene.write().expect("scene lock poisoned");
        let w = rig.display.width.unwrap_or(1920);
        let h = rig.display.height.unwrap_or(1080);
        s.runtime.screen_size = Some((w, h));
        if let Some(hz) = rig.display.refresh_hz {
            s.runtime.frame_rate = hz as f32;
        }
    }

    // Create VTL shared memory on Linux using rig-config parameters.
    // The Arc<Mutex<>> lets both the ZMQ thread (software triggers, naming)
    // and the render backend (frame polling) access it safely.
    #[cfg(target_os = "linux")]
    let vtl: Option<Arc<Mutex<VtlState>>> = vtl::VtlOwner::create(
        &rig.vtl.shm_name,
        rig.vtl.num_input_banks,
        rig.vtl.num_output_banks,
    )
    .map(|owner| {
        let mut state = VtlState::new(owner);
        state.vblank_vtl = rig.vtl.vblank.map(|v| v.to_vtl_bit());
        Arc::new(Mutex::new(state))
    })
    .map_err(|e| log::warn!("vtl: failed to create shm segment: {e}"))
    .ok();
    #[cfg(not(target_os = "linux"))]
    let vtl: Option<Arc<Mutex<VtlState>>> = None;

    if vtl.is_some() {
        log::info!(
            "vtl: segment '{}' created ({} in / {} out bank(s)){}",
            rig.vtl.shm_name,
            rig.vtl.num_input_banks,
            rig.vtl.num_output_banks,
            rig.vtl.vblank.map_or(String::new(), |vb| format!("  vblank=bank{}·bit{}", vb.bank, vb.bit)),
        );
    }

    // Startup scene load. An explicit `--config <path>` wins; otherwise the
    // rig-config `[startup] load_config` (a named config in the config dir, or
    // "last" for the auto-saved last-session slot) is applied, if set.
    if let Some(ref path) = args.config_file {
        match vstimd::io_config::load_config(path) {
            Ok((scene_cfg, io)) => {
                if let Some(ref v) = vtl {
                    let mut v = v.lock().unwrap();
                    v.config.names = io.vtl.names;
                    v.sync_names_to_shm();
                }
                scene
                    .write()
                    .unwrap()
                    .load_snapshot(scene_cfg, vstimd::scene::LoadMode::Replace);
                log::info!("vstimd: loaded config from {:?}", path);
            }
            Err(e) => log::error!("vstimd: failed to load config {:?}: {e}", path),
        }
    } else if let Some(load) = &rig.startup.load_config {
        let name = match load {
            rig_config::StartupLoad::Named(n) => n.as_str(),
            rig_config::StartupLoad::LastSession => vstimd::io_config::LAST_SESSION_CONFIG,
        };
        // Runs before the ZMQ/web threads spawn, but keep scene-then-vtl lock
        // order consistent with ipc.rs regardless.
        let mut scene_guard = scene.write().unwrap();
        let mut vtl_guard = vtl.as_ref().map(|v| v.lock().unwrap());
        let result = scene_guard.load_named_config(name, false, vtl_guard.as_deref_mut());
        match result {
            Ok(()) => log::info!("vstimd: loaded startup config '{name}'"),
            // A missing last-session slot on first boot is expected, not an error.
            Err(e)
                if matches!(load, rig_config::StartupLoad::LastSession)
                    && vstimd::io_config::is_not_found(&e) =>
            {
                log::info!(
                    "vstimd: no last-session config yet ('{name}') — starting with an empty scene"
                );
            }
            Err(e) => log::error!("vstimd: failed to load startup config '{name}': {e}"),
        }
    }

    let (zmq_thread, zmq_shutdown, zmq_bound) = vstimd::ipc::spawn_zmq_thread(
        scene.clone(),
        vtl.clone(),
        &format!("tcp://0.0.0.0:{}", args.zmq_port),
    );

    // Embedded web control surface (HTTP + WebSocket). Shares the scene/vtl Arcs
    // and reuses handle_request — no per-frame render cost. Compiled in only with
    // the `web` Cargo feature; gated at runtime by rig-config `[web].enabled`
    // (overridable by `--no-web` / `--web-port`).
    #[cfg(feature = "web")]
    let web = {
        let enabled = args.web_enabled.unwrap_or(rig.web.enabled);
        let port = args.web_port.unwrap_or(rig.web.port);
        if enabled {
            Some(vstimd::web::spawn_web_thread(
                scene.clone(),
                vtl.clone(),
                &format!("0.0.0.0:{}", port),
            ))
        } else {
            log::info!("vstimd: web control surface disabled");
            None
        }
    };

    // Install signal handlers once, before any render path (including Vulkan
    // init which can take several seconds on DRM).
    install_signal_handlers();

    let overlay_scale = args.overlay_scale.unwrap_or(rig.display.overlay_scale);
    log::info!("vstimd: overlay scale: {overlay_scale}");

    let display_pref = DisplayModePref {
        width: rig.display.width,
        height: rig.display.height,
        refresh_hz: rig.display.refresh_hz,
    };
    let clock_pref = args.preferred_clock_source.unwrap_or(rig.display.clock);
    match clock_pref {
        Some(clock) => log::info!("vstimd: forcing vblank clock: {}", clock.as_str()),
        None => log::info!("vstimd: vblank clock: auto-detect"),
    }

    // Keep Arc clones so the scene can be saved on quit after the render loop
    // (which moves `scene`/`vtl` into BackendData) returns. Cheap Arc clones.
    let scene_for_quit = scene.clone();
    let vtl_for_quit = vtl.clone();

    let data = BackendData {
        scene,
        vtl,
        host_info,
        overlay_scale,
        display_pref,
        clock_pref,
        rig_config_path: args.rig_config.clone(),
    };
    let zmq_port = args.zmq_port;
    let on_ready = move || {
        if wait_zmq_bound(&zmq_bound, zmq_port) {
            notify_ready();
        }
    };

    match render_target {
        #[cfg(target_os = "linux")]
        RenderTarget::Drm => DrmBackend::new(data, log_buffer).run(on_ready),
        #[cfg(not(target_os = "linux"))]
        RenderTarget::Drm => {
            log::error!("DRM/console mode is only available on Linux");
            std::process::exit(1);
        }
        RenderTarget::Desktop(window_mode) => {
            WinitBackend::new(data, window_mode, log_buffer).run(on_ready);
        }
        RenderTarget::Null => NullBackend::new(data).run(on_ready),
        #[cfg(target_os = "linux")]
        RenderTarget::Evdi => {
            vstimd::render::evdi::EvdiBackend::new(data, log_buffer).run(on_ready);
        }
        #[cfg(not(target_os = "linux"))]
        RenderTarget::Evdi => {
            log::error!("--evdi is only available on Linux");
            std::process::exit(1);
        }
    }

    // Persist the scene for the next boot if the rig-config asks for it. Runs
    // after the render loop returns (shutdown requested) while the scene Arc is
    // still live, so `load_config = "last"` can restore it next time.
    if rig.startup.save_on_quit {
        // Lock scene-then-vtl to match the ZMQ thread's order (ipc.rs), which is
        // still running here — the reverse order could deadlock mid-request.
        let scene_guard = scene_for_quit.read().unwrap();
        let vtl_guard = vtl_for_quit.as_ref().map(|v| v.lock().unwrap());
        match scene_guard.save_session_snapshot(vtl_guard.as_deref()) {
            Ok(archive) => {
                log::info!("vstimd: saved session on quit (last-session + archive '{archive}')")
            }
            Err(e) => log::error!("vstimd: failed to save session on quit: {e}"),
        }
    }

    // Signal the web thread to exit and wait for it to finish.
    #[cfg(feature = "web")]
    if let Some((web_thread, web_shutdown, _)) = web {
        let _ = web_shutdown.send(());
        web_thread.join().ok();
    }

    // Signal the ZMQ thread to exit and wait for it to finish.  This ensures
    // the thread's Arc references are released — VtlOwner::drop runs shm_unlink
    // and the shm segment is cleaned up before the process exits.
    drop(zmq_shutdown);
    zmq_thread.join().ok();

    if let Some(reason) = vstimd::shutdown::fatal_reason() {
        log::error!("vstimd: exiting after fatal error: {reason}");
        std::process::exit(1);
    }
}

// ── Argument parsing ──────────────────────────────────────────────────────────

struct Args {
    /// `Some(_)` if `--null` or `--evdi` forced a specific target on the
    /// command line — takes priority over rig-config. `None` means "resolve
    /// later": rig-config's `[display] backend`, then DISPLAY-env
    /// auto-detection (see `main()`, which loads rig-config after
    /// `parse_args()` returns).
    render_target: Option<RenderTarget>,
    window_mode: WindowMode,
    /// True if `--windowed` was passed — used to validate against the
    /// eventually-resolved render target once rig-config is loaded (DRM mode
    /// doesn't support windowed output).
    explicit_windowed: bool,
    verbose: bool,
    zmq_port: u16,
    /// `Some(false)` if `--no-web` was passed; otherwise `None` (use rig-config).
    #[cfg_attr(not(feature = "web"), allow(dead_code))]
    web_enabled: Option<bool>,
    /// `Some(p)` if `--web-port` was passed; otherwise `None` (use rig-config).
    #[cfg_attr(not(feature = "web"), allow(dead_code))]
    web_port: Option<u16>,
    rig_config: String,
    config_file: Option<std::path::PathBuf>,
    config_dir: Option<std::path::PathBuf>,
    /// `Some(s)` if `--overlay-scale` was passed; otherwise `None` (use rig-config).
    overlay_scale: Option<f32>,
    /// `Some(pref)` if `--preferred-clock-source` was passed (overrides rig-config
    /// entirely, including its own `auto` vs. forced choice); otherwise `None`
    /// (use rig-config). The inner `Option<ClockSource>` is `None` for "auto".
    preferred_clock_source: Option<Option<ClockSource>>,
}

/// Choose the directory for named stim-configs (and the save-on-quit slot and
/// archives). An explicit `--config-dir` is honoured verbatim. Otherwise prefer
/// the deployed default (`/var/lib/braemons/vstimd`, matching the packaged
/// systemd `StateDirectory`); if it is not writable — e.g. a non-root dev run —
/// fall back to `~/.local/braemons/vstimd`, then the current directory.
fn resolve_config_dir(explicit: Option<std::path::PathBuf>) -> std::path::PathBuf {
    use vstimd::io_config::{first_writable_dir, DEFAULT_CONFIG_DIR};
    if let Some(dir) = explicit {
        return dir;
    }
    let mut candidates = vec![std::path::PathBuf::from(DEFAULT_CONFIG_DIR)];
    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(std::path::Path::new(&home).join(".local/braemons/vstimd"));
    }
    first_writable_dir(&candidates)
}

/// Automatically detect the best render target for the current platform.
/// `has_display` is passed in — rather than read from the environment here —
/// so the decision itself is a pure, unit-testable function; see
/// `resolve_render_target`'s tests below for the actual `DISPLAY`/
/// `WAYLAND_DISPLAY` read.
///
/// Detection logic:
/// - **Windows/macOS:** Always desktop (winit)
/// - **Linux with DISPLAY or WAYLAND_DISPLAY:** Desktop session → winit
/// - **Linux without display env vars:** Bare console — prefer a connected
///   HDMI/DP display (DRM) if one is plugged in; otherwise fall back to a
///   connected DisplayLink (evdi) output if one is present; otherwise DRM
///   (which will fail loudly at init if nothing is actually connected).
fn detect_render_target(window_mode: WindowMode, has_display: bool) -> RenderTarget {
    if cfg!(not(target_os = "linux")) {
        return RenderTarget::Desktop(window_mode);
    }
    if has_display {
        log::info!("vstimd: detected desktop session (DISPLAY or WAYLAND_DISPLAY set)");
        return RenderTarget::Desktop(window_mode);
    }

    #[cfg(target_os = "linux")]
    {
        if vstimd::render::evdi::has_connected_native_display() {
            log::info!("vstimd: detected console environment with a connected HDMI/DP display");
            return RenderTarget::Drm;
        }
        if vstimd::render::evdi::find_connected_evdi().is_some() {
            log::info!(
                "vstimd: detected console environment with no HDMI/DP display but a connected \
                 DisplayLink (evdi) output — using it"
            );
            return RenderTarget::Evdi;
        }
    }

    log::info!("vstimd: detected console environment (no display server)");
    RenderTarget::Drm
}

/// Resolve the render target from every source, highest priority first:
/// 1. `cli_forced` — an explicit `--null`/`--evdi` flag.
/// 2. `rig_backend` — rig-config's `[display] backend`.
/// 3. `detect_render_target` — `DISPLAY`/`WAYLAND_DISPLAY` auto-detection.
///
/// Pure (no env/global reads) so the precedence chain is unit-testable
/// independent of `detect_render_target`'s own env-var read.
fn resolve_render_target(
    cli_forced: Option<RenderTarget>,
    rig_backend: Option<RenderTargetPref>,
    window_mode: WindowMode,
    has_display: bool,
) -> RenderTarget {
    cli_forced.unwrap_or_else(|| match rig_backend {
        Some(RenderTargetPref::Drm) => RenderTarget::Drm,
        Some(RenderTargetPref::Desktop) => RenderTarget::Desktop(window_mode),
        Some(RenderTargetPref::Null) => RenderTarget::Null,
        Some(RenderTargetPref::Evdi) => RenderTarget::Evdi,
        None => detect_render_target(window_mode, has_display),
    })
}

#[cfg(test)]
mod render_target_resolution_tests {
    use super::*;

    fn windowed() -> WindowMode {
        WindowMode::Windowed { width: 800, height: 600 }
    }

    #[test]
    fn cli_flag_wins_over_rig_config_and_display_env() {
        // --evdi was passed; rig-config says "drm" and a display is even
        // present — the CLI flag still wins outright.
        let target = resolve_render_target(
            Some(RenderTarget::Evdi),
            Some(RenderTargetPref::Drm),
            WindowMode::default(),
            true,
        );
        assert_eq!(target, RenderTarget::Evdi);
    }

    #[test]
    fn rig_config_wins_over_auto_detect_when_no_cli_flag() {
        // No CLI flag; rig-config says "evdi" — used regardless of whether
        // a display session is present. This is the boot-via-systemd case.
        let target = resolve_render_target(None, Some(RenderTargetPref::Evdi), WindowMode::default(), true);
        assert_eq!(target, RenderTarget::Evdi);

        let target = resolve_render_target(None, Some(RenderTargetPref::Evdi), WindowMode::default(), false);
        assert_eq!(target, RenderTarget::Evdi);
    }

    #[test]
    fn rig_config_drm_resolves_to_drm() {
        let target = resolve_render_target(None, Some(RenderTargetPref::Drm), WindowMode::default(), true);
        assert_eq!(target, RenderTarget::Drm);
    }

    #[test]
    fn rig_config_null_resolves_to_null() {
        let target = resolve_render_target(None, Some(RenderTargetPref::Null), WindowMode::default(), false);
        assert_eq!(target, RenderTarget::Null);
    }

    #[test]
    fn rig_config_desktop_carries_window_mode_through() {
        let target = resolve_render_target(None, Some(RenderTargetPref::Desktop), windowed(), false);
        assert_eq!(target, RenderTarget::Desktop(windowed()));
    }

    #[test]
    fn no_cli_flag_no_rig_config_falls_back_to_auto_detect() {
        let target = resolve_render_target(None, None, WindowMode::default(), true);
        assert_eq!(target, RenderTarget::Desktop(WindowMode::default()));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn auto_detect_without_display_picks_drm_on_linux() {
        let target = resolve_render_target(None, None, WindowMode::default(), false);
        assert_eq!(target, RenderTarget::Drm);
    }
}

fn parse_args() -> Args {
    let mut window_mode = WindowMode::default();
    let mut explicit_windowed = false;
    let mut verbose = false;
    let mut null = false;
    let mut evdi = false;
    let mut zmq_port = vstimd::ipc::DEFAULT_ZMQ_PORT;
    let mut web_enabled: Option<bool> = None;
    let mut web_port: Option<u16> = None;
    let mut rig_config  = rig_config::DEFAULT_PATH.to_string();
    let mut config_file: Option<std::path::PathBuf> = None;
    let mut config_dir: Option<std::path::PathBuf> = None;
    let mut overlay_scale: Option<f32> = None;
    let mut preferred_clock_source: Option<Option<ClockSource>> = None;

    let mut args = std::env::args().skip(1).peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--verbose" | "-v" => verbose = true,
            "--null" => null = true,
            "--evdi" => evdi = true,
            "--windowed" | "-w" => {
                let size = args.next().and_then(|s| {
                    let (w, h) = s.split_once('x')?;
                    Some((w.trim().parse::<u32>().ok()?, h.trim().parse::<u32>().ok()?))
                });
                let (w, h) = size.unwrap_or((800, 600));
                window_mode = WindowMode::Windowed {
                    width: w,
                    height: h,
                };
                explicit_windowed = true;
            }
            "--rig-config" => {
                rig_config = args.next().unwrap_or_else(|| {
                    eprintln!("vstimd: --rig-config requires a path argument");
                    std::process::exit(1);
                });
            }
            "--config" => {
                config_file = args.next().map(std::path::PathBuf::from);
            }
            "--config-dir" => {
                config_dir = args.next().map(std::path::PathBuf::from);
            }
            "--zmq-port" => {
                zmq_port = args.next().and_then(|s| s.parse::<u16>().ok()).unwrap_or_else(|| {
                    eprintln!("vstimd: --zmq-port requires a numeric port argument");
                    std::process::exit(1);
                });
            }
            "--overlay-scale" => {
                let s = args.next().and_then(|s| s.parse::<f32>().ok()).unwrap_or_else(|| {
                    eprintln!("vstimd: --overlay-scale requires a numeric argument (e.g. 1.5)");
                    std::process::exit(1);
                });
                if !(s.is_finite() && s > 0.0) {
                    eprintln!("vstimd: --overlay-scale must be a positive number");
                    std::process::exit(1);
                }
                overlay_scale = Some(s);
            }
            "--preferred-clock-source" => {
                let s = args.next().unwrap_or_else(|| {
                    eprintln!(
                        "vstimd: --preferred-clock-source requires a value (auto, drm_vblank, \
                         vk_display_control, present_wait, gpu_completion)"
                    );
                    std::process::exit(1);
                });
                preferred_clock_source = Some(ClockSource::parse_pref(&s).unwrap_or_else(|e| {
                    eprintln!("vstimd: --preferred-clock-source: {e}");
                    std::process::exit(1);
                }));
            }
            "--no-web" => web_enabled = Some(false),
            "--web-port" => {
                let p = args.next().and_then(|s| s.parse::<u16>().ok()).unwrap_or_else(|| {
                    eprintln!("vstimd: --web-port requires a numeric port argument");
                    std::process::exit(1);
                });
                web_port = Some(p);
            }
            "--version" | "-V" => {
                print_version();
                std::process::exit(0);
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            other => {
                eprintln!("vstimd: unknown argument: {other}");
                print_usage();
                std::process::exit(1);
            }
        }
    }

    if null && evdi {
        eprintln!("vstimd: --null and --evdi are mutually exclusive");
        std::process::exit(1);
    }

    // Only `--null`/`--evdi` (or VSTIMD_NULL) force a target here; the
    // rig-config-vs-auto-detect fallback happens in `main()`, after
    // rig-config is loaded — see the `Args::render_target` doc comment.
    let render_target = if null || std::env::var("VSTIMD_NULL").is_ok() {
        Some(RenderTarget::Null)
    } else if evdi {
        Some(RenderTarget::Evdi)
    } else {
        None
    };

    Args {
        render_target,
        window_mode,
        explicit_windowed,
        verbose,
        zmq_port,
        web_enabled,
        web_port,
        rig_config,
        config_file,
        config_dir,
        overlay_scale,
        preferred_clock_source,
    }
}

/// Install SIGTERM/SIGINT handlers that set the shared shutdown flag.
/// Called once before any render path so the handler is active during
/// Vulkan init (which can take several seconds on DRM hardware).
fn install_signal_handlers() {
    #[cfg(target_os = "linux")]
    {
        extern "C" fn on_signal(_: libc::c_int) {
            vstimd::shutdown::request();
        }
        unsafe {
            libc::signal(libc::SIGTERM, on_signal as *const () as libc::sighandler_t);
            libc::signal(libc::SIGINT, on_signal as *const () as libc::sighandler_t);
        }
    }
}

/// Block until the ZMQ thread signals that `socket.bind()` has succeeded.
/// Returns `true` if the signal arrived, `false` on timeout (ZMQ unavailable).
fn wait_zmq_bound(rx: &std::sync::mpsc::Receiver<()>, port: u16) -> bool {
    if rx.recv_timeout(std::time::Duration::from_secs(10)).is_err() {
        log::warn!(
            "vstimd: ZMQ bind did not complete within 10 s — port {port} may not be listening"
        );
        return false;
    }
    true
}

/// Send `READY=1` to systemd via `$NOTIFY_SOCKET` if present.
/// No-op when not launched by systemd or on non-Linux platforms.
fn notify_ready() {
    #[cfg(target_os = "linux")]
    {
        let has_socket = std::env::var_os("NOTIFY_SOCKET").is_some();
        match sd_notify::notify(false, &[sd_notify::NotifyState::Ready]) {
            Ok(()) if has_socket => log::info!("vstimd: systemd READY=1 sent"),
            Ok(()) => {
                log::info!("vstimd: sd_notify: NOTIFY_SOCKET not set (not running under systemd)")
            }
            Err(e) => log::warn!("vstimd: sd_notify failed: {e}"),
        }
    }
}

/// Print version and build provenance: how the binary was compiled (profile,
/// target, enabled features — notably whether the browser UI is embedded) and
/// when. Goes to stdout so `vstimd --version` is pipe-friendly.
fn print_version() {
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };

    // Enabled Cargo features that affect deployment.
    let mut features: Vec<&str> = Vec::new();
    if cfg!(feature = "web") {
        features.push("web");
    }
    if cfg!(feature = "embed-ui") {
        features.push("embed-ui");
    }
    let features = if features.is_empty() {
        "(none)".to_string()
    } else {
        features.join(", ")
    };

    println!("vstimd {}", env!("VSTIMD_VERSION"));
    println!("  commit:   {}", env!("VSTIMD_GIT_HASH"));
    println!("  built:    {}", env!("VSTIMD_BUILD_DATE"));
    println!("  target:   {}", env!("VSTIMD_TARGET"));
    println!("  profile:  {profile}");
    println!("  features: {features}");
    // The single question this flag most often answers: is the web UI baked in?
    let web_ui = if cfg!(feature = "embed-ui") {
        "embedded (served at http://<host>:8080)"
    } else if cfg!(feature = "web") {
        "not embedded (WebSocket API only; `/` serves a placeholder)"
    } else {
        "disabled (compiled out)"
    };
    println!("  web UI:   {web_ui}");
}

fn print_usage() {
    eprintln!("Usage: vstimd [OPTIONS]");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -w, --windowed <WxH>      Start in windowed mode with size WxH (desktop only)");
    eprintln!("      --null                No rendering; ZMQ server only (also: VSTIMD_NULL=1)");
    eprintln!("      --evdi                Render on a DisplayLink (evdi) output directly, no compositor.");
    eprintln!("                            Auxiliary/status display only -- not for stimulus timing.");
    eprintln!("      --zmq-port <N>        ZMQ REP server port (default: 5555)");
    eprintln!("      --no-web              Disable the embedded web control surface");
    eprintln!("      --web-port <N>        Web UI HTTP/WebSocket port (default: 8080)");
    eprintln!("      --overlay-scale <N>   Scale factor for the egui overlay UI (default: 1.0)");
    eprintln!("      --preferred-clock-source <S>");
    eprintln!("                            Force a DRM/console vblank clock (auto, drm_vblank,");
    eprintln!("                            vk_display_control, present_wait, gpu_completion);");
    eprintln!("                            overrides rig-config's [display] clock (default: auto)");
    eprintln!("  -v, --verbose             Enable debug logging (overridden by RUST_LOG)");
    eprintln!("      --rig-config <path>   Rig config (default: {})", vstimd::rig_config::DEFAULT_PATH);
    eprintln!("      --config <path>       Load stim-config file at startup");
    eprintln!("      --config-dir <path>   Directory for named stim-config files");
    eprintln!("                            (default: /var/lib/braemons/vstimd, else");
    eprintln!("                            ~/.local/braemons/vstimd if not writable)");

    eprintln!("  -V, --version             Show version and build info (features, target, date)");
    eprintln!("  -h, --help                Show this help message");
    eprintln!();
    eprintln!("Render target resolution (highest priority first):");
    eprintln!("  1. --null / --evdi on the command line");
    eprintln!("  2. rig-config's [display] backend (\"drm\", \"desktop\", \"null\", \"evdi\")");
    eprintln!("  3. auto-detect: Windows/macOS or DISPLAY/WAYLAND_DISPLAY set -> desktop (winit);");
    eprintln!("     otherwise console -> DRM/KMS if HDMI/DP is connected, else a connected");
    eprintln!("     DisplayLink (evdi) output if present, else DRM/KMS");
}
