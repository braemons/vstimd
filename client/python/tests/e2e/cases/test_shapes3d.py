"""E2E tests for 3-D stimuli (cube, sphere, plane, corridor), the camera and lighting."""

from __future__ import annotations

import pathlib
import struct

import pytest

from vstimd import Connection
from vstimd.exceptions import InvalidArgumentError, WrongStimulusTypeError
from vstimd.stimuli import (
    Color,
    Cube3DParams,
    Material3D,
    Plane3DParams,
    Shading,
    Sphere3DParams,
    StimulusType,
    Transform3D,
    Vec2,
    Vec3,
)
from vstimd.system import Camera3D, Lighting3D

from ._helpers import Stage, check_frame_stats

PHONG = Material3D(shading=Shading.PHONG)


@pytest.mark.onscreen(
    "3D-01",
    "a lit blue sphere in the middle, sitting on a grey floor that runs away "
    "from the viewer; the sphere is brightest on its upper right",
)
@check_frame_stats
def test_create_and_query_sphere(conn: Connection, stage: Stage) -> None:
    floor = conn.stimuli.shapes3d.create_plane(
        name="floor",
        transform=Transform3D(position_cm=Vec3(0, -12, -120)),
        params=Plane3DParams(size_cm=Vec2(120, 200), material=Material3D(
            albedo=Color(0.4, 0.4, 0.4), shading=Shading.PHONG)),
    )
    ball = conn.stimuli.shapes3d.create_sphere(
        name="ball",
        transform=Transform3D(position_cm=Vec3(0, 0, -60), rotation_deg=Vec3(30, -15, 0)),
        params=Sphere3DParams(diameter_cm=20, material=Material3D(
            albedo=Color(0.2, 0.5, 0.9), shading=Shading.PHONG)),
    )
    info = conn.stimuli.query(ball)
    assert info.stimulus_type == StimulusType.SPHERE_3D
    assert info.name == "ball"
    assert info.pos_px is None and info.rotation_deg is None
    assert info.transform_3d is not None
    assert info.transform_3d.position_cm == Vec3(0, 0, -60)
    assert info.transform_3d.rotation_deg.x == pytest.approx(30)
    assert info.transform_3d.rotation_deg.y == pytest.approx(-15)
    assert isinstance(info.params, Sphere3DParams)
    assert info.params.diameter_cm == pytest.approx(20)
    assert (info.params.rings, info.params.sectors) == (16, 32)
    assert info.params.material.shading == Shading.PHONG

    assert conn.stimuli.query(floor).stimulus_type == StimulusType.PLANE_3D
    assert {e.handle for e in conn.system.list_stimuli()} >= {floor, ball}
    stage.hold()
    conn.stimuli.delete(ball)
    conn.stimuli.delete(floor)


@pytest.mark.onscreen(
    "3D-02",
    "a red box in the middle that turns to face a new direction and stretches "
    "tall, then turns yellow and flat-shaded",
)
@check_frame_stats
def test_mutate_cube(conn: Connection, stage: Stage) -> None:
    cube = conn.stimuli.shapes3d.create_cube(
        transform=Transform3D(position_cm=Vec3(0, 0, -70), rotation_deg=Vec3(20, 20, 0)),
        params=Cube3DParams(size_cm=Vec3(15, 15, 15), material=Material3D(
            albedo=Color(0.9, 0.2, 0.2), shading=Shading.PHONG)),
    )
    stage.hold()
    conn.stimuli.shapes3d.set_transform(cube, Transform3D(
        position_cm=Vec3(0, 0, -70), rotation_deg=Vec3(-35, 10, 0)))
    conn.stimuli.shapes3d.set_cube_size(cube, Vec3(15, 30, 15))
    stage.hold()
    conn.stimuli.shapes3d.set_material(cube, Material3D(albedo=Color(0.9, 0.8, 0.2)))

    info = conn.stimuli.query(cube)
    assert info.transform_3d is not None
    assert info.transform_3d.rotation_deg.x == pytest.approx(-35)
    assert isinstance(info.params, Cube3DParams)
    assert info.params.size_cm == Vec3(15, 30, 15)
    assert info.params.material.shading == Shading.UNLIT
    stage.hold()
    conn.stimuli.delete(cube)


@pytest.mark.onscreen(
    "3D-03",
    "nothing new on screen: 2-D commands refuse a sphere, 3-D commands refuse a "
    "rect, a cube refuses a sphere's resize, and bad sizes are refused",
)
@check_frame_stats
def test_commands_check_type_and_dimension(conn: Connection, stage: Stage) -> None:
    ball = conn.stimuli.shapes3d.create_sphere(
        transform=Transform3D(position_cm=Vec3(0, 0, -5000)))
    rect = conn.stimuli.shapes.create_rect()
    with pytest.raises(WrongStimulusTypeError):
        conn.stimuli.set_position(ball, Vec2(10, 10))
    with pytest.raises(WrongStimulusTypeError):
        conn.stimuli.shapes3d.set_transform(rect, Transform3D())
    with pytest.raises(WrongStimulusTypeError):
        conn.stimuli.shapes3d.set_cube_size(ball, Vec3(1, 1, 1))
    with pytest.raises(InvalidArgumentError):
        conn.stimuli.shapes3d.set_sphere_diameter(ball, -1.0)
    conn.stimuli.shapes3d.set_sphere_diameter(ball, 5.0)
    info = conn.stimuli.query(ball)
    assert isinstance(info.params, Sphere3DParams)
    assert info.params.diameter_cm == pytest.approx(5.0)
    conn.stimuli.delete(ball)
    conn.stimuli.delete(rect)


