# Scene Serialization / Deserialization

## Context

The server currently holds all scene state in RAM only. This change adds versioned JSON config files
that bundle the scene (stimuli, animations, background, photodiode) **and** I/O configuration (VTL
channel names; extensible to gamepad/key mappings later) in a single file. Files can be saved/loaded
via the egui overlay (pure egui — no native file dialog — so it works in DRM/headless mode) and via
`--config <path>` at startup. Two scene load modes: **replace** (clear then restore) and **additive**
(merge into existing scene with handle remapping). The I/O section is always fully replaced on load.

---

## Architecture: `SceneConfig` embedded in `SceneState`

The serializable fields live in a `SceneConfig` sub-struct embedded directly in `SceneState`.
`SceneConfig` **is** the file format — saving serializes `scene.config` directly, loading
deserializes directly into a `SceneConfig`. No wrapper type needed.

The non-serializable runtime fields (`Instant`, `tokio::watch`, command log, etc.) live in a
companion `SceneRuntimeState`. `SceneState` implements `Deref<Target = SceneConfig>` so all
existing field access (`scene.stimuli`, `scene.background`, …) continues to work unchanged.

```rust
// scene/config.rs  (new file)
#[derive(Clone, Serialize, Deserialize)]
pub struct SceneConfig {
    pub version:          u32,                   // = 1; checked on load
    pub background:       Deferred<[f32; 4]>,
    pub default_fill:     [f32; 4],
    pub default_outline:  [f32; 4],
    pub photodiode:       PhotoDiodeState,
    pub stimuli:          IndexMap<u32, StimulusEntry>,
    pub next_stim_handle: u32,
    pub animations:       IndexMap<u32, AnimationEntry>,
    pub next_anim_handle: u32,
    pub io:               IoConfig,              // VTL names + future I/O mappings
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct IoConfig {
    pub vtl_names: Vec<VtlNameEntry>,
    // future: gamepad_mappings, key_bindings, …
}

pub const CONFIG_VERSION: u32 = 1;
```

```rust
// scene/state.rs
pub struct SceneRuntimeState {
    pub deferred_mode:      bool,
    pub pending_flip:       bool,
    pub frame_rate:         f32,
    pub screen_size:        Option<(u32, u32)>,
    pub last_uploaded_size: (u32, u32),
    pub error_mask:         u16,
    pub error_code:         i16,
    pub command_log:        VecDeque<CommandEntry>,
    pub command_log_total:  u64,
    pub command_log_errors: u64,
    pub server_start:       Instant,
    pub frame_count:        u64,
    pub frame_notifier:     Arc<tokio::sync::watch::Sender<u64>>,
}

pub struct SceneState {
    pub config:  SceneConfig,
    pub runtime: SceneRuntimeState,
}

impl Deref    for SceneState { type Target = SceneConfig; fn deref    (&self)    -> &SceneConfig { &self.config } }
impl DerefMut for SceneState {                             fn deref_mut(&mut self) -> &mut SceneConfig { &mut self.config } }
```

**Migration impact:** All existing code using `self.stimuli`, `self.background`, etc. compiles
as-is via `DerefMut`. Only `SceneState::new()` needs to change to initialise the two sub-structs.

### `Deferred<T>` — transparent in JSON

Add custom serde to `Deferred<T>` that serializes only the `live` field and deserializes by
setting both `live` and `copy` to the loaded value:

```rust
impl<T: Serialize + Copy + Default> Serialize for Deferred<T> {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error> { self.live.serialize(s) }
}
impl<'de, T: Deserialize<'de> + Copy + Default> Deserialize<'de> for Deferred<T> {
    fn deserialize<D>(d: D) -> Result<Self, D::Error> {
        let v = T::deserialize(d)?;
        Ok(Deferred { live: v, copy: v })
    }
}
```

This makes all `Deferred` fields transparent in JSON (e.g. `"background": [0,0,0,1]`).

---

## File Naming Convention

