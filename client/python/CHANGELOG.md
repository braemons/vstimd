# Changelog

All notable changes to `vstimd-client` are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[semantic versioning](https://semver.org/spec/v2.0.0.html). The client is
versioned independently of the vstimd server.

## [Unreleased]

### Changed — breaking

The API-consistency pass before the first release. No aliases are kept: the
server and the client move together, and nothing has shipped yet.

- **Sizes are full extents everywhere.** The command API always took full
  `width`/`height`; the saved config JSON stored half-extents (`size` for rects
  and gratings, `radii` for ellipses). Config now records the same numbers the
  commands take, so a config can be read straight off as the arguments to pass.
  Config format 2 → 3; a v2 file is rejected rather than silently loaded at half
  size.
- **`delete_all` splits into three.** `conn.system.clear_stimuli()`,
  `clear_animations()` and `clear_all()` — an animation outlives the stimuli it
  drives, so "clear everything" needed to be sayable in one call. Scene-wide
  settings (background, default colours, photodiode, VTL names) survive all
  three.
- **Opacity is a shared property.** `conn.stimuli.set_alpha(handle, opacity)`
  now works on every stimulus type, not just shapes, and multiplies the alpha of
  whatever colours the stimulus carries instead of overwriting the fill's. A
  half-transparent fill under an opaque outline keeps that relationship at every
  opacity. `conn.stimuli.grating.set_opacity` and `create_grating(opacity=...)`
  are gone — use `set_alpha`. The PsychoPy shims send it as its own command, so
  `fillColor`'s alpha and `opacity` are independent.
- **`query()` reports honest fields.** `StimulusInfo.fill_color`,
  `outline_color`, `outline_width` and `draw_mode` were synthesised per type — a
  grating reported its fore colour as "fill" with an outline width of 0 that
  meant nothing. They are now properties reading the shape's own
  `params.appearance`, and `None` for gratings and text, which have no such
  thing. `pos` and `orientation` come from the 2-D placement and are typed
  optional, ready for stimuli placed in 3-D space.
- **Animations name what they drive.** `CreateAnimationRequest.stimuli` becomes
  a `target` oneof on the wire. The Python signatures are unchanged — you still
  pass a handle or a list — but a saved config records
  `"target": {"kind": "Stimuli", "handles": [...]}`.
- **Text field names agree across the three places they appear.**
  `CreateTextRequest.size` → `box_size` (matching the config field and the
  `box_width`/`box_height` arguments), and its `color` → `text_color` (matching
  what a query already called it). `TextParams.size` → `TextParams.box_size`.

## [0.1.0rc3] — 2026-08-13

### Added

- `vstimd-client` with no command now prints its commands, grouped by what they
  are for, along with how to point it at a server and a handful of examples —
  instead of a one-line argparse complaint.
- `--address` accepts what people actually type: the scheme and port are filled
  in when missing, so `-a 10.0.1.42`, `-a 10.0.1.42:5555` and
  `-a tcp://10.0.1.42:5555` all name the same server. Bare IPv6 literals are
  bracketed automatically; `ipc://` and `inproc://` endpoints pass through
  unchanged.
- `VSTIMD_TRACEBACK=1` restores the full traceback for bug reports.
- With no `--address`, `--host` or `$VSTIMD_ADDRESS`, the client now browses for
  a server instead of assuming `tcp://localhost:5555`. One rig found is used
  and announced on stderr; several are listed and prompted for; none falls back
  to `tcp://localhost:5555` as before. On a bench with a single rig, commands no
  longer need an address at all.
- `--non-interactive` refuses to prompt, listing the candidates and exiting `2`
  instead. A non-terminal stdin behaves the same way, so cron jobs and CI steps
  cannot hang on the selector.
- Server errors carry structured context. Every `VstimdError` now has `code`
  (the `ErrorCode` the server returned), `detail` (its message), `command`
  (which request failed, e.g. `set_position`) and `handle` (the stimulus it
  addressed, or `None`). `str(exc)` includes them, so an uncaught error reads
  `no such stimulus (set_position, handle 7)`.
- `StimulusError` and `ConfigError` group the exceptions that share a cause, so
  `except ConfigError` catches all five config failures without listing them.
- `ProtocolError` for a reply that cannot be decoded, or that arrives with no
  result code set — previously the first crashed with a raw protobuf
  `DecodeError` and the second was treated as success.

### Changed

- **Exit codes now say what went wrong**: `3` unreachable, `4` timed out, `5`
  server error, `6` not found, `7` no mDNS backend, on top of the existing `0`,
  `1`, `2` and `130`. Previously almost every failure exited `1`. Scripts that
  test for a specific non-zero code need updating; scripts that test for
  success do not.
  - `discover` finding nothing now exits `6`, was `1`.
  - `discover` with no mDNS backend now exits `7`, was `2` — which argparse
    also uses for command-line errors, so the two were indistinguishable.
  - A request that times out now exits `4`, was `1`.
  - `shutdown` refusing to prompt on a non-interactive stdin now exits `2`, was
    `1`, since the fix is to pass `--yes`.

### Fixed

- The CLI no longer prints a traceback for anything a user can cause. A
  malformed address, an unreachable rig, a missing config file and a closed
  pipe each produce one line on stderr and a meaningful exit code. In
  particular `-a HOST` — an address without a scheme — used to end in a
  `zmq.error.ZMQError: Invalid argument` traceback; it now simply works.
- `ErrorCode` was missing every code above `NOT_READY`, so the five config
  error codes (10–14) had no enum member. Constructing a `ServerResponse` from
  such a reply raised `ValueError: 10 is not a valid ErrorCode` — failing while
  reporting the failure. A unit test now asserts the enum matches
  `service.proto` exactly, in both directions.
- An error code newer than this client no longer raises while being parsed; it
  surfaces as `UnknownServerError` with the raw number in the message.

## [0.1.0rc2] — 2026-08-13

### Changed

- `zeroconf` is now a regular dependency rather than the `[discover]` extra, so
  `vstimd-client discover` works from a plain install — including through
  `uvx vstimd-client`. The `[discover]` extra still resolves, as a no-op, so
  the install command documented for 0.1.0rc1 keeps working.

### Fixed

- Two CLI error messages still told users to `pip install vstimd[discover]`,
  naming a distribution that does not exist.

## [0.1.0rc1] — 2026-08-13

First release candidate; the first version published to PyPI.

### Added

- `Connection` — ZMQ/protobuf client covering stimuli (rect, circle, ellipse,
  grating, text, polygon), animations, VTL lines, server config, and system
  queries.
- `vstimd.psychopy` — drop-in replacement for `psychopy.visual` providing
  `Window`, `Rect`, `Circle`, `GratingStim` and deferred (frame-buffer) mode.
- `vstimd-client` command-line tool: `discover`, `info`, `ls`, `background`,
  `delete-all`, `enable-all`/`disable-all`, `wait-frames`, `wait-ready`,
  `shutdown`, and `config list|save|load|get|upload`, all with `--json`.
- mDNS discovery of `_vstimd._tcp` servers via the optional `[discover]` extra,
  falling back to `avahi-browse`.
- PEP 561 typing marker — the package ships `py.typed`.

### Changed

- The client is licensed under the LGPLv3, rather than the AGPLv3 that covers
  the rest of vstimd, so that importing it does not place an experiment's own
  code under copyleft.

[Unreleased]: https://github.com/braemons/vstimd/compare/python-v0.1.0rc3...HEAD
[0.1.0rc3]: https://github.com/braemons/vstimd/compare/python-v0.1.0rc2...python-v0.1.0rc3
[0.1.0rc2]: https://github.com/braemons/vstimd/compare/python-v0.1.0rc1...python-v0.1.0rc2
[0.1.0rc1]: https://github.com/braemons/vstimd/releases/tag/python-v0.1.0rc1
