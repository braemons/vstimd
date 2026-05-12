"""Shared e2e test cases. Imported by test_e2e.py and test_e2e_null.py.

Each function receives a `conn` fixture from the importing test module,
so the same cases run against both a real and a null-renderer server.
"""

import time

import pytest

from vstimd import Connection
from vstimd.stimuli import GratingParams, RectParams, StimulusType
from vstimd._proto.vstimd.v1 import stimuli_2d_pb2 as _pb2


def test_create_rect(conn: Connection) -> None:
    handle = conn.stimuli.create_rect(x=0, y=0, width=100, height=100, r=1.0, g=0.0, b=0.0)
    assert handle > 0

    info = conn.stimuli.query(handle)
    assert info.stimulus_type == StimulusType.RECT
    assert isinstance(info.params, RectParams)
    assert info.params.width == pytest.approx(100.0, abs=0.5)
    assert info.params.height == pytest.approx(100.0, abs=0.5)
    assert info.fill_color.r == pytest.approx(1.0, abs=0.01)
    assert info.fill_color.g == pytest.approx(0.0, abs=0.01)
    assert info.fill_color.b == pytest.approx(0.0, abs=0.01)

    time.sleep(1.0)
    conn.stimuli.delete(handle)


def test_create_grating(conn: Connection) -> None:
    handle = conn.stimuli.create_grating(
        x=0, y=0, width=200, height=200, sf=0.05, phase=0.25, angle=45.0,
        contrast=0.8, r=0.0, g=1.0, b=0.0,
        waveform=_pb2.WAVEFORM_TYPE_SQR, mask=_pb2.MASK_TYPE_CIRCLE,
    )
    assert handle > 0

    info = conn.stimuli.query(handle)
    assert info.stimulus_type == StimulusType.GRATING
    assert isinstance(info.params, GratingParams)
    assert info.params.width == pytest.approx(200.0, abs=0.5)
    assert info.params.height == pytest.approx(200.0, abs=0.5)
    assert info.params.sf == pytest.approx(0.05, rel=1e-3)
    assert info.params.phase == pytest.approx(0.25, abs=0.01)
    assert info.params.contrast == pytest.approx(0.8, abs=0.01)
    assert info.params.waveform == _pb2.WAVEFORM_TYPE_SQR
    assert info.params.mask == _pb2.MASK_TYPE_CIRCLE

    conn.stimuli.delete(handle)


