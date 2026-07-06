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

#[test]
fn example_configs_parse_cleanly() {
    let examples = [
        "config/jetson-orin-nano.toml",
        "config/raspberry-pi-5.toml",
        "config/raspberry-pi-4.toml",
    ];
    for path in &examples {
        let raw = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("read {path}: {e}"));
        let _cfg: RigConfig = toml::from_str(&raw)
            .unwrap_or_else(|e| panic!("parse {path}: {e}"));
    }
}
