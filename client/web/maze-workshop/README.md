# Maze Workshop

Maze Workshop is a visual scene editor for vstimd. Production builds serve it at
`http://<rig>:8080/maze-workshop/`, linked from the control panel. Vite injects
`src/workshop.ts`, which supplies the official web client to the editor. No
additional daemon, port, Python process, CDN or browser framework is needed.

The editor stays independent of the transport. Its hosted adapter uses
`SceneConfigClient.upload` (save only, no apply) and `load` (replace, never
additive) over `/ws`. The JSON scene document remains separate from protobuf.
Wheel loading checks the live input registry: cumulative axis, scale 1,
connected and non-stale. Calibration remains owned by mousewheeld.

Choose navigation in **Export to vstimd**, enter project/name, then **Save in
vstimd**. Existing names require the explicit overwrite checkbox. **Load on
display (replace)** becomes available only for the saved design and settings;
editing either requires saving again. Connection failure is reported, with no
automatic mutation retries. **Save project** still downloads editable `.maze.json`;
server-side persistence of editable projects is deferred. The saved scene is
available in vstimd's normal scene browser.

This editor exports scene-config v6, requiring the camera movement support in
this tree. Older daemons reject it. Return-at-end uses 2000 ms of black, then the
initial camera position and heading. Patterned landmark rendering remains a
coarse geometry approximation. The browser preview is illustrative, not Vulkan
or physical timing validation. Closing the editor does not stop vstimd.

The floor plan supports walls, floors, arches and landmarks, with selection,
resize, copy/paste, undo/redo, repeated wall panels and measurements in cm.
Templates include a corridor, a T-maze and an empty canvas. Wheel export drives
a straight track without steering; drawing a T-maze does not add branching
navigation. The editor does not control trials, rewards or other daemons.

Keep the two file types distinct:

| File | Purpose |
| --- | --- |
| `.maze.json` | Editable project, reopened with **Open project**. |
| `.config.json` | Exported vstimd scene, saved/loaded through the scene-config API. It cannot be imported as an editable project. |

The divider above **3D preview** can be dragged vertically to resize the preview
while leaving usable floor-plan space. Double-click resets its height. With the
divider focused, Up/Down resize (Shift for larger steps), Home/End select minimum /
maximum. The height is a browser preference, independent of editable projects
and scene exports, and adjusts when the window becomes smaller.

## Build and check

From `client/web`: `make build` builds both pages and the console element bundle.
`make typecheck` checks the public client. The root Makefile includes workshop
sources in web dependencies. `make build` at the vstimd root embeds `dist/`.

Within this directory, `make build check` produces an offline `editor.html` and
runs the editor tests. The standalone file works without the API; hosted save /
load controls only appear when the web adapter is supplied by vstimd.
All sources and assets are contained in this repository; no external workspace
folders are required to build or package the editor.

API integration tests live in `client/web/tests/workshop.test.ts`; set
`VSTIMD_BIN` to an embed-ui build before `make test-e2e`. They launch an isolated
null server and test routes/assets, save without apply, replace loading, duplicate
protection, input checks and reconnect after a closed channel. This does not
validate encoder hardware, Vulkan rendering or real presentation timing.

For example, from the repository root (Node/npm and Rust required):

```sh
make -C client/web/maze-workshop build check
make build
VSTIMD_BIN="$PWD/target/release/vstimd" make -C client/web test
```

On macOS the existing VTL integration tests may fail because shared-memory
semaphores are unavailable; report those separately from Workshop results.
CI builds the embedded web bundle and runs the editor and API checks on Linux.

## Source map

| Source | Responsibility |
| --- | --- |
| `index.html`, `style.css`, `app.mjs` | Layout, user interaction and project history. |
| `model.mjs`, `landmarks.mjs`, `wheel.mjs` | Project validation, geometry and scene export. |
| `collisions.mjs`, `return.mjs` | Preview movement limits and return hold. |
| `preview.mjs`, `surfaces.mjs`, `surface-preview.mjs` | Software preview, depth and analytic landmark surfaces. |
| `measurements.mjs`, `navigation.mjs` | Plan measurements and minimap coordinates. |
| `resizer.mjs`, `shortcuts.mjs` | Preview height and keyboard help. |
| `../src/workshop.ts`, `../src/workshopApi.ts` | Hosted adapter using the public web client. |
| `build.mjs` | Self-contained offline HTML generator. |

The native movement contract and its limits are documented in
[`dev/CAMERA_MOVEMENT.md`](../../../dev/CAMERA_MOVEMENT.md). Native collision
handling stays in the scene/animation engine; browser requests are outside the
frame timing path.
