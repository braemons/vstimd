use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use vinput::{AxisDesc, Semantic, VinputClient, VinputOwner};

fn unique_name(tag: &str) -> String {
    format!("/vinput_test_{}_{tag}", std::process::id())
}

fn two_axes() -> [AxisDesc; 2] {
    [
        AxisDesc::new("x", Semantic::Absolute, 1.0),
        AxisDesc::new("y", Semantic::Absolute, 1.0).with_deadzone(0.5),
    ]
}

#[test]
fn round_trip_and_description() {
    let name = unique_name("roundtrip");
    let mut owner = VinputOwner::create(&name, &two_axes()).unwrap();
    owner.write(&[1.0, 2.0]);
    let client = VinputClient::open(&name).unwrap();
    let mut buf = [0.0; 4];
    assert_eq!(client.read_into(&mut buf), Ok(2));
    assert_eq!(&buf[..2], &[1.0, 2.0]);
    assert_eq!(client.n_axes(), 2);
    assert_eq!(client.axes(), two_axes().to_vec());
    assert_eq!(client.device_name(), name);
    assert_eq!(client.write_count(), 1);
}

#[test]
fn opening_a_missing_segment_fails_and_creates_nothing() {
    let name = unique_name("missing");
    assert!(VinputClient::open(&name).is_err());
    assert!(VinputClient::open(&name).is_err(), "the failed open must not have created it");
}

#[test]
fn a_bad_axis_count_is_refused_at_create() {
    assert!(VinputOwner::create(&unique_name("zero"), &[]).is_err());
}

#[test]
fn reads_are_coherent_under_a_hammering_writer() {
    let name = unique_name("seqlock");
    let mut owner = VinputOwner::create(&name, &two_axes()).unwrap();
    owner.write(&[0.0, 0.0]);
    let client = VinputClient::open(&name).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let writer = {
        let stop = stop.clone();
        std::thread::spawn(move || {
            let mut n = 0.0;
            while !stop.load(Ordering::Relaxed) {
                n += 1.0;
                owner.write(&[n, n]);
                // Far faster than any device, but not a loop with no gap at
                // all, which no reader could ever catch between writes.
                std::thread::yield_now();
            }
            owner
        })
    };
    let (mut ok, mut torn) = (0u64, 0u64);
    let deadline = std::time::Instant::now() + Duration::from_millis(300);
    let mut buf = [0.0; 2];
    while std::time::Instant::now() < deadline {
        match client.read_into(&mut buf) {
            Ok(_) => {
                assert_eq!(buf[0], buf[1], "a read mixed two writes");
                ok += 1;
            }
            Err(_) => torn += 1,
        }
    }
    stop.store(true, Ordering::Relaxed);
    drop(writer.join().unwrap());
    assert!(ok > 1000, "only {ok} coherent reads ({torn} torn)");
}

#[test]
fn age_grows_once_the_writer_stops() {
    let name = unique_name("age");
    let mut owner = VinputOwner::create(&name, &two_axes()).unwrap();
    owner.write(&[1.0, 1.0]);
    let client = VinputClient::open(&name).unwrap();
    let young = client.age_ns();
    std::thread::sleep(Duration::from_millis(30));
    assert!(client.age_ns() >= young + 25_000_000);
    owner.write(&[2.0, 2.0]);
    assert!(client.age_ns() < 25_000_000, "a write refreshes the heartbeat");
}

#[test]
fn cumulative_deltas_lose_and_duplicate_nothing() {
    let name = unique_name("cumulative");
    let mut owner =
        VinputOwner::create(&name, &[AxisDesc::new("ticks", Semantic::Cumulative, 1.0)]).unwrap();
    owner.write(&[0.0]);
    let client = VinputClient::open(&name).unwrap();
    const TICKS: u32 = 5_000;
    let writer = std::thread::spawn(move || {
        let mut total = 0.0;
        for _ in 0..TICKS {
            total += 1.0; // ~10 kHz
            owner.write(&[total]);
            std::thread::sleep(Duration::from_micros(100));
        }
        owner
    });
    let (mut last, mut sum) = (0.0f64, 0.0f64);
    let mut buf = [0.0];
    let mut read = |last: &mut f64, sum: &mut f64| {
        if client.read_into(&mut buf).is_ok() {
            *sum += buf[0] - *last;
            *last = buf[0];
        }
    };
    while !writer.is_finished() {
        read(&mut last, &mut sum);
        std::thread::sleep(Duration::from_millis(16)); // 60 Hz
    }
    let owner = writer.join().unwrap();
    read(&mut last, &mut sum);
    assert_eq!(sum, f64::from(TICKS));
    drop(owner);
}

/// The producer is a separate process: this test binary re-run with an env var
/// selects `child_writer`, which creates the segment and writes.
#[test]
fn a_second_process_reads_what_the_first_writes() {
    let name = unique_name("process");
    let ready = format!("{}.ready", std::env::temp_dir().join(name.trim_start_matches('/')).display());
    let _ = std::fs::remove_file(&ready);
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_writer", "--nocapture"])
        .env("VINPUT_CHILD_SEGMENT", &name)
        .env("VINPUT_CHILD_READY", &ready)
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !std::path::Path::new(&ready).exists() {
        assert!(std::time::Instant::now() < deadline, "child never became ready");
        std::thread::sleep(Duration::from_millis(10));
    }
    let client = VinputClient::open(&name).unwrap();
    let mut buf = [0.0; 2];
    assert_eq!(client.read_into(&mut buf), Ok(2));
    assert_eq!(buf, [42.0, -7.5]);
    child.kill().ok();
    child.wait().ok();
    let _ = std::fs::remove_file(&ready);
}

#[test]
fn child_writer() {
    let (Ok(name), Ok(ready)) = (std::env::var("VINPUT_CHILD_SEGMENT"), std::env::var("VINPUT_CHILD_READY")) else {
        return; // not running as the child
    };
    let mut owner = VinputOwner::create(&name, &two_axes()).unwrap();
    owner.write(&[42.0, -7.5]);
    std::fs::write(&ready, b"").unwrap();
    std::thread::sleep(Duration::from_secs(30)); // killed by the parent
}
