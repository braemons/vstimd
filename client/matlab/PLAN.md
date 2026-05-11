# MATLAB Client Plan

Drop-in replacement for the ARCADE `StimServer` MATLAB client. Replaces the
Windows named-pipe + binary protocol with ZeroMQ + protobuf, targeting vstimd
instead of `StimServer.exe`. Goal: swap `arcade/StimServer/` with this folder
in a future ARCADE release and existing experiment code works unchanged.

## Transport & serialization

- **ZMQ**: [JeroMQ](https://github.com/zeromq/jeromq) (pure Java, no native
  `.so`/`.dll`), loaded via `javaaddpath`. Single JAR, works on all platforms
  MATLAB supports.
- **Protobuf**: `protobuf-java` 4.x JAR + Java stubs generated from
  `proto/vstimd/v1/*.proto` and committed to `java/src/`. Stubs are compiled
  into `lib/vstimd-proto.jar` by `java/build.sh`. Both JARs are pinned and
  committed under `lib/`.

From MATLAB, messages are built via the generated Java builder API:

```matlab
req = vstimd.v1.Request.newBuilder() ...
    .setSystem(vstimd.v1.SystemTarget.getDefaultInstance()) ...
    .setSetBackground( ...
        vstimd.v1.SetBackgroundRequest.newBuilder() ...
            .setColor(colorToProto(color)).build()) ...
    .build();
socket.send(req.toByteArray(), 0);
resp = vstimd.v1.Response.parseFrom(socket.recv(0));
```

## File layout

```
client/matlab/
  StimServer.m          % sealed singleton, static methods — mirrors ARCADE API
  Stimulus.m            % abstract base (key, visible, position, delete)
  Shape.m               % abstract (angle, drawMode, faceColor/Alpha, lineColor/Alpha/Width)
  Stimuli/
    Rectangle.m
    Ellipse.m
    Circle.m
    Contents.m
  lib/
    jeromq-0.6.0.jar
    protobuf-java-4.29.0.jar
    vstimd-proto.jar    % built by java/build.sh
  java/
    build.sh            % protoc → javac → jar
    src/                % committed generated Java stubs
  examples/
    flash_rects.m
  setup_vstimd.m        % javaaddpath for the three JARs; call once per session
                        % (or invoke from ARCADE startup)
  PLAN.md               % this file
```

## API mapping

### StimServer (static class)

| ARCADE | vstimd | Notes |
|---|---|---|
| `StimServer.Connect()` | open JeroMQ REQ socket | default `tcp://localhost:5555`; optional host arg for remote |
| `StimServer.Connect(host)` | open JeroMQ REQ socket to `tcp://host:5555` | |
| `StimServer.Disconnect()` | close socket | |
| `StimServer.SetBackgroundColor([r g b])` | `SetBackgroundRequest` | 0–255 → 0–1 conversion |
| `StimServer.Defer(1)` | `SetDeferredModeRequest{active:true}` | |
| `StimServer.Defer(0)` | `SetDeferredModeRequest{active:false}` | |
| `StimServer.Defer(2)` | `SetDeferredModeRequest{active:false, cancel:true}` | |
| `StimServer.RemoveAll()` | `DeleteAllRequest` | |
| `StimServer.ShowAll(v)` | `SetAllEnabledRequest` | |
| `StimServer.GetFrameRate()` | `QueryServerInfoRequest` → `frame_rate` | |
| `StimServer.GetScreenSize()` | `QueryServerInfoRequest` → `[width height]` | |
| `StimServer.GetConnectionStatus()` | check socket is non-empty | |

`PDshow`, `PDmode`, `PDposition`, `SetDefaultDrawColor`, `SetDefaultFinalAction`,
`InvertGammaCorrection` — emit `warning('vstimd: not supported')` and return.

### Stimulus (abstract base)

