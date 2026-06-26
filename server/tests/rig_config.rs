use vstimd::rig_config::{self, RigConfig};

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
