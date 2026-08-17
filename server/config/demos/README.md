# Demo configs

Ordinary scene-configs, in exactly the format `config save` writes. They are
compiled into the binary (`scene_config_file::DEMO_CONFIGS`) and installed into the
config dir at startup, so a dev checkout, a `.deb` install and the Raspberry Pi
image all offer the same set.

There is deliberately no demo-specific command or load path: a demo is loaded,
edited and re-saved through the same `config load` / `config save` a user config
goes through. See `docs/getting-started/demos.md` for what each one does.

## What happens to a demo file already on the rig

At startup the server compares each demo on disk against the shipped copy and
against a fingerprint of what it last wrote (`.vstimd_demo_seed` in the config
dir). Exactly one of these applies per demo:

| On disk | Action | Logged as |
|---|---|---|
| absent | written, fingerprint recorded | `installed demo configs` |
| identical to the shipped copy | left as-is, (re)stamped | (nothing) |
| unchanged since the server wrote it, but the shipped copy has changed | **replaced** | `updated demo configs` |
| edited since the server wrote it | left alone, permanently | `kept local demo configs` |
| present with no fingerprint on record | left alone, permanently | `kept local demo configs` |

Two consequences worth being explicit about:

- **Editing a file in this directory ships it to every rig on the next start —
  but only to rigs whose copy is untouched.** A rig where someone edited that
  demo keeps their version and stops tracking the shipped one until they delete
  it.
- **A file identical to the shipped copy is adopted**, even if this server never
  wrote it, so restoring a demo by hand puts it back in the refresh path.
- Config dirs predating the fingerprint sidecar have no record, so their demos
  are treated as operator files. Delete one to opt back in.

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