Files saved from vstimd follow the pattern `vstimd_<name>.<type>.ext`:

| File | Convention | Example |
|------|-----------|---------|
| Config (scene + I/O) | `vstimd_<name>.config.json` | `vstimd_motion_exp.config.json` |
| Archive (config + assets) | `vstimd_<name>.archive.zip` | `vstimd_motion_exp.archive.zip` |
| Event log | `vstimd_<name>.events.sqlite` | `vstimd_motion_exp.events.sqlite` |

The egui file browser enforces this when saving (auto-appends `vstimd_` prefix and `.config.json`
suffix if absent). The ZMQ protocol accepts any path — callers are responsible for the convention.

---

## File I/O

```rust
// scene/config.rs
impl SceneConfig {
    pub fn save_to_file(&self, path: &Path) -> std::io::Result<()> {
        std::fs::write(path, serde_json::to_string_pretty(self)?)
    }

    pub fn load_from_file(path: &Path) -> anyhow::Result<Self> {
        let s = std::fs::read_to_string(path)?;
        let cfg: SceneConfig = serde_json::from_str(&s)?;
        anyhow::ensure!(cfg.version == CONFIG_VERSION,
            "Unsupported config version {} (expected {})", cfg.version, CONFIG_VERSION);
        Ok(cfg)
    }
}
```

**Save** is just:
```rust
scene.read().config.save_to_file(&path)
```
(VTL names are already inside `config.io` — no assembly required.)

**After load**, propagate the I/O section and apply to scene:
```rust
let cfg = SceneConfig::load_from_file(&path)?;
if let Some(vtl) = vtl {
    vtl.lock().names = cfg.io.vtl_names.clone();
    vtl.lock().sync_names_to_shm();
}
scene.write().load_snapshot(cfg, load_mode);
```

---

## Load Modes

```rust
pub enum LoadMode { Replace, Additive }

impl SceneState {
    pub fn load_snapshot(&mut self, cfg: SceneConfig, mode: LoadMode) {
        match mode {
            LoadMode::Replace => {
                self.config = cfg;
                self.fixup_after_load();
            }
            LoadMode::Additive => {
                // Remap handles to avoid collision with existing scene
                let stim_offset = self.config.next_stim_handle;
                let anim_offset = self.config.next_anim_handle;
                for (handle, entry) in cfg.stimuli {
                    self.config.stimuli.insert(handle + stim_offset, entry_dirty(entry));
                }
                for (handle, mut anim) in cfg.animations {
                    for sh in &mut anim.stimuli { *sh += stim_offset; }
                    anim.state = AnimState::Idle;
                    self.config.animations.insert(handle + anim_offset, anim);
                }
                self.config.next_stim_handle += cfg.next_stim_handle;
                self.config.next_anim_handle += cfg.next_anim_handle;
                // background/photodiode/io not merged in additive mode
            }
        }
    }

    fn fixup_after_load(&mut self) {
        for entry in self.config.stimuli.values_mut() {
            entry.stimulus.flags_mut().dirty = true;
            entry.stimulus.reset_phase_accum();
            entry.stimulus.make_copy();
        }
        for anim in self.config.animations.values_mut() {
            anim.state = AnimState::Idle;
            anim.captured_user_enabled = None;
        }
        self.config.background.make_copy();
        self.config.photodiode.make_copy();
    }
}
```

---

## Serde Derives — Files to Touch

Add `#[derive(Serialize, Deserialize)]` to:

