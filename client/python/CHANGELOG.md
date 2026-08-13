# Changelog

All notable changes to `vstimd-client` are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[semantic versioning](https://semver.org/spec/v2.0.0.html). The client is
versioned independently of the vstimd server.

## [Unreleased]

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

[Unreleased]: https://github.com/braemons/vstimd/compare/python-v0.1.0rc1...HEAD
[0.1.0rc1]: https://github.com/braemons/vstimd/releases/tag/python-v0.1.0rc1
