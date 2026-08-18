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

## Four ways to do it

Saving and loading is the same operation on the same files whichever route you
take, so use whichever is in front of you.

| From | How | Good for |
|---|---|---|
| **The on-device overlay** | The **Config** panel (++f6++) lists the configs in the config directory; load, save, and overwrite from there | a rig with a keyboard and no client attached |
| **The web control UI** | The config section of the browser UI served by the device — [Web control UI](../client/web.md) | setting a rig up from a laptop or a phone on the same network, with no software installed |
| **The command-line client** | `vstimd-client config list` / `save NAME` / `load NAME` / `get` / `upload NAME FILE` — [Command-line client](../client/cli.md) | scripts, deployment, CI, and anything you want in a shell history |
| **The Python client** | `conn.config.*` — see below | building a scene programmatically and persisting it in the same script |

```console
$ vstimd-client config list
$ vstimd-client config save center_target -f
$ vstimd-client config load center_target
```

All four write the same `.config.json` files into the same directory, so a scene
saved from the overlay loads from Python, and a config uploaded from CI shows up
in the web UI. A fifth route exists on a rig with the
[Samba shares](../operations/appliance-setup.md#6-admin-access-ssh-optional-samba)
installed: the config directory under `/var/lib/braemons` is exported as a
network share — browsable by anyone on the LAN, writable with an admin account —
so the `.config.json` files can be copied on and off the device from a lab
Windows or macOS machine like any other files.

## From a client (`config` namespace)

```python
with Connection("tcp://stimulus-pc:5555") as conn:
    # Build a scene however you like…
    conn.stimuli.shapes.create_rect(
        position=Vec2(0, 0),
        params=RectParams(
            width=200,
            height=100,
            appearance=ShapeAppearance(fill_color=Color(1, 0, 0)),
        ),
    )

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

That section lives in the rig config, `/etc/braemons/vstimd-rig-config.toml`
(the shipped template with every key documented is
`server/config/default-rig-config.toml`). On a rig with the
[Samba shares](../operations/appliance-setup.md#6-admin-access-ssh-optional-samba)
installed, `/etc/braemons` is a network share, so pointing a rig at a different
startup scene is a file edit from your own machine — no SSH session and no
`vstimd-client` needed. Restart `vstimd.service` for it to take effect.

An explicit `--config <path>` CLI flag overrides `[startup] load_config`. A missing
last-session slot on first boot is a no-op (the rig starts with an empty scene), not
an error. See [Deployment](../operations/deployment.md) for the wider boot flow, and
[Gratings, triggers & a saved config](../tutorials/gratings-triggers-config.md)
for a worked example that ends with exactly this step.

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

- **[How vstimd works](how-vstimd-works.md)** — where config files fit as a setup
  API alongside the command API, ahead of trigger-driven execution.
- **[Deferred mode](deferred-mode.md)** — atomic frame flips for coordinated changes.
- **[Build the demos yourself](../tutorials/index.md)** — six scripts that each
  end by saving the scene they built.
