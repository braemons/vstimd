# Demo configs

Ordinary stim-configs, in exactly the format `config save` writes. They are
compiled into the binary (`io_config::DEMO_CONFIGS`) and installed into the
config dir at startup, so a dev checkout, a `.deb` install and the Raspberry Pi
image all offer the same set.

Editing one of these files therefore ships a new version to every rig on the
next start — but only to demo files the server installed and the operator never
touched, tracked by fingerprint in a `.vstimd_demo_seed` sidecar. An operator's
edited copy is left alone forever (they can delete it to get the update back).

There is deliberately no demo-specific command or load path: a demo is loaded,
edited and re-saved through the same `config load` / `config save` a user config
goes through. See `docs/getting-started/demos.md` for what each one does.

## Editing them

Two ways, both fine:

- Load a demo on a running server, change it from the overlay / web UI / a
  client, `config save` it, and copy the resulting
  `vstimd_demo_*.config.json` back over the file here.
- Edit the JSON directly.

Either way run `cargo test -p vstimd --test demo_configs` afterwards. Those
tests load every demo the way the server does, assert the behaviour each demo is
supposed to show (which pins, which durations, which final actions), and fail if
a file no longer round-trips — which is what catches a key that a scene-model
rename left behind.

## Trigger lines

The trigger demos address VTL bits by the bit numbers the **Raspberry Pi 5**
`gpiochip-daqd` example (`gpiochip-daqd/config/raspberry-pi-5_in16_out4.toml`,
installed by the SD-card image) maps to 40-pin header pins — `vtl_bit` == header
pin number. Keep the two in sync: a demo that names `in_pin11` must use bit 11,
or it will wait on a pin nobody wired.