| ARCADE property/method | vstimd | Notes |
|---|---|---|
| `stim.visible = v` | `SetEnabledRequest` | |
| `stim.position = [x y]` | `SetPositionRequest` | same pixel coords, origin = screen centre, Y-up |
| `get(stim, 'position')` | `QueryStimulusRequest` → `pos` | |
| `delete(stim)` | `DeleteRequest` | |
| `stim.play_animation(a)` | not yet implemented | `error()` |
| `stim.stop_animation()` | not yet implemented | `error()` |
| `stim.bring_to_front()` | not yet implemented | `error()` |
| `stim.swap_order_with(s2)` | not yet implemented | `error()` |
| `stim.toggle_visibility()` | implemented in MATLAB (no server call) | reads `visible`, flips it |

Constructor reads the returned handle from `Response.handle` (analogous to
`StimServer.ReadAck()` in ARCADE).

### Shape (abstract, extends Stimulus)

| ARCADE | vstimd | Notes |
|---|---|---|
| `shape.angle = deg` | `SetOrientationRequest` | same CCW degrees |
| `shape.drawMode = m` | `SetDrawModeRequest` | 1=filled, 2=outlined, 3=both |
| `shape.faceColor = [r g b]` | `SetFillColorRequest` | 0–255 → 0–1 |
| `shape.faceAlpha = a` | `SetAlphaRequest` | a/255 → 0–1 |
| `shape.lineColor = [r g b]` | `SetOutlineColorRequest` | 0–255 → 0–1 |
| `shape.lineAlpha = a` | `SetOutlineWidthRequest` — alpha not separate in vstimd | see note below |
| `shape.lineWidth = w` | `SetOutlineWidthRequest` | |

> **Note on `lineAlpha`:** vstimd does not currently expose a separate outline
> alpha — the outline uses the same global opacity as the fill (`SetAlphaRequest`).
> Setting `lineAlpha` will `warning()` and be ignored until vstimd adds per-channel
> alpha.

### Rectangle

| ARCADE | vstimd |
|---|---|
| `Rectangle()` | `CreateRectRequest` |
| `rect.width = w` | `SetRectSizeRequest{width:w, height:obj.height}` |
| `rect.height = h` | `SetRectSizeRequest{width:obj.width, height:h}` |

### Ellipse

| ARCADE | vstimd |
|---|---|
| `Ellipse()` | `CreateEllipseRequest` |
| `e.width = w` | `SetEllipseSizeRequest{width:w, height:obj.height}` |
| `e.height = h` | `SetEllipseSizeRequest{width:obj.width, height:h}` |

### Circle

| ARCADE | vstimd |
|---|---|
| `Circle()` | `CreateCircleRequest` |
| `c.diameter = d` | `SetDiscRadiusRequest{radius:d/2}` |
| `get(c, 'diameter')` | `QueryStimulusRequest` → `params.disc.radius * 2` |

## Stimulus types not yet in vstimd

The following ARCADE stimulus classes have no vstimd counterpart yet.
They will be kept as stub `.m` files that `error()` on construction with a
clear message:

- `Gabor`, `Grating`, `SquareWaveGrating`, `Gaussian`, `Gammatron`
- `Picture`, `PixelShadedPicture`, `MotionPicture`
- `MovingBar`, `Petal`, `Wedge`, `ParticleStimulus`
- `Symbol` (bitmap glyph)

## Build step

`java/build.sh` — run once after cloning or when `.proto` files change:

```bash
#!/usr/bin/env bash
set -euo pipefail
REPO_ROOT=$(git rev-parse --show-toplevel)
PROTO_ROOT="$REPO_ROOT/proto"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

mkdir -p "$SCRIPT_DIR/src" "$SCRIPT_DIR/classes"

protoc -I"$PROTO_ROOT" \
  --java_out="$SCRIPT_DIR/src" \
  "$PROTO_ROOT"/vstimd/v1/*.proto

javac -cp "$SCRIPT_DIR/../lib/protobuf-java-4.29.0.jar" \
  -d "$SCRIPT_DIR/classes" \
  $(find "$SCRIPT_DIR/src" -name '*.java')

jar cf "$SCRIPT_DIR/../lib/vstimd-proto.jar" \
  -C "$SCRIPT_DIR/classes" .
```

## Open questions

- Should `Connect()` default to `localhost` (most common case) or require an
  explicit host? Current plan: default `localhost`, optional string arg for
  remote — same as ARCADE's optional server arg.
- Port should be configurable (second optional arg or name-value pair), default
  5555.