def test_grating_mutate_phase(conn: Connection) -> None:
    handle = conn.stimuli.create_grating(sf=0.05)
    conn.stimuli.set_grating_phase(handle, 0.5)

    info = conn.stimuli.query(handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.phase == pytest.approx(0.5, abs=0.01)

    conn.stimuli.delete(handle)


def test_grating_mutate_sf(conn: Connection) -> None:
    handle = conn.stimuli.create_grating(sf=0.05)
    conn.stimuli.set_grating_sf(handle, 0.1)

    info = conn.stimuli.query(handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.sf == pytest.approx(0.1, rel=1e-3)

    conn.stimuli.delete(handle)


def test_grating_mutate_contrast(conn: Connection) -> None:
    handle = conn.stimuli.create_grating(sf=0.05)
    conn.stimuli.set_grating_contrast(handle, 0.5)

    info = conn.stimuli.query(handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.contrast == pytest.approx(0.5, abs=0.01)

    conn.stimuli.delete(handle)


def test_grating_mutate_waveform(conn: Connection) -> None:
    handle = conn.stimuli.create_grating(waveform=_pb2.WAVEFORM_TYPE_SIN)
    conn.stimuli.set_grating_waveform(handle, _pb2.WAVEFORM_TYPE_SAW)

    info = conn.stimuli.query(handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.waveform == _pb2.WAVEFORM_TYPE_SAW

    conn.stimuli.delete(handle)


def test_grating_drift_speed(conn: Connection) -> None:
    handle = conn.stimuli.create_grating(sf=0.05, drift_speed=2.0)
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.drift_speed == pytest.approx(2.0, abs=0.01)
    assert info.params.drift_coupled is True

    conn.stimuli.set_grating_drift_speed(handle, 0.0)
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.drift_speed == pytest.approx(0.0, abs=0.01)

    conn.stimuli.delete(handle)


def test_grating_drift_decoupled(conn: Connection) -> None:
    handle = conn.stimuli.create_grating(sf=0.05, drift_decoupled=True, drift_angle=90.0)
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.drift_coupled is False
    assert info.params.drift_angle == pytest.approx(90.0, abs=0.1)

    conn.stimuli.set_grating_drift_decoupled(handle, False)
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.drift_coupled is True

    conn.stimuli.delete(handle)


def test_grating_visual(conn: Connection, step_delay: float) -> None:
    """Walk through all mutable grating parameters with a visible pause between steps.

    With step_delay > 0 a human observer can verify each change looks correct.
    With step_delay == 0 (null renderer) this still exercises every setter end-to-end.
    """

    def step() -> None:
        time.sleep(step_delay)

    conn.system.set_background(r=0.5, g=0.5, b=0.5)
    handle = conn.stimuli.create_grating(
        x=0, y=0, width=400, height=400,
        sf=0.05, phase=0.0, angle=0.0,
        contrast=1.0, r=1.0, g=1.0, b=1.0,
        waveform=_pb2.WAVEFORM_TYPE_SIN,
        mask=_pb2.MASK_TYPE_NONE,
    )
    assert handle > 0
    step()

    # spatial frequency
    conn.stimuli.set_grating_sf(handle, 0.02)
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.sf == pytest.approx(0.02, rel=1e-3)
    step()

    conn.stimuli.set_grating_sf(handle, 0.1)
    info = conn.stimuli.query(handle)
    assert info.params.sf == pytest.approx(0.1, rel=1e-3)
    step()

    # contrast
    conn.stimuli.set_grating_contrast(handle, 0.3)
    info = conn.stimuli.query(handle)
    assert info.params.contrast == pytest.approx(0.3, abs=0.01)
    step()

    conn.stimuli.set_grating_contrast(handle, 1.0)
    step()

    # phase
    conn.stimuli.set_grating_phase(handle, 0.25)
    info = conn.stimuli.query(handle)
    assert info.params.phase == pytest.approx(0.25, abs=0.01)
    step()

    conn.stimuli.set_grating_phase(handle, 0.75)
    step()

    # orientation
    conn.stimuli.set_orientation(handle, 45.0)
    step()

    conn.stimuli.set_orientation(handle, 90.0)
    step()

    # waveform
    conn.stimuli.set_grating_waveform(handle, _pb2.WAVEFORM_TYPE_SQR)
    info = conn.stimuli.query(handle)
    assert info.params.waveform == _pb2.WAVEFORM_TYPE_SQR
    step()

    conn.stimuli.set_grating_waveform(handle, _pb2.WAVEFORM_TYPE_SAW)
    info = conn.stimuli.query(handle)
    assert info.params.waveform == _pb2.WAVEFORM_TYPE_SAW
    step()

    conn.stimuli.set_grating_waveform(handle, _pb2.WAVEFORM_TYPE_SIN)
    step()

    # mask
    conn.stimuli.set_grating_mask(handle, _pb2.MASK_TYPE_CIRCLE)
    step()

    conn.stimuli.set_grating_mask(handle, _pb2.MASK_TYPE_GAUSS)
    step()

    conn.stimuli.set_grating_mask(handle, _pb2.MASK_TYPE_NONE)
    step()

    # drift — coupled to orientation (default)
    conn.stimuli.set_grating_drift_speed(handle, 1.0)
    info = conn.stimuli.query(handle)
    assert info.params.drift_speed == pytest.approx(1.0, abs=0.01)
    assert info.params.drift_coupled is True
    time.sleep(step_delay * 3)  # longer pause so drift is visible

    conn.stimuli.set_grating_drift_speed(handle, -1.0)
    time.sleep(step_delay * 3)

    # drift — decoupled from orientation
    conn.stimuli.set_grating_drift_decoupled(handle, True)
    conn.stimuli.set_grating_drift_angle(handle, 90.0)
    info = conn.stimuli.query(handle)
    assert info.params.drift_coupled is False
    assert info.params.drift_angle == pytest.approx(90.0, abs=0.1)
    time.sleep(step_delay * 3)

    # stop drift, recouple
    conn.stimuli.set_grating_drift_speed(handle, 0.0)
    conn.stimuli.set_grating_drift_decoupled(handle, False)
    info = conn.stimuli.query(handle)
    assert info.params.drift_speed == pytest.approx(0.0, abs=0.01)
    assert info.params.drift_coupled is True
    step()

    conn.stimuli.delete(handle)
    conn.system.set_background(r=0.0, g=0.0, b=0.0)
