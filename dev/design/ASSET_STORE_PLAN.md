# Asset store plan

**Status:** proposal, not implemented.
**Blocks:** #108 (image stimulus), #109 (scriptable movie stimulus), #70 (mesh
textures).
**Supersedes:** Phase A of `dev/design/IMAGES_MOVIES_PLAN.md` (the in-memory
`UploadAsset` store). Phases B–D of that document still describe the stimuli;
this document replaces the storage layer they sit on.

---

## 1. The problem

Everything vstimd draws today is *described* by a command: a rect is four
numbers, a grating is a dozen, a text stimulus is a string. Nothing the server
renders comes from a file. #108 and #109 break that: an image stimulus needs
pixels, a movie script needs a program plus the hundreds of frames it presents,
and #70 wants textures for meshes.

Three constraints shape the answer:

- **The client is usually on another machine.** The primary deployment is a
  PsychoPy workstation driving a Jetson Nano / Raspberry Pi over ZMQ. There is no
  shared filesystem, so a client-side path is meaningless to the server.
- **The render thread must never block or heap-allocate.** Decode and upload
  happen once, at create time, on the ZMQ thread. The render thread only binds a
  resident GPU resource.
- **A rig must boot into a scene with no client connected.** Save/load already
  guarantees that (`docs/concepts/saving-loading.md`); an asset reference inside a
  saved config must not break it.

So: assets live **on the device**, are addressed by **name, never by path**
(exactly like scene-configs), and can be installed by upload over ZMQ, over the
Samba share, or by hand over ssh.

---

## 2. Where assets live

### Recommendation

```
<asset-dir>/            default: <config-dir>/assets  →  /var/lib/braemons/vstimd/assets
  <project>/
    images/
    meshes/
    scripts/
    data/
```

`--asset-dir <path>` overrides it, resolved by exactly the ladder
`resolve_config_dir` already uses (`server/src/main.rs:340`): explicit flag →
`/var/lib/braemons/vstimd/assets` → `~/.local/braemons/vstimd/assets` → `./assets`,
picking the first writable one via `first_writable_dir`.

### Why under the existing config dir, not `/var/lib/braemons/assets`

The obvious alternative — a sibling `assets/` next to `vstimd/`, shared by every
braemons daemon — costs more than it buys today:

| | `<config-dir>/assets` | `/var/lib/braemons/assets` |
|---|---|---|
| systemd | already writable: `StateDirectory=braemons/vstimd` (`packaging/systemd/vstimd.service:44`) | needs a second `StateDirectory=braemons/assets`, and ownership decisions between services |
| Samba | already visible: the `vstimd-data` share exports `/var/lib/braemons` (`packaging/samba/vstimd-shares.conf:65`) | also visible — no difference |
| Dev run | falls out of the existing `~/.local/braemons/vstimd` fallback | needs a parallel fallback ladder |
| Sharing between tools | none today — `gpiochip-daqd` has no assets | the only real advantage, and speculative |
| SD-image upgrade | preserved (`/var/lib/braemons` is what an upgrade keeps) | preserved |

If a second braemons service ever needs the same files, hoisting the directory is
one constant plus a compat symlink. Doing it now buys a permissions problem and
nothing else.

### Why project-first, type-second

A **project** is the unit humans move around: you drag one folder onto the Samba
share, you archive it, you delete it when the study ends. A **type** is the unit
*the server* cares about: it says how to interpret the bytes and it scopes name
collisions (`stim.png` and `stim.obj` can coexist). Project-first also means
`rm -rf assets/oldstudy` is a complete, safe uninstall.

- Project names: `[A-Za-z0-9._-]{1,64}`. Reserved: names starting with `_`
  (server-owned, see `_scratch` in §6).
- Types: a **fixed, server-known set**. An unknown directory at the type level is
  ignored with one warning at scan time (forward compatibility with a newer
  server having written there), and rejected on upload.
- Below the type, arbitrary nesting is allowed: a movie's frames are naturally
  `myproject/images/clip_a/frame_0001.png`.

### The type set (v1)

| Type | Contents | Consumer |
|---|---|---|
| `images/` | PNG, JPEG (later: EXR, TIFF) | #108 image stimulus; #70 mesh textures |
| `meshes/` | OBJ, glTF | #70 / 3-D roadmap |
| `scripts/` | `.mv1`, or whatever #109 settles on | #109 |
| `data/` | opaque blobs: lookup tables, calibration, gamma tables | anything |

Deliberately **not** in v1:

- **`scenes/`** — scene-configs already have their own storage, their own bare-name
  addressing, demo seeding, the save-on-quit slot and timestamped archives
  (`server/src/scene_config_file.rs`). Folding them into the asset tree is a
  migration with no user benefit today. The name is *reserved* so a future
  unification can take it.
- **`textures/`** — a texture is a PNG. One `images/` type, referenced by both an
  image stimulus and a mesh's `texture` field. Fewer types is the whole point.
- **`fonts/`** — reserved; revisit if the text stimulus grows user-supplied fonts.

---

## 3. The asset reference

