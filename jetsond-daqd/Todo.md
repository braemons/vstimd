# jetsond-daqd — TODO

## Missing features

### Output pulse support
`VtlSegment::output_set_pulse` / `drain_output_pulse` are not handled.
vstimd can fire one-shot TTL pulses (e.g. frame-sync markers) via this field,
but the output loop currently only mirrors sustained `output_state` levels.
The output loop needs a second check: drain pulse bits → drive pin high → short
delay → drive pin low.

### Input watcher restart
If a GPIO event handle returns an I/O error the input thread exits permanently.
The main thread should detect the dead `JoinHandle` and re-spawn it with
back-off.

### VTL segment reconnect
If vstimd restarts it unlinks and recreates `/dev/shm/vstimd_vtl`.  All open
`VtlClient` handles become stale (they map the old anonymous segment).  The
daemon needs to detect the stale mapping (e.g. re-check magic periodically) and
re-open with `open_vtl_with_retry`.

### Graceful shutdown
No SIGTERM/SIGINT handler.  The daemon should drain output pins to 0 on exit
so downstream equipment isn't left with a stale high signal.

### Output timing accuracy
The 1 ms `thread::sleep` drifts under load (std sleep is a minimum, not exact).
For tighter output timing consider:
- `timerfd_create(CLOCK_MONOTONIC)` with a fixed interval
- A higher-priority thread (SCHED_FIFO) for the output loop
- Accepting that outputs are inherently coarser than the interrupt-driven inputs
  and documenting the expected worst-case latency

### Per-line GPIO chip
All lines must currently share the same `[gpio] chip`.  Some Jetson header pins
are on `gpiochip1` (tegra234-gpio-aon).  Config should allow per-line chip
override:
```toml
[[outputs]]
name      = "stim_onset"
gpio_chip = "/dev/gpiochip1"   # override
gpio_line = 5
```

### VTL named-line registration
The daemon should write its configured lines into the VTL names table
(`write_named_line` / `set_n_named_lines`) so that `gpioinfo`-style tooling and
vstimd's ZMQ `ListVtlLines` command can discover them by name.

### Debian packaging scripts
`Cargo.toml` references `packaging/debian/` maintainer scripts (postinst,
prerm) but those files do not exist yet.  At minimum `postinst` should create
the config directory and set GPIO group permissions.

## Known gaps in tests

### Hardware pin mapping table
`tests/loopback.rs` documents GPIO line *offsets* but not which 40-pin header
pins they correspond to on Jetson Orin Nano.  Add a mapping table so it's easy
to pick a safe loopback pair without consulting the datasheet.

### Output pulse test
No test exercises `output_set_pulse` / `poll_outputs_once` for one-shot pulse
behaviour (drive high → delay → drive low).

### Latency measurement test
Add a hardware test that timestamps the GPIO event (`LineEvent::timestamp`) and
compares it to the `Instant` before `set_value` to characterise round-trip
latency of the loopback path.

### CI for hw-tests
Hardware tests are manually opt-in only.  If a Jetson with loopback wiring is
available in CI, wire up `--features hw-tests` with the appropriate env vars.
