# Changelog

All notable changes to `vstimd-client` are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[semantic versioning](https://semver.org/spec/v2.0.0.html). The client is
versioned independently of the vstimd server.

## [Unreleased]

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

[Unreleased]: https://github.com/braemons/vstimd/compare/python-v0.1.0rc2...HEAD
[0.1.0rc2]: https://github.com/braemons/vstimd/compare/python-v0.1.0rc1...python-v0.1.0rc2
[0.1.0rc1]: https://github.com/braemons/vstimd/releases/tag/python-v0.1.0rc1