One string, the only thing that ever crosses the wire or lands in a config:

```
<project>/<type>/<name...>       e.g.  faces2026/images/id07_neutral.png
```

Validation (one function, exhaustively unit-tested, no filesystem access):

- at least three segments; second segment is a known type
- every segment matches `[A-Za-z0-9._-]{1,64}`, so `..`, absolute paths,
  backslashes, NUL, leading `-` and unicode lookalikes are all rejected by
  construction rather than by sanitising
- total length ≤ 512 bytes, depth ≤ 8
- case-sensitive, but a name that differs from an existing one **only** by case is
  rejected on upload (macOS and Samba clients will collide otherwise)

Refs are always fully qualified on the wire. "Default project" convenience is a
*client* feature (`conn.assets.project = "faces2026"`); the server stays dumb.
That keeps a saved config unambiguous when it is loaded by a rig with no client.

---

## 4. Server module

```
server/src/assets/
  mod.rs        AssetStore: the public API
  asset_ref.rs  AssetRef: parse / validate / to_path — pure, no I/O
  store.rs      scan, stat, read, write, delete
```

`AssetStore` holds the root path plus a metadata cache keyed by ref, each entry
`{ size, mtime, sha256: Option }`.

**The filesystem is the single source of truth.** Files arrive over Samba, ssh
and `rsync` without the server's knowledge, so there is no sidecar index and no
manifest to rot. The cache is validated against `(size, mtime)` on every lookup
and discarded when it disagrees; `sha256` is computed lazily (on first request,
on config save, on cache miss) and never required for a plain read. This is the
same rule as "the config format *is* the runtime shape": no second source of
truth that can disagree with the first.

**Lock placement.** `zmq_server.rs:151` takes the scene **write lock** around
every `handle_request`. A 40 MB chunked upload must not serialise behind — or in
front of — scene commands for its whole duration. Two options, decide at
implementation time:

- (a) keep chunks small (≤ 512 KB) and accept that each chunk briefly holds the
  write lock — simplest, and the lock is dropped between chunks so the render
  thread still gets its window every frame;
- (b) dispatch asset commands *before* acquiring the scene lock, with the store
  behind its own `RwLock`.

(a) is the recommended start; (b) is the fallback if upload is observed to cost
frames. Either way this is a documented decision, not an accident.

---

## 5. Protocol

New `proto/vstimd/v1/assets.proto`, all system-target commands, wired in
`ipc/dispatch.rs` + a new `ipc/asset_commands.rs` (per the module rule in
CLAUDE.md).

```proto
// List assets, optionally filtered. Both filters empty = whole store.
message ListAssetsRequest {
  string project     = 1;  // "" = all
  string type        = 2;  // "" = all
  bool   with_digest = 3;  // compute sha256 for each hit (can be slow)
}
message AssetInfo {
  string ref           = 1;
  uint64 size          = 2;
  int64  modified_unix = 3;
  string sha256        = 4;  // empty unless requested
}
message ListAssetsResponse { repeated AssetInfo assets = 1; }

// Upload one chunk. offset==0 creates/truncates a temp file; the asset becomes
// visible only when the last chunk lands and the digest matches — an interrupted
// upload leaves no half-asset behind.
message UploadAssetRequest {
  string ref        = 1;
  uint64 total_size = 2;
  uint64 offset     = 3;
  bytes  data       = 4;
  string sha256     = 5;  // of the whole asset, verified on completion
  bool   overwrite  = 6;
}

// Read back, chunked the same way. Lets a client verify, back up, or preview.
message DownloadAssetRequest { string ref = 1; uint64 offset = 2; uint32 max_bytes = 3; }
message DownloadAssetResponse { bytes data = 1; uint64 total_size = 2; string sha256 = 3; }

message DeleteAssetRequest { string ref = 1; bool recursive = 2; }  // recursive: a whole prefix
```

Errors get their own codes alongside the existing config ones
(`service.proto:46`): `ASSET_NOT_FOUND`, `ASSET_EXISTS` (upload without
overwrite), `ASSET_INVALID_REF`, `ASSET_TOO_LARGE`, `ASSET_DIGEST_MISMATCH`,
`ASSET_UNSUPPORTED_TYPE`.

**Limits** (rig-config, `[assets]`): `max_asset_bytes` (default 256 MB),
`max_store_bytes` warn threshold, `max_image_pixels` — an image larger than the
device's `maxImageDimension2D` is **rejected at create time with the actual
limit in the message**, never silently clamped (#108 asks for this decision
explicitly).

---

## 6. Inline bytes, and how they round-trip

#108 wants raw pixels in a create request for one-offs. Model it as a `oneof` on
the create request:

```proto
oneof source {
  string asset_ref = 1;   // named asset on the device
  bytes  inline    = 2;   // one-off, e.g. a generated noise patch
}
```

Inline bytes are **promoted to a real asset on arrival**, written to
`_scratch/images/<sha256[..16]>.png`, and the stimulus keeps only the resulting
ref. Consequences, all good:

- the texture cache has exactly one keying scheme (ref + digest), not two
- save/load round-trips for free — an inline-created stimulus saves as a normal
  asset reference
