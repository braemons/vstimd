# Web control UI

!!! note "Placeholder — documentation in progress"
    This page is a stub. The web UI works, but its full documentation (screens,
    controls, screenshots) has not been written yet.

vstimd serves a browser-based **control and configuration UI** directly from the
server — no client install needed. Once the server is running, browse to:

```
http://<device-ip>:8080
```

from any machine on the same network.

## What it's for

The web UI is the no-code path to a running rig. It is meant for **online control and
configuration** rather than scripted experiments:

- inspect and toggle stimuli, background, and draw order live;
- watch scene state, animations, and Virtual Trigger Line activity in real time (it
  subscribes to a `/events` WebSocket that streams a full scene snapshot);
- **save, load, and manage configs** on the device (see
  [Saving & loading scenes](../concepts/saving-loading.md)) — useful for booting a rig
  into a known scene with no experiment PC attached.

For scripted, timing-critical experiments, drive the server from the
[Python client](python.md) or arm on-device
[animations](../concepts/vtl-and-animations.md) instead.

## Availability and configuration

- The HTTP + WebSocket control surface is **enabled by default** on `0.0.0.0:8080`
  (all interfaces).
- The **full React UI** is served only when the binary was built with the embedded UI
  (`make build`, or the `.deb`/`.rpm` packages). A plain `cargo build --release` /
  `make build-server` binary still runs the server and serves the WebSocket API, but
  shows a placeholder page at `/`.
- Configure via the rig-config (`[web] enabled`, `[web] port`) or CLI flags
  (`--no-web`, `--web-port <N>`).
- If the device runs a firewall, open the port: e.g. `sudo ufw allow 8080/tcp`.

See [Deployment → Web control surface](../operations/deployment.md#web-control-surface)
for the deployment details and [Building & packaging](../developer/building.md) for how
the UI is embedded.
