//! Read or write a segment from the command line — the Rust half of the
//! cross-language layout tests in the Python client.
//!
//!     segment_tool read  /name          prints "n_axes semantic... | values..."
//!     segment_tool write /name MS v...  creates (all rate axes), writes, lives MS ms
//!     segment_tool check /name MS       reads for MS ms; fails if any sample's values differ

use vinput::{AxisDesc, Semantic, VinputClient, VinputOwner};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [cmd, name] if cmd == "read" => {
            let c = VinputClient::open(name).unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(1)
            });
            let mut buf = [0.0f64; vinput::MAX_AXES];
            let n = c.read_into(&mut buf).unwrap_or_else(|_| {
                eprintln!("torn");
                std::process::exit(2)
            });
            let axes: Vec<String> =
                c.axes().iter().map(|a| format!("{}:{:?}:{}", a.name, a.semantic, a.scale)).collect();
            let values: Vec<String> = buf[..n].iter().map(|v| v.to_string()).collect();
            println!(
                "{} {} | {} | count={} age_ms={}",
                c.device_name(),
                axes.join(","),
                values.join(","),
                c.write_count(),
                c.age_ns() / 1_000_000
            );
        }
        [cmd, name, ms, values @ ..] if cmd == "write" => {
            let values: Vec<f64> = values.iter().map(|v| v.parse().expect("number")).collect();
            let axes: Vec<AxisDesc> =
                (0..values.len()).map(|i| AxisDesc::new(format!("a{i}"), Semantic::Rate, 1.0)).collect();
            let mut o = VinputOwner::create(name, &axes).expect("create");
            o.write(&values);
            println!("ready");
            std::thread::sleep(std::time::Duration::from_millis(ms.parse().expect("ms")));
        }
        [cmd, name, ms] if cmd == "check" => {
            let c = VinputClient::open(name).expect("open");
            let n = c.n_axes();
            let end = std::time::Instant::now() + std::time::Duration::from_millis(ms.parse().expect("ms"));
            let (mut ok, mut torn) = (0u64, 0u64);
            let mut buf = [0.0f64; vinput::MAX_AXES];
            while std::time::Instant::now() < end {
                match c.read_into(&mut buf[..n]) {
                    Ok(_) => {
                        if buf[..n].iter().any(|v| *v != buf[0]) {
                            println!("MIXED {:?}", &buf[..n]);
                            std::process::exit(3);
                        }
                        ok += 1;
                    }
                    Err(_) => torn += 1,
                }
            }
            println!("ok={ok} torn={torn}");
        }
        _ => {
            eprintln!("usage: segment_tool read NAME | write NAME MS VALUES...");
            std::process::exit(64);
        }
    }
}