| File | Types |
|------|-------|
| `scene/deferred.rs` | `Deferred<T>` — custom serde (live only, see above) |
| `scene/stimulus/stimulus_flags.rs` | `StimulusFlags` — skip `dirty`; restore with `dirty: true` |
| `scene/stimulus/transform2d.rs` | `Transform2D` |
| `scene/stimulus/shape_appearance.rs` | `ShapeAppearance`, `DrawMode` |
| `scene/stimulus/primitive_shapes.rs` | `RectStimulus`, `CircleStimulus`, `EllipseStimulus` |
| `scene/stimulus/grating/grating_params.rs` | `GratingParams`, `Waveform`, `GratingMask` |
| `scene/stimulus/grating/grating_stimulus.rs` | `GratingStimulus` — `#[serde(skip, default)]` on `phase_accum` |
| `scene/stimulus/text/text_params.rs` | `TextRenderParams`, `Anchor`, `LanguageStyle` |
| `scene/stimulus/text/text_stimulus.rs` | `TextStimulus` — `#[serde(skip, default)]` on `text_copy` |
| `scene/stimulus/mod.rs` | `Stimulus`, `ShapeStimulus` |
| `scene/stimulus/entry.rs` (or mod.rs) | `StimulusEntry` |
| `scene/animation.rs` | `Animation`, `AnimState`, `StartAction`, `FinalAction`, `Edge`, `AnimationEntry` |
| `scene/photodiode.rs` | `PhotoDiodeState` |
| `vtl_state.rs` | `VtlNameEntry`, `VtlBit`, `Edge` |

`bitflags!` types (`StartAction`, `FinalAction`) — use `#[serde(transparent)]` wrapping the u8.
`uuid` needs the `"serde"` feature. `indexmap` needs the `"serde"` feature.

If `vtl::Direction` lacks serde (external crate), wrap it:
```rust
#[derive(Serialize, Deserialize)]
#[serde(remote = "vtl::Direction")]
enum DirectionDef { In, Out }   // mirror the actual variants
```

---

## CLI `--config <path>` (main.rs)

```rust
struct Args {
    render_target: RenderTarget,
    verbose: bool,
    config_file: Option<PathBuf>,   // --config / -c
}
```

After `SceneState::new()`, before `spawn_zmq_thread()`:
```rust
if let Some(ref path) = args.config_file {
    match SceneConfig::load_from_file(path) {
        Ok(cfg) => {
            if let Some(ref vtl) = vtl {
                let mut v = vtl.lock().unwrap();
                v.names = cfg.io.vtl_names.clone();
                v.sync_names_to_shm();
            }
            scene.write().unwrap().load_snapshot(cfg, LoadMode::Replace);
        }
        Err(e) => log::error!("Failed to load config {path:?}: {e}"),
    }
}
```

---

## egui File Browser (`render/file_browser.rs` — new file)

A self-contained egui modal, no native OS dialog, works in DRM mode.

```rust
pub enum BrowserMode { Save, OpenReplace, OpenAdditive }

pub struct FileBrowser {
    pub open: bool,
    mode: BrowserMode,
    current_dir: PathBuf,
    entries: Vec<DirEntry>,    // sorted: dirs first, then *.config.json
    filename: String,
    pub result: Option<(BrowserMode, PathBuf)>,
}
```

UI layout (inside `egui::Window`):
- Path breadcrumbs (clickable directory segments)
- Scrollable `egui::Grid` of entries: `📁 dirname` / `📄 filename.config.json`
- `TextEdit` for filename (editable; auto-filled on selection; save mode auto-appends `.config.json` if missing, and `vstimd_` prefix if absent)
- `[Open]` / `[Save]` + `[Cancel]` buttons

File filter: `*.config.json` (parent dirs always shown).

---

## egui Overlay Integration (`render/overlay.rs`)

New `egui::Window::new("Config")`:

