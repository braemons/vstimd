//! Reference producer: a synthetic wheel publishing a cumulative tick count.
//!
//!     cargo run -p vinput --example wheel_writer -- /vstimd_wheel 1000
//!
//! Writes `total += 1` at the given rate (Hz, default 1000) until killed. A real
//! wheel reader replaces the ramp with counts from its serial port — and, like
//! this, publishes the running total, never a per-write delta.

use vinput::{AxisDesc, Semantic, VinputOwner};

fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    let name = args.next().unwrap_or_else(|| "/vstimd_wheel".into());
    let hz: f64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1000.0);

    let mut dev = VinputOwner::create(&name, &[AxisDesc::new("distance", Semantic::Cumulative, 1.0)])?;
    println!("writing {name} at {hz} Hz — Ctrl+C to stop");
    let period = std::time::Duration::from_secs_f64(1.0 / hz);
    let mut total = 0.0f64;
    loop {
        total += 1.0;
        dev.write(&[total]);
        std::thread::sleep(period);
    }
}
