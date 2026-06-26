use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Config {
    #[serde(default)]
    pub vtl:  VtlConfig,
    #[serde(default)]
    pub gpio: GpioConfig,
    #[serde(default)]
    pub outputs: Vec<OutputLine>,
    #[serde(default)]
    pub inputs: Vec<InputLine>,
    #[serde(default)]
    pub scheduling: SchedulingConfig,
}

#[derive(Deserialize, Debug)]
pub struct VtlConfig {
    /// POSIX shared-memory name (must start with `/`).
    #[serde(default = "VtlConfig::default_shm_name")]
    pub shm_name: String,
}

impl VtlConfig {
    fn default_shm_name() -> String { "/vstimd_vtl".into() }
}

impl Default for VtlConfig {
    fn default() -> Self { Self { shm_name: Self::default_shm_name() } }
}

#[derive(Deserialize, Debug, Clone)]
pub struct GpioConfig {
    /// Linux GPIO character device path.
    #[serde(default = "GpioConfig::default_chip")]
    pub chip: String,
}

impl GpioConfig {
    fn default_chip() -> String { "/dev/gpiochip0".into() }
}

impl Default for GpioConfig {
    fn default() -> Self { Self { chip: Self::default_chip() } }
}

/// Thread scheduling options.
///
/// All fields are optional.  Omit a field to use the built-in default.
/// `*_cpu_core` is accepted by the parser but not yet applied.
#[derive(Deserialize, Debug, Default)]
#[allow(dead_code)]
pub struct SchedulingConfig {
    /// SCHED_FIFO priority for the output thread (1–99).  Default: 60.
    pub output_rt_prio: Option<i32>,
    /// CPU core to pin the output thread to.  Not yet applied.
    pub output_cpu_core: Option<usize>,
    /// SCHED_FIFO priority for each input watcher thread (1–99).  Default: 50.
    pub input_rt_prio: Option<i32>,
    /// CPU core to pin input watcher threads to.  Not yet applied.
    pub input_cpu_core: Option<usize>,
}

/// Maps one VTL output bit → one GPIO output pin.
///
/// vstimd writes `output_state`; this daemon drives the pin to match.
#[derive(Deserialize, Debug, Clone)]
pub struct OutputLine {
    /// Must match the name registered in the VTL names table by vstimd.
    pub name: String,
    pub vtl_bank: u8,
    pub vtl_bit: u8,
    /// GPIO line offset within the chip (not the 40-pin header number).
    pub gpio_line: u32,
}

/// Maps one GPIO input pin → one VTL input bit + rise/fall latches.
///
/// This daemon watches for edges and writes `input_state` and latches.
#[derive(Deserialize, Debug, Clone)]
pub struct InputLine {
    pub name: String,
    pub vtl_bank: u8,
    pub vtl_bit: u8,
    pub gpio_line: u32,
    pub edge: Edge,
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    Rising,
    Falling,
    Both,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_config() {
        let raw = r#"
            [vtl]
            shm_name = "/vstimd_vtl"

            [gpio]
            chip = "/dev/gpiochip0"
        "#;
        let cfg: Config = toml::from_str(raw).unwrap();
        assert_eq!(cfg.vtl.shm_name, "/vstimd_vtl");
        assert!(cfg.outputs.is_empty());
        assert!(cfg.inputs.is_empty());
    }

    #[test]
    fn parse_full_config() {
        let raw = r#"
            [vtl]
            shm_name = "/vstimd_vtl"

            [gpio]
            chip = "/dev/gpiochip0"

            [[outputs]]
            name      = "stim_onset"
            vtl_bank  = 0
            vtl_bit   = 0
            gpio_line = 79

            [[inputs]]
            name      = "scanner_trigger"
            vtl_bank  = 0
            vtl_bit   = 0
            gpio_line = 77
            edge      = "rising"
        "#;
        let cfg: Config = toml::from_str(raw).unwrap();
        assert_eq!(cfg.outputs.len(), 1);
        assert_eq!(cfg.outputs[0].gpio_line, 79);
        assert_eq!(cfg.inputs[0].edge, Edge::Rising);
    }

    #[test]
    fn reject_unknown_edge() {
        let raw = r#"
            [vtl]
            shm_name = "/vstimd_vtl"
            [gpio]
            chip = "/dev/gpiochip0"
            [[inputs]]
            name = "x"
            vtl_bank = 0
            vtl_bit = 0
            gpio_line = 1
            edge = "bogus"
        "#;
        assert!(toml::from_str::<Config>(raw).is_err());
    }
}