```rust
egui::Window::new("Config").show(ctx, |ui| {
    ui.horizontal(|ui| {
        if ui.button("Save…").clicked()             { args.file_browser.open(BrowserMode::Save); }
        if ui.button("Load (replace)…").clicked()  { args.file_browser.open(BrowserMode::OpenReplace); }
        if ui.button("Load (additive)…").clicked() { args.file_browser.open(BrowserMode::OpenAdditive); }
    });
});
args.file_browser.show(ctx);

if let Some((mode, path)) = args.file_browser.take_result() {
    match mode {
        BrowserMode::Save => {
            // VTL names already live in scene.config.io — just sync from VtlState first
            if let Some(vtl) = args.vtl.as_ref() {
                scene.write().unwrap().config.io.vtl_names = vtl.lock().unwrap().names.clone();
            }
            if let Err(e) = scene.read().unwrap().config.save_to_file(&path) {
                log::error!("{e}");
            }
        }
        BrowserMode::OpenReplace | BrowserMode::OpenAdditive => {
            let load_mode = if matches!(mode, BrowserMode::OpenReplace) {
                LoadMode::Replace } else { LoadMode::Additive };
            match SceneConfig::load_from_file(&path) {
                Ok(cfg) => {
                    if let Some(vtl) = args.vtl.as_ref() {
                        let mut v = vtl.lock().unwrap();
                        v.names = cfg.io.vtl_names.clone();
                        v.sync_names_to_shm();
                    }
                    scene.write().unwrap().load_snapshot(cfg, load_mode);
                }
                Err(e) => log::error!("{e}"),
            }
        }
    }
}
```

`OverlayArgs` gains `file_browser: &mut FileBrowser` and `vtl: Option<&Arc<Mutex<VtlState>>>`.
`FileBrowser` lives on `RenderState`.

---

## Cargo.toml Changes

```toml
serde      = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow     = "1"                                         # if not already present
uuid       = { version = "1", features = ["v4", "serde"] }
indexmap   = { version = "2", features = ["serde"] }
```

---

## Files Created / Modified

**New files:**
- `server/src/scene/config.rs` — `SceneConfig`, `IoConfig`, `SceneRuntimeState`, `CONFIG_VERSION`, save/load
- `server/src/render/file_browser.rs` — egui file browser widget

**Modified files (main changes):**
- `server/Cargo.toml` — add serde, serde_json, anyhow; add features to uuid + indexmap
- `server/src/main.rs` — `--config` arg + startup load
- `server/src/scene/mod.rs` — expose `config` module; re-export key types + `LoadMode`
- `server/src/scene/state.rs` — restructure `SceneState` (embed `config: SceneConfig`, `runtime: SceneRuntimeState`), add `Deref`, `load_snapshot`, `fixup_after_load`
- `server/src/scene/deferred.rs` — custom `Serialize`/`Deserialize` for `Deferred<T>`
- `server/src/scene/animation.rs` — serde derives on all animation types
- `server/src/scene/photodiode.rs` — serde derives
- `server/src/scene/stimulus/{mod,entry,stimulus_flags,transform2d,shape_appearance,primitive_shapes}.rs` — serde derives
- `server/src/scene/stimulus/grating/{grating_params,grating_stimulus}.rs` — serde derives
- `server/src/scene/stimulus/text/{text_params,text_stimulus}.rs` — serde derives
- `server/src/vtl_state.rs` — serde derives on `VtlNameEntry`, `VtlBit`, `Edge`
- `server/src/render/overlay.rs` — Config window + file browser integration
- `server/src/render/mod.rs` — expose `file_browser` module
- `proto/vstimd/v1/service.proto` — add `SaveConfigRequest`/`LoadConfigRequest` to `Request.body`; add 4 new `ErrorCode` values
- `proto/vstimd/v1/system.proto` — define `SaveConfigRequest` and `LoadConfigRequest` messages
- `server/src/scene/command.rs` — dispatch `SaveConfig` and `LoadConfig` arms in `handle_system_command`

---

## Proto: Save / Load Commands

### New messages (`system.proto`)

```protobuf
// Save the current scene + I/O config to a file on the server's filesystem.
// path should follow the vstimd_<name>.config.json convention.
message SaveConfigRequest {
  string path = 1;
}

// Load a config file from the server's filesystem.
// additive=false: clear existing scene then restore (default)
// additive=true:  merge stimuli/animations into the current scene;
//                 I/O config (VTL names etc.) is always fully replaced.
message LoadConfigRequest {
  string path     = 1;
  bool   additive = 2;
}
```

