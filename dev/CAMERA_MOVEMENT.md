# Camera movement constraints (scene-config v6)

An optional `scene.camera_movement` config constrains camera motion driven by
LinearNav3D and DeviceDrivenTransform locally, before the camera is rendered.
It is domain configuration, loaded/retrieved through the existing scene-config
API; it introduces no protobuf type or RPC. Scenes without it remain v5. Scenes
with it must be v6: older daemons reject that version instead of silently
discarding collision rules. v5 and v6 are accepted by the updated daemon.

```
"camera_movement": {
  "start_position_cm": [0, 10, 0],
  "start_yaw_deg": 0,
  "clearance_cm": 1,
  "end_blackout_ms": 2000,
  "obstacles": [
    {"type":"Box", "position_cm":[0,15,-100],
     "yaw_deg":0, "size_cm":[40,30,4]},
    {"type":"Sphere", "position_cm":[20,20,-50], "diameter_cm":10}
  ]
}
```

Positions and full sizes use world centimetres, Y up, forward -Z at yaw zero.
Box, Cylinder and Diamond have `position_cm`, `yaw_deg`, `size_cm`.
Cylinder is vertical, circular in XZ (`size_cm[0] == size_cm[2]`). Diamond is
the extrusion along local Z of `abs(x)/(width/2)+abs(y)/(height/2) <= 1`.
Sphere has `position_cm` and `diameter_cm`.

The camera cannot cross the plane at start_position_cm behind the starting
horizontal heading. Obstacles are static, explicit scene volumes, independent
of render tessellation and shading. Floors are not obstacles; arches have
three separate boxes, preserving their openings. The editor exports one
analytic volume per landmark, including patterned landmarks.

The entire requested segment is swept against all obstacles, stopping at the
first contact with 0.001 cm separation. Input beyond a stop is discarded each
frame: reversing the wheel moves away immediately, with no accumulated debt.
No sliding, gravity, steering or path finding is introduced. Sphere clearance
is exact; box/cylinder/diamond clearance conservatively expands their planes
and radial extents. The declared starting position must be outside expanded
obstacles. Limit: 3000 obstacles, finite coordinates, positive sizes and margin.

Constraints apply to camera animation movement, not explicit SetCamera/scene
loads or finite-track return teleports. Such commands remain deliberate camera
placement operations. A return restores the declared valid start; the start
plane stays fixed across laps. Wrapping navigation is incompatible with these
constraints and refused when loading the config. Distance travelled reports
accepted movement, excluding blocked input and return jumps.

Additive loads involving a constrained scene are refused: colliders are a
complete scene snapshot. Use replace for constrained scenes.
Changes to rendered geometry via RPC do not regenerate the declared obstacles;
replace the scene-config to update both together.


`end_blackout_ms` (optional, default 0, maximum 60000) controls the return of
finite-track LinearNav3D in this constrained scene. When nonzero, reaching the
track end cuts the scene to black immediately, discards navigation input during
the hold, waits using a local monotonic Instant deadline, then restores the
starting position and heading and shows the scene immediately. This replaces
frame-counted fades for that scene; zero preserves the legacy fade behaviour.
The renderer clears black and omits scene stimuli and the photodiode during the
hold. Operator overlays remain available. No rig configuration is changed.
The end boundary is clamped before sweeping, so the camera stops at the end
plane; a collision before it still prevents reaching the end.
