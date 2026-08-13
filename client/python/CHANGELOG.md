# Changelog

All notable changes to `vstimd-client` are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[semantic versioning](https://semver.org/spec/v2.0.0.html). The client is
versioned independently of the vstimd server.

## [Unreleased]

### Added

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

### Fixed

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

[Unreleased]: https://github.com/braemons/vstimd/compare/python-v0.1.0rc2...HEAD
[0.1.0rc2]: https://github.com/braemons/vstimd/compare/python-v0.1.0rc1...python-v0.1.0rc2
[0.1.0rc1]: https://github.com/braemons/vstimd/releases/tag/python-v0.1.0rc1