### New fields in `Request.body` oneof (`service.proto`, system target)

```protobuf
// ── Config persistence (system target) ──────────────────────────────────────
SaveConfigRequest save_config = 110;
LoadConfigRequest load_config = 111;
```

### New error codes (`service.proto`)

```protobuf
// The specified file path does not exist or cannot be opened.
ERROR_CODE_FILE_NOT_FOUND      = 10;
// A filesystem error occurred (permission denied, disk full, etc.).
ERROR_CODE_FILE_IO             = 11;
// The file is not valid JSON or fails schema validation.
ERROR_CODE_FILE_FORMAT         = 12;
// The file's version field is not supported by this server.
ERROR_CODE_UNSUPPORTED_VERSION = 13;
```

### Dispatch (`command.rs`)

`handle_system_command` gains two new arms:

```rust
request::Body::SaveConfig(r) => {
    // Sync live VTL names into config.io before saving
    if let Some(vtl) = vtl.as_ref() {
        self.config.io.vtl_names = vtl.lock().names.clone();
    }
    match self.config.save_to_file(Path::new(&r.path)) {
        Ok(()) => ok_ack(),
        Err(e) if e.kind() == NotFound => err(ErrorCode::FileNotFound, &e.to_string()),
        Err(e) => err(ErrorCode::FileIo, &e.to_string()),
    }
}
request::Body::LoadConfig(r) => {
    match SceneConfig::load_from_file(Path::new(&r.path)) {
        Ok(cfg) => {
            if let Some(vtl) = vtl {
                vtl.lock().names = cfg.io.vtl_names.clone();
                vtl.lock().sync_names_to_shm();
            }
            let mode = if r.additive { LoadMode::Additive } else { LoadMode::Replace };
            self.load_snapshot(cfg, mode);
            ok_ack()
        }
        Err(e) if is_not_found(&e) => err(ErrorCode::FileNotFound, &e.to_string()),
        Err(e) if is_format(&e)    => err(ErrorCode::FileFormat, &e.to_string()),
        Err(e)                     => err(ErrorCode::FileIo, &e.to_string()),
    }
}
```

`handle_request` already takes `vtl: Option<&mut VtlState>` — pass it through to these arms.

---

## Future: Archive Format (`vstimd_<name>.archive.zip`)

When stimuli that reference external assets (images, movies, 3-D models) are added, the config
file alone will not be self-contained. A future archive format will bundle:

- The `vstimd_<name>.config.json` file (scene + I/O config)
- All referenced asset files under an `assets/` directory inside the ZIP

Asset paths in `SceneConfig` will be stored as relative `Option<PathBuf>` fields on future stimulus
types; on load they resolve against the extract directory. The JSON format is unchanged — assets are
additional fields. The archive wraps the same JSON.

The egui file browser will support both `*.config.json` and `*.archive.zip` when implemented.

---

## Verification

1. `cargo build --release` — no compile errors
2. `cargo clippy` — no new warnings
3. Start server (null renderer); create stimuli + animations via Python client; open overlay → "Save…" → inspect `vstimd_test.config.json` (check `version=1`, `scene.*` fields and `io.vtl_names` present)
4. Restart server with `--config <file>` → verify stimuli reappear with correct positions and appearances; VTL names restored
5. Load at runtime via overlay (replace mode) → scene replaced, dirty flags trigger retessellation
6. Load at runtime via overlay (additive mode) → new stimuli appended, no handle collisions
7. Verify animation stimulus handle references remapped correctly in additive mode
8. Send `SaveConfigRequest{path: "vstimd_test.config.json"}` via Python client → file appears, JSON valid
9. Send `LoadConfigRequest{path: "...", additive: false}` → scene restored; bad path → `ERROR_CODE_FILE_NOT_FOUND`; bad JSON → `ERROR_CODE_FILE_FORMAT`
10. `make test-e2e-null` — existing tests still pass
