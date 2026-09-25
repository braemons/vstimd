"""Unit tests for the 3-D models: every field survives the proto, both ways.

Values are binary fractions: the wire is float32, so 0.1 would come back as its
nearest float32 rather than as itself.
"""

from __future__ import annotations

from vstimd_client.stimuli import (
    Color,
    Corridor3DParams,
    Cube3DParams,
    Material3D,
    Plane3DParams,
    Repeat3D,
    Shading,
    Sphere3DParams,
    Transform3D,
    Vec2,
    Vec3,
)
from vstimd_client.system import Camera3D, Lighting3D


def test_transform_round_trip() -> None:
    t = Transform3D(
        position_cm=Vec3(1.0, 2.0, -3.0),
        rotation_deg=Vec3(30.0, -15.0, 5.0),
        scale=Vec3(1.0, 2.0, 0.5),
    )
    assert Transform3D.from_proto(t.to_proto()) == t


def test_default_transform_is_identity() -> None:
    t = Transform3D()
    assert (t.position_cm, t.rotation_deg, t.scale) == (
        Vec3(0, 0, 0),
        Vec3(0, 0, 0),
        Vec3(1, 1, 1),
    )


def test_material_round_trip_and_default() -> None:
    m = Material3D(albedo=Color(0.25, 0.5, 0.75, 0.5), emissive=Vec3(0.125, 0, 0), shading=Shading.PHONG)
    assert Material3D.from_proto(m.to_proto()) == m
    assert Material3D().albedo == Color(1.0, 1.0, 1.0), "the default surface is visible"
    assert Material3D().shading == Shading.UNLIT


def test_params_round_trip() -> None:
    m = Material3D(shading=Shading.PHONG)
    cube = Cube3DParams(size_cm=Vec3(20, 10, 5), material=m)
    sphere = Sphere3DParams(diameter_cm=7.5, rings=8, sectors=24, material=m)
    plane = Plane3DParams(size_cm=Vec2(100, 40), material=m)
    assert Cube3DParams.from_proto(cube.to_proto()) == cube
    assert Sphere3DParams.from_proto(sphere.to_proto()) == sphere
    assert Plane3DParams.from_proto(plane.to_proto()) == plane


def test_camera_and_lighting_round_trip() -> None:
    cam = Camera3D(position_cm=Vec3(0, 10, 30), yaw_deg=-20, pitch_deg=5, fov_y_deg=90)
    assert Camera3D.from_proto(cam.to_proto()) == cam
    light = Lighting3D(
        ambient_color=Vec3(0.125, 0.25, 0.0),
        sun_direction=Vec3(-1, 0, 0),
        sun_color=Vec3(2, 2, 2),
    )
    assert Lighting3D.from_proto(light.to_proto()) == light


def test_corridor_and_repeat_round_trip() -> None:
    corridor = Corridor3DParams(
        width_cm=80, height_cm=50, period_cm=200, periods_ahead=12, periods_behind=1,
        floor_color=Color(0.25, 0.25, 0.25), wall_color=Color(0.75, 0.5, 0.25),
        stripe_color=Color(0.125, 0.125, 0.125), ceiling=True, material=Material3D(shading=Shading.PHONG),
    )
    assert Corridor3DParams.from_proto(corridor.to_proto()) == corridor
    sphere = Sphere3DParams(repeat=Repeat3D(period_cm=200, ahead=12, behind=1))
    assert Sphere3DParams.from_proto(sphere.to_proto()) == sphere
    assert Sphere3DParams.from_proto(Sphere3DParams().to_proto()).repeat is None
