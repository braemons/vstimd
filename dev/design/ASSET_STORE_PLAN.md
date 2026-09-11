# Projects and the asset store

**Status:** proposal. **Do not start implementing.** Blocked on the in-flight
refactor series finishing — Phase 0 renames flags, wire messages and client
namespaces across the same files, and racing it would produce conflicts nobody
wants to resolve twice. #129 has landed; this waits for the rest.
**Blocks:** #108 (image stimulus), #109 (scriptable movie stimulus), #70 (mesh
textures).
**Touches:** every scene-config path, and the not-yet-built event logger
(`dev/EVENT_LOGGING.md`).
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
  saved scene-config must not break it.

So: assets live **on the device**, are addressed by **name, never by path**
(exactly like scene-configs), and can be installed by upload over ZMQ, over the
Samba share, or by hand over ssh.

---

## 2. Where things live

### Recommendation

The storage dir holds **projects**. A project holds **typed files**, and a
scene-config is one of the types:

```
<storage-dir>/                     /var/lib/braemons/vstimd  (or ~/.local/braemons/vstimd)
  projects/
    <project>/
      scene-configs/             <name>.config.json
      images/
      meshes/
      scripts/
      data/
      logs/                      ← written, not uploaded (dev/EVENT_LOGGING.md)
    default/                     ← where an unqualified name lands
    demos/                       ← seeded at startup
    _session/                    ← last-session slot + quit-time archives
```