@pytest.mark.onscreen(
    "3D-04",
    "a white cube straight ahead, then the view turns so the cube slides to the "
    "right edge and the scene goes dim and reddish",
)
@check_frame_stats
def test_camera_and_lighting(conn: Connection, stage: Stage) -> None:
    cube = conn.stimuli.shapes3d.create_cube(
        transform=Transform3D(position_cm=Vec3(0, 0, -60), rotation_deg=Vec3(25, 25, 0)),
        params=Cube3DParams(material=PHONG),
    )
    stage.hold()
    conn.system.set_camera(Camera3D(yaw_deg=20.0))
    conn.system.set_lighting(Lighting3D(
        ambient_color=Vec3(0.05, 0.0, 0.0), sun_color=Vec3(0.6, 0.2, 0.2)))

    cam = conn.system.query_camera()
    assert cam.yaw_deg == pytest.approx(20.0)
    assert (cam.fov_y_deg, cam.near_cm, cam.far_cm) == (60.0, 1.0, 50_000.0)
    light = conn.system.query_lighting()
    assert (light.sun_color.x, light.sun_color.y) == (pytest.approx(0.6), pytest.approx(0.2))

    with pytest.raises(InvalidArgumentError):
        conn.system.set_camera(Camera3D(near_cm=10.0, far_cm=5.0))
    with pytest.raises(InvalidArgumentError):
        conn.system.set_lighting(Lighting3D(sun_direction=Vec3(0, 0, 0)))
    stage.hold()
    conn.stimuli.delete(cube)


@pytest.mark.onscreen(
    "3D-06",
    "a floor with spheres along it; the camera glides forward and the scene "
    "loops every metre, with no visible jump",
)
@check_frame_stats
def test_camera_navigation(conn: Connection, stage: Stage) -> None:
    from vstimd.animations import AnimationState

    floor = conn.stimuli.shapes3d.create_plane(
        transform=Transform3D(position_cm=Vec3(0, -15, -200)),
        params=Plane3DParams(size_cm=Vec2(100, 500), material=Material3D(albedo=Color(0.3, 0.3, 0.3))),
    )
    # One sphere per metre, so a 100 cm wrap is seamless.
    balls = [
        conn.stimuli.shapes3d.create_sphere(
            transform=Transform3D(position_cm=Vec3(-25, -5, -100.0 * k)),
            params=Sphere3DParams(diameter_cm=10, material=PHONG),
        )
        for k in range(0, 5)
    ]
    nav = conn.animations.create_linear_nav_3d(50.0, wrap_period_cm=100.0, name="nav")
    conn.animations.arm(nav)
    conn.system.wait_for_frames(30)
    stage.hold(2.0)

    details = conn.animations.query(nav)
    assert details.camera and details.stimuli == ()
    assert details.state == AnimationState.RUNNING
    assert details.distance_travelled_cm > 0.0
    cam = conn.system.query_camera()
    assert 0.0 <= cam.position_cm.z < 100.0

    conn.animations.set_nav_speed(nav, 0.0)
    conn.system.wait_for_frames(2)
    held = conn.animations.query(nav).distance_travelled_cm
    conn.system.wait_for_frames(5)
    assert conn.animations.query(nav).distance_travelled_cm == pytest.approx(held)

    conn.animations.cancel(nav)
    conn.animations.delete(nav)
    for h in [*balls, floor]:
        conn.stimuli.delete(h)


@pytest.mark.onscreen(
    "3D-08",
    "a striped corridor the camera walks down; a red square in the corner shows "
    "only while the camera passes through the zone in each period",
)
@check_frame_stats
def test_camera_zone_drives_a_trigger_line(conn: Connection, stage: Stage) -> None:
    import time

    from vstimd.animations import VtlPolarity
    from vstimd.stimuli import Corridor3DParams, RectParams, ShapeAppearance
    from vstimd.system import CameraZone
    from vstimd.vtl import VtlHandle

    line = VtlHandle.input(3, 0)
    conn.stimuli.shapes3d.create_corridor(params=Corridor3DParams(periods_ahead=20, periods_behind=1))
    marker = conn.stimuli.shapes.create_rect(
        position_px=Vec2(400, 300),
        params=RectParams(width_px=80, height_px=80, appearance=ShapeAppearance(fill_color=Color(1, 0, 0))),
    )
    conn.system.set_camera_zones([CameraZone("marker", line, z_cm=(30.0, 70.0))])
    couple = conn.animations.create_couple_visibility_to_trigger_line(line, marker, polarity=VtlPolarity.ACTIVE_HIGH)
    conn.animations.arm(couple)
    conn.system.set_camera(Camera3D(position_cm=Vec3(0, 20, 99.0)))
    nav = conn.animations.create_linear_nav_3d(120.0, wrap_period_cm=100.0)
    conn.animations.arm(nav)

    saw_inside = saw_outside = False
    deadline = time.monotonic() + 5.0
    while time.monotonic() < deadline and not (saw_inside and saw_outside):
        conn.system.wait_for_frames(2)
        inside = conn.system.list_camera_zones()[0].inside
        shown = conn.stimuli.query(marker).anim_enabled
        if inside and shown:
            saw_inside = True
        if not inside and not shown:
            saw_outside = True
    assert saw_inside and saw_outside, (saw_inside, saw_outside)
    stage.hold()

    conn.animations.delete(nav)
    conn.animations.delete(couple)
    conn.system.set_camera_zones([])
    assert conn.system.list_camera_zones() == []