- identical inline uploads deduplicate by content hash
- `_scratch` is the one project the server may garbage-collect: on startup, any
  `_scratch` asset referenced by no config in the config dir and older than N
  days is deleted (default off; a rig-config knob).

---

## 7. Save / load semantics

A stimulus serialises its asset as:

```json
"source": { "ref": "faces2026/images/id07_neutral.png",
            "sha256": "9f86d0…", "size": 240192 }
```

The digest and size are **advisory** — recorded at save, checked at load, never
used to locate the file.

On load:

| Situation | Behaviour |
|---|---|
| asset present, digest matches | normal |
| asset present, digest differs | load succeeds; **warning event** naming the ref, both digests |
| asset missing | the stimulus loads **disabled**, in an `unresolved` state, and a warning event names the missing ref. The rest of the config loads normally |
| `LoadConfigRequest.strict_assets = true` | any missing asset fails the whole load instead |

The default is lenient because the alternative breaks the property that makes
save/load worth having: a rig boots into a scene with no client attached. A
scene with one blank stimulus and a loud warning beats a rig that boots to
nothing. `strict_assets` exists for CI and deployment scripts, which *do* want a
hard failure.

An `unresolved` stimulus becomes live the moment its asset appears — resolution
is retried on `enable` and on an explicit `assets refresh` command, so the fix
("drop the file on the Samba share") needs no reload.

---

## 8. GPU side (what #108 and #70 actually consume)

- Decode (`image` crate) and upload happen on the ZMQ thread, at create time.
- The texture cache lives where every other GPU cache lives — `render/vk/cache/`,
  a `texture_cache.rs` beside `solid_mesh_cache.rs` — keyed by `(AssetRef,
  sha256)`. Replacing an asset therefore invalidates its texture without any
  explicit invalidation call.
- Nothing in `scene/` holds a texture handle. `Mesh3d::texture_path: Option<String>`
  (`server/src/scene/stimulus/mesh3d.rs:44`) becomes `Option<AssetRef>` — the
  same change #70 needs, which is why the two are worth designing together, as
  #108 notes.
- Premultiplied-alpha upload, so non-1.0 opacity composites correctly (#108).

---

## 9. Clients

- **Python:** `conn.assets.upload(local_path, ref=…)`, `.list(project=…, type=…)`,
  `.download(ref, local_path)`, `.delete(ref)`, `.exists(ref)`; chunking hidden.
  New `client/python/vstimd/assets/`, mirroring `config/`.
- **CLI:** `vstimd-client asset list|push|pull|rm`, with `push -r <dir>` for a
  whole project folder — the "install my study" one-liner.
- **Web:** an asset section with drag-and-drop upload and a thumbnail grid; the
  web surface already speaks proto (`proto.rs` is at the crate root for exactly
  this reason).
- **Overlay:** a read-only Assets panel (count + total size per project) so an
  operator at the rig can see what is installed. No upload from the overlay.

---

## 10. Docs

- New `docs/concepts/assets.md` — the model, the ref syntax, the four install
  routes (client, CLI, web, Samba/ssh).
- `docs/concepts/saving-loading.md` — the missing-asset table from §7.
- `docs/operations/appliance-setup.md` + `raspberry-pi-image.md` — the share now
  also carries assets; what an upgrade preserves.
- `docs/client/{python,cli,web}.md` — the new namespace.

---

## 11. Phasing

Each phase is a shippable PR; the first three are pure infrastructure and unblock
#108.

| # | Phase | Depends on |
|---|---|---|
| 1 | `AssetRef` + `AssetStore` (parse, scan, stat, read, write, delete) + unit tests. No proto, no GPU | — |
| 2 | `assets.proto`, `ipc/asset_commands.rs`, dispatcher wiring, error codes, integration tests via `SceneState::handle_request` | 1 |
| 3 | Python client + CLI + web + overlay panel + docs | 2 |
| 4 | Config round-trip: `source` serialisation, `unresolved` state, warning events, `strict_assets` | 2 |
| 5 | Texture cache + premultiplied upload — lands with #108 | 2 |
| 6 | Consumers: #108 image stimulus, #70 mesh textures, #109 scripts | 4, 5 |

---

## 12. Open questions

1. **Upload over ZMQ REQ/REP vs. a side channel.** Chunked REQ/REP needs no new
   socket and no new port in the rig's firewall story, which is why it wins here.
   If installing a 10 000-frame movie proves painfully slow, the answer is a
   second ZMQ socket for bulk transfer, not a bigger chunk size. Measure first.
2. **`_scratch` GC policy** — off by default, or age-based by default?
3. **Does a project need metadata** (a description, a creation date, a `README`)?
   A plain `_project.toml` the server never parses is nearly free; skip until
   someone asks.
4. **Per-project quotas** — probably never; a single store-wide warn threshold is
   likely enough for a single-user rig.
5. **Digest algorithm** — sha256 for familiarity, or blake3 for speed on a
   Jetson? Only matters once assets are large; the field is a string either way.
