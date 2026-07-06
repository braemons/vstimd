# Saving & loading scenes

A whole scene — every stimulus, every animation, the background, **and** the VTL
line names (the I/O map) — can be saved to and loaded from a named **config** on the
device. This lets a rig boot into a known stimulus configuration with **no client
connected at all**, and lets you version and share scenes as plain JSON.

Configs are stored as `.config.json` files in the server's config directory
(set with `--config-dir`; the default is baked into the rig's deployment). You never
handle paths from a client — configs are addressed by a bare **name**.

## What a config contains

| Included | Not included |
|---|---|
| All stimuli (geometry, colour, enabled state, draw order) | Live animation *state* (armed/running) — animations are saved as definitions |
| All animation definitions | Anything outside the scene (rig-config, network settings) |
| Background colour | |
| VTL line **names** (the I/O map) | VTL live line levels |

## From a client (`config` namespace)

```python
with Connection("tcp://stimulus-pc:5555") as conn:
    # Build a scene however you like…
    conn.stimuli.shapes.create_rect(pos=Vec2(0, 0), width=200, height=100,
                                    color=Color(1, 0, 0))

    # …then save it under a name on the device:
    conn.config.save("center_target")             # → center_target.config.json

    # Later, list and load:
    print(conn.config.list_configs())             # ['center_target', …]
    conn.config.load("center_target")             # clears the scene, then loads
```

### Additive load

By default `load` **clears** the scene first. Pass `additive=True` to merge the
config's stimuli and animations into the current scene instead — handles are remapped
to avoid collisions. The I/O config (VTL names) is always fully replaced.

```python
conn.config.load("distractors", additive=True)    # add on top of what's shown
```

### Retrieve, upload, and round-tripping JSON

`retrieve` returns the current scene as a JSON string (the same format as the
`.config.json` files), and `upload` sends a JSON string back to the device under a
name. This lets you inspect, edit, version-control, or template configs off-device:

```python
text = conn.config.retrieve()                      # current scene → JSON string
open("my_scene.json", "w").write(text)

# …edit / commit / template it, then push it back:
conn.config.upload("my_scene", text, overwrite=True, apply_now=True)
```

`upload` raises `ConfigAlreadyExistsError` if the name exists and `overwrite=False`.
With `apply_now=True` the config is applied immediately after saving (honouring
`additive`). `save(name)` is just a convenience wrapper around `retrieve` + `upload`.

## Booting a rig into a fixed scene

Because a config carries the whole scene *and* the VTL line map, a deployed rig can
come up showing a known configuration with no experiment PC attached — useful for
home-cage training or a self-contained demo. Save the scene once from any client,
then have the device load it at startup via the rig-config `[startup]` section:

```toml
[startup]
# Load a named config from the config dir at boot. The literal "last" loads the
# auto-saved last-session slot (see save_on_quit). Omit for an empty scene.
load_config  = "center_target"

# On graceful shutdown, save the current scene: overwrite the last-session slot
# AND write a timestamped archive for history (see below).
save_on_quit = false
```

An explicit `--config <path>` CLI flag overrides `[startup] load_config`. A missing
last-session slot on first boot is a no-op (the rig starts with an empty scene), not
an error. See the **Config** panel (F6) of the on-device overlay, or the web control
UI, for loading configs interactively, and
[Deployment](../operations/deployment.md) for the wider boot flow.

### Where configs live, and save-on-quit archives

Named configs are stored in `--config-dir`. On a deployed rig this defaults to
`/var/lib/braemons/vstimd` (created by the packaged systemd unit's `StateDirectory`);
if that directory is not writable — for example a non-root development run — vstimd
falls back to `~/.local/braemons/vstimd`, then the current directory, logging which it
chose.

With `save_on_quit = true`, each graceful shutdown writes **two** files:

- `vstimd__last_session.config.json` — overwritten every quit; restored by
  `load_config = "last"`.
- `vstimd_<YYYYMMDDTHHMMSSZ>.config.json` — a timestamped archive (UTC), so you keep
  a history of what each session ended with.

Archives are never pruned automatically; vstimd logs a warning once more than 500
accumulate in one directory, as a nudge to clean up.

## See also

- **[Choosing an API path](../tutorial/index.md)** — where config files fit among
  the command and VTL paths.
- **[Deferred mode](deferred-mode.md)** — atomic frame flips for coordinated changes.