**`<storage-dir>` is the directory systemd's `StateDirectory=braemons/vstimd`
creates** — `/var/lib/braemons/vstimd` on a packaged rig, `~/.local/braemons/vstimd`
on a dev run. Today it holds scene-configs at its root and nothing else, which is
why the flag naming it is spelled `--config-dir`; assets make that spelling wrong
twice over (it is no longer only configs, and bare "config" is exactly the word
`CLAUDE.md`'s two-configs rule forbids). **Phase 0 fixes the vocabulary and the
layout before anything is built on top of them** — see §11.

Nothing is released beyond `v0.1.0-alpha*`/`beta*` pre-releases, so Phase 0 breaks
freely: no aliases, no migration shims, no deprecation window.

`--storage-dir <path>` names the root; `projects/` is always its child, so there is
one flag and no way to point the pieces at unrelated places. It is
resolved by exactly the ladder `resolve_config_dir` uses today
(`server/src/main.rs:340`): explicit flag → `/var/lib/braemons/vstimd` →
`~/.local/braemons/vstimd` → `.`, picking the first writable one via
`first_writable_dir`.

### Why under the storage dir, not `/var/lib/braemons/assets`

The obvious alternative — a sibling `assets/` next to `vstimd/`, shared by every
braemons daemon — costs more than it buys today:

| | `<storage-dir>/projects` | `/var/lib/braemons/assets` |
|---|---|---|
| systemd | already writable: `StateDirectory=braemons/vstimd` (`packaging/systemd/vstimd.service:44`) | needs a second `StateDirectory=braemons/assets`, and ownership decisions between services |
| Samba | already visible: the `vstimd-data` share exports `/var/lib/braemons` (`packaging/samba/vstimd-shares.conf:65`) | also visible — no difference |
| Dev run | falls out of the existing `~/.local/braemons/vstimd` fallback | needs a parallel fallback ladder |
| Sharing between tools | none today — `gpiochip-daqd` has no assets | the only real advantage, and speculative |
| SD-image upgrade | preserved (`/var/lib/braemons` is what an upgrade keeps) | preserved |

If a second braemons service ever needs the same files, hoisting the directory is
one constant plus a compat symlink. Doing it now buys a permissions problem and
nothing else.

### Why the scene-config is *inside* the project

The alternative — `scene-configs/` as a sibling of `assets/`, addressed by bare
name — keeps today's addressing and is a smaller change. It is still the wrong
split, for one reason that outweighs the rest:

**A scene-config and the files it references are one artefact.** A config for a
face-discrimination study is meaningless without the 400 face images it names.
Split them across two trees and every operation on a study becomes two operations
that can drift: copying an experiment to a second rig, archiving it at the end,
deleting it, versioning it in git, or handing it to a collaborator. Put them in one
directory and a study is a **folder you can drag onto the Samba share**, and
`rm -rf projects/oldstudy` is a complete, safe uninstall.

The payoff that makes it more than tidiness: **asset refs inside a scene-config can
be project-relative** (§3), so a project keeps working after it is renamed or
copied to a rig where it lives under a different name. A cross-tree layout can
never offer that — the config would hard-code a project name that the copy
invalidates.

A **type** stays the second level because that is what the *server* needs: it says
how to interpret the bytes and it scopes name collisions (`stim.png`, `stim.obj`
and a `stim` scene-config can coexist).

- Project names: `[A-Za-z0-9._-]{1,64}`. Names starting with `_` are server-owned.
- Types: a **fixed, server-known set**. An unknown directory at the type level is
  ignored with one warning at scan time (forward compatibility with a newer server
  having written there), and rejected on upload.
- Below the type, arbitrary nesting is allowed: a movie's frames are naturally
  `myproject/images/clip_a/frame_0001.png`.

### The type set (v1)

| Type | Contents | Consumer |
|---|---|---|
| `scene-configs/` | `<name>.config.json` | save/load, demos, the overlay's config panel |
| `images/` | PNG, JPEG (later: EXR, TIFF) | #108 image stimulus; #70 mesh textures |
| `meshes/` | OBJ, glTF | #70 / 3-D roadmap |
| `scripts/` | `.mv1`, or whatever #109 settles on | #109 |
| `data/` | opaque blobs: lookup tables, calibration, gamma tables | anything |
| `logs/` | `*.wllog` event logs, and any SQLite export beside them | `dev/EVENT_LOGGING.md` |

Deliberately **not** types:

- **`textures/`** — a texture is a PNG. One `images/` type, referenced by both an
  image stimulus and a mesh's `texture` field. Fewer types is the whole point.
- **`fonts/`** — reserved; revisit if the text stimulus grows user-supplied fonts.

Scene-configs keep their own **command surface** (`LoadSceneConfig`,
`UploadSceneConfig`, …) rather than being loaded through the asset commands: they
are the one type the server parses, versions and applies, and they carry machinery
no other type has. Only their *storage* joins the project tree.

### Inputs and outputs in one tree

`logs/` is the odd one out and worth being explicit about: every other type is an
**input** — uploaded once, read many times, small in number. An event log is an
**output** — the server writes it during a session, it grows without bound, and
nobody uploads one. Putting them in the same project directory anyway is right,
because it is what makes a project a complete record of a study: the scene-config
that ran, the images it presented, and what actually happened, in one folder you
can copy off the rig when the study ends.

What differs is the API, not the location:

- `logs/` is **read-only over the wire**: `ListAssets` and `DownloadAsset` work,
  `UploadAsset` is rejected with `ASSET_READ_ONLY_TYPE`, and `DeleteAsset` is
  allowed (an operator reclaiming disk) but never implicit.
- Its writer is the messenger thread of `dev/EVENT_LOGGING.md`, not the asset
  store. That document's `--log-dir` (default `./logs/`) becomes
  `<storage-dir>/projects/<project>/logs/`, which also answers its open question of
  which project a session's log belongs to: **the project of the scene-config
  currently loaded**, falling back to `default` when a session runs an unsaved
  scene.
- Retention is a `logs`-only concern the rest of the store does not have. Out of
  scope here; `dev/EVENT_LOGGING.md` should own it. The store-wide size warning of
  §5 will fire on logs long before it fires on assets, so the warning message must
  break the total down per type or it will be useless.

### The reserved projects

| Project | Holds | Notes |
|---|---|---|
| `default` | anything saved without naming a project | keeps `scene-config save test` a one-word operation, and is the active project at boot |
| `demos` | the shipped demo scene-configs | replaces the `demo_` name prefix (`scene_config_file.rs:DEMO_PREFIX`); a demo that ships with images later needs no special case, which the prefix scheme could never offer |
| `_session` | the `_last_session` save-on-quit slot and the timestamped quit archives | per-rig, not per-study, so it does not belong in `default` |
| `_scratch` | promoted inline uploads (§6) | the only project the server may garbage-collect |

`default` and `demos` are ordinary projects the server seeds; the
leading-underscore ones are server-owned and cannot be created or deleted over the
wire. `ListProjects` marks both kinds so a UI can fold them away.

---

## 3. The asset reference

One string, the only thing that ever crosses the wire or lands in a scene-config:

```
absolute   <project>/<type>/<name...>   faces2026/images/id07_neutral.png
relative              <type>/<name...>  images/id07_neutral.png
```

Validation (one function, exhaustively unit-tested, no filesystem access):

- 2 segments (relative) or >= 3 (absolute); the type segment must be a known type,
  which is what tells the two forms apart with no ambiguity — a project may not be
  named after a type
- every segment matches `[A-Za-z0-9._-]{1,64}`, so `..`, absolute paths,
  backslashes, NUL, leading `-` and unicode lookalikes are all rejected by
  construction rather than by sanitising
- total length ≤ 512 bytes, depth ≤ 8
- case-sensitive, but a name that differs from an existing one **only** by case is
  rejected on upload (macOS and Samba clients will collide otherwise)

**Where each form is legal:**

| Context | Form | Why |
|---|---|---|
| A ref *inside* a saved scene-config | **relative**, resolved against the project the config was loaded from | makes the project relocatable: rename the folder, or copy it to another rig under another name, and every ref still resolves |
| A ref in a live command from a client | either — a relative ref resolves against the **active project** (below) | one setting, then short refs everywhere; a client that wants to be explicit, or to reach into another project, sends the absolute form |

Saving a scene-config **rewrites** every ref that points into the project being
saved to the relative form, and leaves refs pointing at *other* projects absolute.
That is the one asymmetry in the scheme, and it is what makes a project
self-contained without forbidding deliberate cross-project reuse of, say, a shared
calibration image.

### The active project

The server holds one **active project** — `SceneState.runtime.active_project`,
default `default`. It is what a relative ref in a live command resolves against,
where `scene-config save <name>` writes, and which `logs/` directory the event log
lands in.

**Resolution happens once, at command-handling time.** A relative ref arriving in a
create command is expanded to its absolute form before it is stored in the scene, so
a later project change cannot retroactively repoint a live stimulus. Nothing in the
scene tree ever holds an unresolved ref; the relative form exists only on the wire
and in a saved file.

Set it four ways, all the same state:

| Where | How |
|---|---|
| Boot | `vstimd --project faces2026`, or `[startup] project = "faces2026"` in the rig-config, beside the existing `load_config` |
| Wire | `SetProjectRequest { name, create }` — a system command, `GetProject` to read back |
| Implicitly | loading a scene-config sets the active project to the one it came from. This is the common path: `scene-config load faces2026/session1` and everything downstream is already pointed at the right place |
| Overlay / web | a project selector, see §9 |

**It is server-global state, not per-connection** — the same as the background
colour or deferred mode. The server has no session concept behind a ZMQ REQ/REP
socket, and inventing one for this alone would be a large change for a rig that
almost always has a single controlling client. The consequence must be documented
rather than hidden: two clients driving one rig share the setting, and a client that
cannot assume exclusivity should send absolute refs and pass an explicit project to
every scene-config command. A `ProjectChanged` event is emitted on every change so
overlays and secondary clients stay in sync rather than guessing.

---

## 4. Server module

```
server/src/assets/
  mod.rs        AssetStore: the public API
  asset_ref.rs  AssetRef: parse / validate / resolve / to_path — pure, no I/O
  project.rs    project names, the reserved set, enumeration
  store.rs      scan, stat, read, write, delete
```

`AssetStore` holds the storage-dir root plus a metadata cache keyed by absolute ref,
each entry `{ size, mtime, sha256: Option }`. It does **not** hold the active
project: that is scene state (`SceneState.runtime.active_project`), because it
changes with a loaded scene-config and belongs in the same lock as the scene it
scopes.

**The filesystem is the single source of truth.** Files arrive over Samba, ssh
and `rsync` without the server's knowledge, so there is no sidecar index and no
manifest to rot. The cache is validated against `(size, mtime)` on every lookup
and discarded when it disagrees; `sha256` is computed lazily (on first request,
on scene-config save, on cache miss) and never required for a plain read. This is the
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

// The active project: what relative refs resolve against, where scene-config
// saves and event logs land. Server-global. `create` makes the directory if it
// does not exist; without it, an unknown name is PROJECT_NOT_FOUND.
message SetProjectRequest { string name = 1; bool create = 2; }
message GetProjectRequest {}
message GetProjectResponse { string name = 1; }

// Projects are directories, so they need no create command — uploading into one
// makes it. Listing and deleting are explicit.
message ListProjectsRequest {}
message ProjectInfo {
  string name        = 1;
  bool   reserved    = 2;  // leading underscore, or `default` / `demos`
  uint64 total_bytes = 3;
  map<string, uint32> counts_by_type = 4;
}
message ListProjectsResponse { repeated ProjectInfo projects = 1; }

// Refuses a non-empty project unless `recursive`. Never touches reserved projects.
message DeleteProjectRequest { string name = 1; bool recursive = 2; }
```

Errors get their own codes alongside the existing scene-config ones
(`service.proto:46`): `ASSET_NOT_FOUND`, `ASSET_EXISTS` (upload without
overwrite), `ASSET_INVALID_REF`, `ASSET_TOO_LARGE`, `ASSET_DIGEST_MISMATCH`,
`ASSET_UNSUPPORTED_TYPE`, `ASSET_READ_ONLY_TYPE` (upload into `logs/`),
`PROJECT_NOT_FOUND`, `PROJECT_RESERVED` (delete of `_session` and friends),
`PROJECT_NOT_EMPTY`.

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
ref (absolute — `_scratch` is never the project a scene-config is saved into). Consequences, all good:

- the texture cache has exactly one keying scheme (ref + digest), not two
- save/load round-trips for free — an inline-created stimulus saves as a normal
  asset reference
- identical inline uploads deduplicate by content hash
- `_scratch` is the one project the server may garbage-collect: on startup, any
  `_scratch` asset referenced by no scene-config and older than N
  days is deleted (default off; a rig-config knob).

---

## 7. Save / load semantics

A stimulus serialises its asset as:

```json
"source": { "ref": "images/id07_neutral.png",
            "sha256": "9f86d0…", "size": 240192 }
```

The ref is **relative** when it points inside the config's own project (§3) and
absolute otherwise, so the whole project directory can be renamed or copied to
another rig and still resolve. The digest and size are **advisory** — recorded at
save, checked at load, never used to locate the file.

On load:

| Situation | Behaviour |
|---|---|
| asset present, digest matches | normal |
| asset present, digest differs | load succeeds; **warning event** naming the ref, both digests |
| asset missing | the stimulus loads **disabled**, in an `unresolved` state, and a warning event names the missing ref. The rest of the scene-config loads normally |
| `LoadSceneConfigRequest.strict_assets = true` | any missing asset fails the whole load instead |

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
  whole project folder — the "install my study" one-liner. Plus
  `vstimd-client project list|show|use <name>|rm <name>`, where `use` sets the
  active project on the server.
- **Web:** a project picker in the header — it scopes every other panel, so the
  asset list, the scene-config list and the log list all show one study at a time —
  plus an asset section with drag-and-drop upload and a thumbnail grid. The web
  surface already speaks proto (`proto.rs` is at the crate root for exactly this
  reason).
- **Overlay:** the active project shown in the status line (an operator must be able
  to see, at the rig, which study is about to be recorded into), selectable from the
  config panel's project dropdown, and an Assets panel listing count + total size
  per type. Read-only otherwise: no upload from the overlay.

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

Each phase is a shippable PR; phases 0–2 are pure infrastructure and unblock #108.

| # | Phase | Depends on |
|---|---|---|
| 0 | **Naming + layout pass:** `scene-config` / `rig-config` everywhere, `--storage-dir`, scene-configs into `scene-configs/` (below) | — |
| 1 | `AssetRef` (absolute + relative) + `AssetStore` + project enumeration (scan, stat, read, write, delete) + unit tests. No proto, no GPU | — |
| 2 | `assets.proto`, `ipc/asset_commands.rs`, the active-project commands + `ProjectChanged` event, dispatcher wiring, error codes, integration tests via `SceneState::handle_request` | 1 |
| 3 | Python client + CLI (`asset`, `project`) + web project picker + overlay panel + docs | 2 |
| 4 | Scene-config round-trip: `source` serialisation, `unresolved` state, warning events, `strict_assets` | 2 |
| 5 | Texture cache + premultiplied upload — lands with #108 | 2 |
| 6 | Consumers: #108 image stimulus, #70 mesh textures, #109 scripts | 4, 5 |

### Phase 0 — the naming and layout pass

`CLAUDE.md` already rules that the two configs are always named: **rig-config**
(the rig's TOML in `/etc/braemons`) and **scene-config** (one experiment's JSON in
the storage dir). The code, the CLI and the wire only half-follow it, and the asset
store is about to add a third kind of file to the same directory. Fix it first, in
one mechanical PR, so nothing new is built on the ambiguous spelling.

Nothing beyond `v0.1.0` pre-releases has shipped, so this is a **hard rename**:
old spellings are removed outright, no aliases, no migration code, no deprecation
warnings. A stale `--config-dir` in a hand-written unit file fails at startup with
an unknown-flag error, which is the outcome we want.

**CLI**

| Today | Becomes |
|---|---|
| `--config-dir <path>` | `--storage-dir <path>` — the root; `projects/` is its only child |
| `--config <name>` | `--scene-config [<project>/]<name>` |
| — | `--project <name>` — the boot active project (also `[startup] project` in the rig-config) |
| `--rig-config <path>` | unchanged — already right |

**Server**

| Today | Becomes |
|---|---|
| `scene_config_file::DEFAULT_CONFIG_DIR` | `DEFAULT_STORAGE_DIR` |
| `main::resolve_config_dir` | `resolve_storage_dir` |
| `SceneState.runtime.config_dir` | `storage_dir`, plus the loaded project (see below) |
| `ipc/config_commands.rs` | `ipc/scene_config_commands.rs` |
| `scene_config_file::config_path(dir, name)` | `project_dir(storage, project).join("scene-configs").join(…)` |
| `DEMO_PREFIX = "demo_"` | the `demos` project — demo names lose the prefix |

**Wire and clients** — renamed too, since there is no compatibility to keep and
half-consistency is what got us here:

| Today | Becomes |
|---|---|
| `ListConfigsRequest`, `LoadConfigRequest`, `UploadConfigRequest`, `RetrieveConfigRequest` | `ListSceneConfigs…`, `LoadSceneConfig…`, `UploadSceneConfig…`, `RetrieveSceneConfig…` |
| `conn.config.*` (Python) | `conn.scene_config.*` |
| `vstimd-client config list\|save\|load\|get\|upload` | `vstimd-client scene-config …`, with `-p/--project` (default `default`) |
| web `src/config.ts` | `src/sceneConfig.ts` |

**Layout** — scene-configs move off the root of the storage dir into their project,
which also retires the `vstimd_` filename prefix: the prefix only ever existed to
keep them distinguishable in a shared directory, and a dedicated one does that
better.

```
vstimd_center_target.config.json  →  projects/default/scene-configs/center_target.config.json
vstimd_demo_gratings.config.json  →  projects/demos/scene-configs/gratings.config.json
vstimd__last_session.config.json  →  projects/_session/scene-configs/_last_session.config.json
```

`config_path()` (`scene_config_file.rs:26`) is the single place that layout is
defined, so the path change is small. The **project dimension** is the real work:
every scene-config command gains a project field, `SceneState` gains a
"currently-loaded project" (which is what relative asset refs resolve against and
what the event log picks its directory from), and the overlay's config panel, the
web UI and the CLI all need a project selector. That is the cost of this design and
it is worth naming: it is a bigger Phase 0 than a pure rename.

Docs and packaging follow:
`docs/concepts/saving-loading.md`, `docs/getting-started/demos.md`,
`docs/client/*.md`, `docs/operations/{appliance-setup,raspberry-pi-image,deployment}.md`,
the rig-config comments (`server/config/default-rig-config.toml:84`), the packaged
unit (`packaging/systemd/vstimd.service:45`) and the Samba share docs.

---

## 12. Open questions

1. **Upload over ZMQ REQ/REP vs. a side channel.** Chunked REQ/REP needs no new
   socket and no new port in the rig's firewall story, which is why it wins here.
   If installing a 10 000-frame movie proves painfully slow, the answer is a
   second ZMQ socket for bulk transfer, not a bigger chunk size. Measure first.
2. **Active project: global vs. per-connection.** Global is recommended above and
   is what the current stateless REQ/REP surface supports. If two-client rigs turn
   out to be common, the fix is a connection identity in the request envelope —
   a much larger change that should be driven by a real use case, not anticipated.
3. **Does loading a scene-config *always* switch the active project?** Convenient,
   and occasionally surprising if a client loads a config from another project just
   to inspect it. Suggest: yes, always, with the `ProjectChanged` event making it
   visible.
4. **`_scratch` GC policy** — off by default, or age-based by default?
5. **Does a project need metadata** (a description, a creation date, a `README`)?
   A plain `_project.toml` the server never parses is nearly free; skip until
   someone asks.
6. **Per-project quotas** — probably never; a single store-wide warn threshold is
   likely enough for a single-user rig. Event logs may change that answer.
7. **Digest algorithm** — sha256 for familiarity, or blake3 for speed on a
   Jetson? Only matters once assets are large; the field is a string either way.
