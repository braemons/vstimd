use vstimd::render::RenderTargetPref;
use vstimd::rig_config::{self, RigConfig, StartupLoad};

fn parse(toml: &str) -> RigConfig {
    toml::from_str(toml).expect("parse rig-config")
}

#[test]
fn defaults_when_empty() {
    let cfg = parse("");
    assert_eq!(cfg.vtl.shm_name, "/vstimd_vtl");
    assert_eq!(cfg.vtl.num_input_banks,  1);
    assert_eq!(cfg.vtl.num_output_banks, 1);
    assert!(cfg.vtl.vblank.is_none());
    assert!(cfg.display.width.is_none());
}

#[test]
fn vtl_section_parsed() {
    let cfg = parse(r#"
[vtl]
shm_name         = "/my_vtl"
num_input_banks  = 2
num_output_banks = 1
"#);
    assert_eq!(cfg.vtl.shm_name, "/my_vtl");
    assert_eq!(cfg.vtl.num_input_banks, 2);
}

#[test]
fn vblank_bit_parsed() {
    let cfg = parse(r#"
[vtl.vblank]
bank = 0
bit  = 63
"#);
    let vb = cfg.vtl.vblank.expect("vblank should be Some");
    assert_eq!(vb.bank, 0);
    assert_eq!(vb.bit, 63);
}

#[test]
fn display_section_parsed() {
    let cfg = parse(r#"
[display]
width      = 1920
height     = 1080
refresh_hz = 60.0
"#);
    assert_eq!(cfg.display.width,      Some(1920));
    assert_eq!(cfg.display.height,     Some(1080));
    assert_eq!(cfg.display.refresh_hz, Some(60.0));
}

#[test]
fn backend_defaults_to_auto() {
    let cfg = parse("");
    assert!(cfg.display.backend.is_none());
}

#[test]
fn backend_evdi_parsed() {
    let cfg = parse(r#"
[display]
backend = "evdi"
"#);
    assert_eq!(cfg.display.backend, Some(RenderTargetPref::Evdi));
}

#[test]
fn backend_auto_keyword_is_none() {
    let cfg = parse(r#"
[display]
backend = "auto"
"#);
    assert!(cfg.display.backend.is_none());
}

#[test]
fn backend_invalid_value_rejected() {
    let result: Result<RigConfig, _> = toml::from_str(
        r#"
[display]
backend = "not_a_backend"
"#,
    );
    assert!(result.is_err());
}

#[test]
fn startup_defaults_to_no_load_and_no_save() {
    let cfg = parse("");
    assert!(cfg.startup.load_config.is_none());
    assert!(!cfg.startup.save_on_quit);
}

#[test]
fn startup_named_config_parsed() {
    let cfg = parse(r#"
[startup]
load_config  = "center_target"
save_on_quit = true
"#);
    assert_eq!(
        cfg.startup.load_config,
        Some(StartupLoad::Named("center_target".into()))
    );
    assert!(cfg.startup.save_on_quit);
}

#[test]
fn startup_last_keyword_maps_to_last_session() {
    // Case-insensitive so "last", "Last", "LAST" all work.
    for kw in ["last", "Last", "LAST"] {
        let cfg = parse(&format!("[startup]\nload_config = \"{kw}\"\n"));
        assert_eq!(cfg.startup.load_config, Some(StartupLoad::LastSession));
    }
}

#[test]
fn startup_empty_load_config_is_none() {
    let cfg = parse("[startup]\nload_config = \"\"\n");
    assert!(cfg.startup.load_config.is_none());
}

#[test]
fn startup_rejects_unknown_field() {
    let err = toml::from_str::<RigConfig>("[startup]\nbogus = 1\n");
    assert!(err.is_err(), "deny_unknown_fields should reject bogus keys");
}

#[test]
fn load_returns_default_when_absent() {
    let cfg = rig_config::load("/nonexistent/path/rig-config.toml")
        .expect("missing file should not be an error");
    assert_eq!(cfg.vtl.shm_name, "/vstimd_vtl");
}

/// Every bundled `.toml` under `config/` (the default plus each board example)
/// must load cleanly through the real loader — this guards against a typo or a
/// stale key (e.g. after a schema change) slipping into a shipped example.
/// Globbing the directory means new examples are covered automatically.
#[test]
fn example_configs_parse_cleanly() {
    let dir = std::path::Path::new("config");
    let mut checked = 0;
    for entry in std::fs::read_dir(dir).expect("read config dir") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        // `load` exercises the real startup path (read + parse + deny_unknown_fields).
        rig_config::load(path.to_str().unwrap())
            .unwrap_or_else(|e| panic!("load {path:?}: {e}"));
        checked += 1;
    }
    assert!(
        checked >= 4,
        "expected at least the default + 3 board example configs, checked {checked}"
    );
}

/// The board examples all keep `[startup]` commented out, so they parse to the
/// no-load / no-save defaults — a rig using an example as-is won't unexpectedly
/// load or overwrite a scene until the operator opts in.
#[test]
fn example_configs_default_startup_to_off() {
    for name in ["jetson-orin-nano", "raspberry-pi-5", "raspberry-pi-4"] {
        let cfg = rig_config::load(&format!("config/{name}.toml"))
            .unwrap_or_else(|e| panic!("load {name}: {e}"));
        assert!(cfg.startup.load_config.is_none(), "{name}: startup.load_config");
        assert!(!cfg.startup.save_on_quit, "{name}: startup.save_on_quit");
    }
}
