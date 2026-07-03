# Wire protocol

vstimd speaks **[Protocol Buffers](https://protobuf.dev/) (proto3) over
[ZMQ](https://zeromq.org/) REQ/REP**. Every client — the [Python client](../client/python.md), the web UI, or
one you write yourself — encodes the same messages, so the protocol is the real
contract for talking to the server.

## Why ZeroMQ and protobuf

The transport is chosen for **timing**, not features:

- **ZeroMQ** gives low-latency messaging with very little overhead. It is a thin,
  brokerless layer over TCP with **no authentication or session negotiation** on the
  hot path, so a command round-trip costs little more than the socket itself. (The
  flip side: the server does no access control — keep it on a trusted rig network.)
- **Binary messages.** Commands and responses are serialized to compact binary rather
  than text (JSON/XML), keeping encode/decode time and wire size small — the point is
  to spend the frame's millisecond budget on rendering, not parsing.
- **Protocol Buffers** is the most widespread binary schema format, with mature code
  generators for virtually every programming language. That means a client can be
  written in Python, C#, C++, MATLAB, Go, Rust, … from the same `.proto` files, with
  no bespoke parser to maintain.

!!! tip "New to protobuf?"
    Protocol Buffers is Google's language- and platform-neutral serialization
    format. If you have not used it before, start here:

    - **[Protocol Buffers overview](https://protobuf.dev/overview/)** — what it is
      and why.
    - **[proto3 language guide](https://protobuf.dev/programming-guides/proto3/)** —
      the schema syntax vstimd uses.
    - **[Tutorials](https://protobuf.dev/getting-started/)** — generating code for
      Python, C++, C#, Go, and more.

## The schema is the source of truth

The message definitions live in the repository and are the authoritative,
always-current reference — this page does **not** duplicate them, because a
hand-maintained message list drifts out of date. Read the `.proto` files directly:

**→ [`proto/vstimd/v1/`](https://github.com/braemons/vstimd/tree/main/proto/vstimd/v1)**

| File | Contents |
|---|---|
| [`service.proto`](https://github.com/braemons/vstimd/blob/main/proto/vstimd/v1/service.proto) | `Request` / `Response` envelopes, `ErrorCode` |
| [`system.proto`](https://github.com/braemons/vstimd/blob/main/proto/vstimd/v1/system.proto) | Scene-wide commands, `QueryServerInfoResponse`, config persistence |
| [`stimuli/`](https://github.com/braemons/vstimd/tree/main/proto/vstimd/v1/stimuli) | Create / mutate / query messages per stimulus type |
| [`animations.proto`](https://github.com/braemons/vstimd/blob/main/proto/vstimd/v1/animations.proto) | Animation create / arm / query |
| [`vtl.proto`](https://github.com/braemons/vstimd/blob/main/proto/vstimd/v1/vtl.proto) | Virtual Trigger Line commands |
| [`snapshot.proto`](https://github.com/braemons/vstimd/blob/main/proto/vstimd/v1/snapshot.proto) | Full scene snapshot pushed on the web UI's `/events` WebSocket |

## Transport

- **Socket type:** ZMQ REQ/REP (synchronous request-reply)
- **Address:** the server binds `tcp://0.0.0.0:5555`; clients connect to
  `tcp://<device>:5555`
- **Encoding:** protobuf (proto3); every ZMQ frame carries exactly one `Request`,
  answered by exactly one `Response`

!!! warning "Bind address gotcha"
    The server binds `tcp://0.0.0.0:5555`. Do **not** use `tcp://*:5555` — the
    `zeromq` crate resolves the host as DNS and that form fails.

## The envelope, at a glance

Every `Request` carries a **target** (`system` for scene-wide commands, or a
`stimulus` handle) and a `oneof body` naming exactly one command. Every `Response`
carries a `handle`, an `ErrorCode`, and an optional query payload. The precise field
numbers and the full command set are in
[`service.proto`](https://github.com/braemons/vstimd/blob/main/proto/vstimd/v1/service.proto);
the shape is:

```protobuf
message Request {
  oneof target {
    SystemTarget system   = 1;   // scene-wide commands
    uint32       stimulus = 2;   // per-stimulus commands (handle > 0)
  }
  oneof body {
    CreateRectRequest create_rect  = 10;   // system target
    SetPositionRequest set_position = 32;  // stimulus target
    // … one field per command; see service.proto
  }
}

message Response {
  int32     handle = 1;   // new handle on Create*, -1 ack, 0 on error
  ErrorCode code   = 2;   // ERROR_CODE_OK on success
  string    error  = 3;   // human-readable, only when code != OK
  string    id     = 4;   // UUID on Create*
  // oneof body { … query payloads … }
}
```

## A raw round-trip in Python

You rarely need this — the [Python client](../client/python.md) wraps it — but it
shows the protocol end to end using the generated stubs and `pyzmq`:

```python
import zmq
from vstimd._proto.vstimd.v1 import service_pb2, system_pb2
from vstimd._proto.vstimd.v1.stimuli import rect_pb2

ctx = zmq.Context()
sock = ctx.socket(zmq.REQ)
sock.connect("tcp://localhost:5555")

# Build a "create rectangle" request (system target).
req = service_pb2.Request(
    system=service_pb2.SystemTarget(),
    create_rect=rect_pb2.CreateRectRequest(width=300, height=150),
)
sock.send(req.SerializeToString())

resp = service_pb2.Response()
resp.ParseFromString(sock.recv())
assert resp.code == service_pb2.ERROR_CODE_OK
print("new stimulus handle:", resp.handle)
```

## Regenerating stubs

The Python stubs under `client/python/vstimd/_proto/` are generated from the
`.proto` files. Regenerate them with the client Makefile:

```sh
cd client/python
make proto
```

## Error codes

`ErrorCode` values are defined in
[`service.proto`](https://github.com/braemons/vstimd/blob/main/proto/vstimd/v1/service.proto).
Note that `ERROR_CODE_OK` is **1** (the proto3 zero value `ERROR_CODE_UNSPECIFIED`
marks an unset/malformed response). The Python client translates each non-OK code
into a typed exception — see [Python client → Errors](../client/python.md#errors).

## gRPC

`service.proto` also declares a `VstimdService` with `Execute` / `ExecuteBatch`
RPCs. The primary transport is ZMQ REQ/REP; the service block is included so tooling
(grpcurl, reflection) works and to keep the option of a gRPC adapter open without a
schema change. There is no gRPC listener today.
