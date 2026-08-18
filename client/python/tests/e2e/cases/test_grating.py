"""E2E tests for grating stimuli."""
from __future__ import annotations

import pytest

from vstimd import Connection
from vstimd.stimuli import GratingMask, GratingParams, GratingTexture, StimulusType
from vstimd.stimuli.stimuli_models import Color, Vec2

from ._helpers import Stage


@pytest.mark.onscreen(
    "GRAT-01",
    "a 200×200 px green square-wave grating in the centre, tilted 45°, "
    "circular-masked so it reads as a disc of hard-edged stripes",
)
def test_create_grating(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.grating.create_grating(
        position_px=Vec2(0, 0),
        rotation_deg=45.0,
        params=GratingParams(
            width_px=200,
            height_px=200,
            sf_cycles_per_px=0.05,
            phase_cycles=0.25,
            contrast=0.8,
            fore_color=Color(0.0, 1.0, 0.0),
            waveform=GratingTexture.SQR,
            mask=GratingMask.CIRCLE,
        ),
    )
    assert handle > 0

    info = conn.stimuli.query(handle)
    assert info.stimulus_type == StimulusType.GRATING
    assert isinstance(info.params, GratingParams)
    assert info.params.width_px == pytest.approx(200.0, abs=0.5)
    assert info.params.height_px == pytest.approx(200.0, abs=0.5)
    assert info.params.sf_cycles_per_px == pytest.approx(0.05, rel=1e-3)
    assert info.params.phase_cycles == pytest.approx(0.25, abs=0.01)
    assert info.params.contrast == pytest.approx(0.8, abs=0.01)
    assert info.params.waveform == GratingTexture.SQR
    assert info.params.mask == GratingMask.CIRCLE

    stage.hold()
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "GRAT-02",
    "a centred grating whose stripes shift sideways by half a cycle when the "
    "phase jumps from 0 to 0.5 — light bars land where dark ones were",
)
def test_grating_mutate_phase(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.grating.create_grating(params=GratingParams(sf_cycles_per_px=0.05))
    stage.step("phase_cycles = 0", hold=0.5)

    conn.stimuli.grating.set_phase(handle, 0.5)
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.phase_cycles == pytest.approx(0.5, abs=0.01)

    stage.step("phase_cycles = 0.5 — the stripes have swapped places")
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "GRAT-03",
    "a centred grating whose stripes double in number when the spatial "
    "frequency goes from 0.05 to 0.1 cycles/px — bars get half as wide",
)
def test_grating_mutate_sf(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.grating.create_grating(params=GratingParams(sf_cycles_per_px=0.05))
    stage.step("0.05 cycles/px — wide bars", hold=0.5)

    conn.stimuli.grating.set_sf(handle, 0.1)
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.sf_cycles_per_px == pytest.approx(0.1, rel=1e-3)

    stage.step("0.1 cycles/px — twice as many, half as wide")
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "GRAT-04",
    "a centred grating that fades from full contrast to 0.5 — same stripes, "
    "visibly greyer, neither black nor white at the extremes",
)
def test_grating_mutate_contrast(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.grating.create_grating(params=GratingParams(sf_cycles_per_px=0.05))
    stage.step("contrast 1.0 — full black-to-white swing", hold=0.5)

    conn.stimuli.grating.set_contrast(handle, 0.5)
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.contrast == pytest.approx(0.5, abs=0.01)

    stage.step("contrast 0.5 — washed out towards mid grey")
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "GRAT-05",
    "a centred grating whose profile changes from a smooth sinusoid to a "
    "sawtooth — soft gradients replaced by ramps with a hard edge per cycle",
)
def test_grating_mutate_waveform(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.grating.create_grating(
        params=GratingParams(waveform=GratingTexture.SIN),
    )
    stage.step("SIN — smoothly graded stripes", hold=0.5)

    conn.stimuli.grating.set_waveform(handle, GratingTexture.SAW)
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.waveform == GratingTexture.SAW

    stage.step("SAW — ramps, one hard edge per cycle")
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "GRAT-06",
    "a full square patch of stripes that becomes a circular disc of stripes "
    "when the CIRCLE mask is applied — the corners are cut away",
)
def test_grating_set_mask(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.grating.create_grating(params=GratingParams(mask=GratingMask.NONE))
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.mask == GratingMask.NONE
    stage.step("mask NONE — square patch with corners", hold=0.5)

    conn.stimuli.grating.set_mask(handle, GratingMask.CIRCLE)
    info = conn.stimuli.query(handle)
    assert info.params.mask == GratingMask.CIRCLE

    stage.step("mask CIRCLE — the same stripes inside a disc")
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "GRAT-07",
    "a centred grating drifting at 2 Hz (stripes marching sideways), which "
    "then freezes when the drift speed is set to 0",
)
def test_grating_drift_speed(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.grating.create_grating(
        params=GratingParams(sf_cycles_per_px=0.05, drift_speed_hz=2.0)
    )
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.drift_speed_hz == pytest.approx(2.0, abs=0.01)
    assert info.params.drift_coupled is True
    stage.step("drifting at 2 Hz — stripes moving")

    conn.stimuli.grating.set_drift_speed(handle, 0.0)
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.drift_speed_hz == pytest.approx(0.0, abs=0.01)

    stage.step("drift speed 0 — the stripes have stopped dead")
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "GRAT-08",
    "a static centred grating: drift is decoupled from the stripe orientation "
    "at 90°, then recoupled. Speed is 0 throughout, so nothing moves",
)
def test_grating_drift_decoupled(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.grating.create_grating(
        params=GratingParams(sf_cycles_per_px=0.05, drift_coupled=False, drift_angle_deg=90.0),
    )
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.drift_coupled is False
    assert info.params.drift_angle_deg == pytest.approx(90.0, abs=0.1)
    stage.step("decoupled drift direction, 90° — still frozen at speed 0", hold=0.5)

    conn.stimuli.grating.set_drift_decoupled(handle, False)
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.drift_coupled is True

    stage.step("recoupled to the stripe orientation — still frozen")
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "GRAT-09",
    "on mid-grey, a row of grating patches sweeping one parameter at a time "
    "(spatial frequency, contrast, phase, orientation, waveform, mask), then a "
    "single large patch drifting forwards, backwards and sideways",
)
def test_grating_visual(conn: Connection, stage: Stage) -> None:
    """Display grating parameter variations sequentially, one row at a time."""
    PATCH_W, PATCH_H = 200, 150
    COL_STEP = 230

    _SF       = 0.05
    _WAVEFORM = GratingTexture.SIN
    _MASK     = GratingMask.NONE

    ROWS: list[tuple[str, list[dict]]] = [
        ("spatial frequency — five patches, stripes getting finer to the right",
         [{"sf_cycles_per_px": sf_cycles_per_px} for sf_cycles_per_px in [0.01, 0.03, 0.05, 0.07, 0.10]]),
        ("contrast — five patches, faintest on the left, strongest on the right",
         [{"contrast": c} for c in [0.2, 0.4, 0.6, 0.8, 1.0]]),
        ("phase — five identical patches, stripes offset a quarter cycle each step",
         [{"phase_cycles": p} for p in [0.0, 0.25, 0.5, 0.75, 1.0]]),
        # `rotation_deg` is the placement's, not a params field — see the split below.
        ("orientation — five patches rotating 0°, 45°, 90°, 135°, 180°",
         [{"rotation_deg": a} for a in [0.0, 45.0, 90.0, 135.0, 180.0]]),
        ("waveform — sine, square, sawtooth, triangle, left to right",
         [{"waveform": w} for w in [
            GratingTexture.SIN, GratingTexture.SQR,
            GratingTexture.SAW, GratingTexture.TRI,
        ]]),
        ("mask — none, circle, gaussian, hann, raised cosine, left to right",
         [{"mask": m} for m in [
            GratingMask.NONE, GratingMask.CIRCLE,
            GratingMask.GAUSS, GratingMask.HANN, GratingMask.RAISED_COS,
        ]]),
    ]

    conn.system.set_background(r=0.4, g=0.4, b=0.4)

    for row_name, patches in ROWS:
        n = len(patches)
        xs = [(j - (n - 1) / 2) * COL_STEP for j in range(n)]
        handles: list[int] = []

        for x, overrides in zip(xs, patches):
            base: dict = dict(
                width_px=PATCH_W, height_px=PATCH_H,
                sf_cycles_per_px=_SF, phase_cycles=0.0,
                contrast=1.0, waveform=_WAVEFORM, mask=_MASK,
            )
            base.update(overrides)
            # One row varies the placement's rotation_deg rather than a params field,
            # so it is split back out before the params object is built.
            rotation_deg = base.pop("rotation_deg", 0.0)
            h = conn.stimuli.grating.create_grating(
                position_px=Vec2(x, 0), rotation_deg=rotation_deg, params=GratingParams(**base),
            )
            assert h > 0
            handles.append(h)

        stage.step(row_name)

        for h in handles:
            conn.stimuli.delete(h)

    # Assertions via fresh single-grating queries.
    h_sf = conn.stimuli.grating.create_grating(
        position_px=Vec2(0, 0),
        params=GratingParams(width_px=PATCH_W, height_px=PATCH_H, sf_cycles_per_px=0.05),
    )
    info = conn.stimuli.query(h_sf)
    assert isinstance(info.params, GratingParams)
    assert info.params.sf_cycles_per_px == pytest.approx(0.05, rel=1e-3)
    stage.step("single patch, 0.05 cycles/px", hold=0.5)
    conn.stimuli.delete(h_sf)

    h_wf = conn.stimuli.grating.create_grating(
        position_px=Vec2(0, 0),
        params=GratingParams(width_px=PATCH_W, height_px=PATCH_H, waveform=GratingTexture.SQR),
    )
    info = conn.stimuli.query(h_wf)
    assert isinstance(info.params, GratingParams)
    assert info.params.waveform == GratingTexture.SQR
    stage.step("single patch, square-wave stripes", hold=0.5)
    conn.stimuli.delete(h_wf)

    # Drift animation.
    drift_handle = conn.stimuli.grating.create_grating(
        position_px=Vec2(0, 0),
        params=GratingParams(width_px=300, height_px=300, sf_cycles_per_px=0.05, contrast=1.0),
    )
    assert drift_handle > 0

    conn.stimuli.grating.set_drift_speed(drift_handle, 1.0)
    info = conn.stimuli.query(drift_handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.drift_speed_hz == pytest.approx(1.0, abs=0.01)
    assert info.params.drift_coupled is True
    stage.step("300×300 patch drifting at 1 Hz, across the stripes", hold=3.0)

    conn.stimuli.grating.set_drift_speed(drift_handle, -1.0)
    stage.step("same patch at −1 Hz — drifting back the other way", hold=3.0)

    conn.stimuli.grating.set_drift_decoupled(drift_handle, True)
    conn.stimuli.grating.set_drift_angle(drift_handle, 90.0)
    info = conn.stimuli.query(drift_handle)
    assert info.params.drift_coupled is False
    assert info.params.drift_angle_deg == pytest.approx(90.0, abs=0.1)
    stage.step("drift decoupled at 90° — motion now runs along the stripes", hold=3.0)

    conn.stimuli.grating.set_drift_speed(drift_handle, 0.0)
    conn.stimuli.grating.set_drift_decoupled(drift_handle, False)
    info = conn.stimuli.query(drift_handle)
    assert info.params.drift_speed_hz == pytest.approx(0.0, abs=0.01)
    assert info.params.drift_coupled is True
    stage.step("stopped — a static patch again", hold=0.5)

    conn.stimuli.delete(drift_handle)
    conn.system.set_background(r=0.0, g=0.0, b=0.0)


@pytest.mark.onscreen(
    "GRAT-10",
    "a 200×200 px grating in the centre made of two colours: red bars "
    "alternating with blue bars, no grey anywhere",
)
def test_grating_two_color_create(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.grating.create_grating(
        position_px=Vec2(0, 0),
        params=GratingParams(
            width_px=200,
            height_px=200,
            fore_color=Color(1.0, 0.0, 0.0),
            back_color=Color(0.0, 0.0, 1.0),
        ),
    )
    assert handle > 0

    info = conn.stimuli.query(handle)
    assert info.stimulus_type == StimulusType.GRATING
    assert isinstance(info.params, GratingParams)
    assert info.params.fore_color.r == pytest.approx(1.0, abs=0.01)
    assert info.params.fore_color.g == pytest.approx(0.0, abs=0.01)
    assert info.params.fore_color.b == pytest.approx(0.0, abs=0.01)
    assert info.params.fore_color.a == pytest.approx(1.0, abs=0.01)
    assert info.params.back_color.r == pytest.approx(0.0, abs=0.01)
    assert info.params.back_color.g == pytest.approx(0.0, abs=0.01)
    assert info.params.back_color.b == pytest.approx(1.0, abs=0.01)
    assert info.params.back_color.a == pytest.approx(1.0, abs=0.01)

    stage.hold()
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "GRAT-11",
    "a default grating whose foreground bars turn half-transparent brown "
    "(0.5, 0.25, 0) while the background bars stay as they were",
)
def test_grating_mutate_fore_color(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.grating.create_grating()
    stage.step("default grating, before the foreground colour changes", hold=0.5)

    conn.stimuli.grating.set_fore_color(handle, Color(0.5, 0.25, 0.0, 0.7))
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.fore_color.r == pytest.approx(0.5, abs=0.01)
    assert info.params.fore_color.g == pytest.approx(0.25, abs=0.01)
    assert info.params.fore_color.b == pytest.approx(0.0, abs=0.01)
    assert info.params.fore_color.a == pytest.approx(0.7, abs=0.01)

    stage.step("foreground bars now brown and part-transparent")
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "GRAT-12",
    "a default grating whose background bars turn a dark, mostly transparent "
    "blue-grey while the foreground bars are untouched",
)
def test_grating_mutate_back_color(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.grating.create_grating()
    stage.step("default grating, before the background colour changes", hold=0.5)

    conn.stimuli.grating.set_back_color(handle, Color(0.1, 0.2, 0.3, 0.4))
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.back_color.r == pytest.approx(0.1, abs=0.01)
    assert info.params.back_color.g == pytest.approx(0.2, abs=0.01)
    assert info.params.back_color.b == pytest.approx(0.3, abs=0.01)
    assert info.params.back_color.a == pytest.approx(0.4, abs=0.01)

    stage.step("background bars now dark blue-grey and mostly transparent")
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "GRAT-13",
    "a default grating that fades to 40 % opacity as a whole — the whole patch "
    "dims together, bars keeping their relative contrast",
)
def test_grating_mutate_opacity(conn: Connection, stage: Stage) -> None:
    """Opacity is the shared property, set with the shared command."""
    handle = conn.stimuli.grating.create_grating()
    stage.step("fully opaque grating", hold=0.5)

    conn.stimuli.set_alpha(handle, 0.4)
    assert conn.stimuli.query(handle).opacity == pytest.approx(0.4, abs=0.01)

    stage.step("the same grating at opacity 0.4 — faded into the background")
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "GRAT-14",
    "a red/green grating that becomes blue/green, then blue/yellow: setting "
    "one bar colour never disturbs the other",
)
def test_grating_fore_back_color_independent(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.grating.create_grating(
        params=GratingParams(fore_color=Color(1.0, 0.0, 0.0), back_color=Color(0.0, 1.0, 0.0)),
    )
    stage.step("red bars on green bars", hold=0.5)

    conn.stimuli.grating.set_fore_color(handle, Color(0.0, 0.0, 1.0))
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.fore_color.b == pytest.approx(1.0, abs=0.01)
    assert info.params.back_color.g == pytest.approx(1.0, abs=0.01)
    stage.step("foreground now blue, background still green", hold=0.5)

    conn.stimuli.grating.set_back_color(handle, Color(1.0, 1.0, 0.0))
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.fore_color.b == pytest.approx(1.0, abs=0.01)
    assert info.params.back_color.r == pytest.approx(1.0, abs=0.01)
    assert info.params.back_color.g == pytest.approx(1.0, abs=0.01)

    stage.step("background now yellow, foreground still blue")
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "GRAT-15",
    "a grating with half-transparent red bars over fully transparent blue "
    "ones, dimmed further to 80 % overall — the background shows through the "
    "gaps between the red bars",
)
def test_grating_per_color_alpha(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.grating.create_grating(
        params=GratingParams(
            fore_color=Color(1.0, 0.0, 0.0, 0.5),
            back_color=Color(0.0, 0.0, 1.0, 0.0),
        ),
    )
    assert handle > 0
    conn.stimuli.set_alpha(handle, 0.8)
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, GratingParams)
    assert info.params.fore_color.r == pytest.approx(1.0, abs=0.01)
    assert info.params.fore_color.a == pytest.approx(0.5, abs=0.01)
    assert info.params.back_color.b == pytest.approx(1.0, abs=0.01)
    assert info.params.back_color.a == pytest.approx(0.0, abs=0.01)
    assert info.opacity == pytest.approx(0.8, abs=0.01)

    stage.hold()
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "GRAT-16",
    "a 200×200 px red grating that drops to 50 % opacity — the whole patch, "
    "bars and gaps alike, is half faded into the background",
)
def test_grating_opacity(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.grating.create_grating(
        position_px=Vec2(0, 0),
        params=GratingParams(width_px=200, height_px=200, fore_color=Color(1.0, 0.0, 0.0)),
    )
    assert handle > 0
    stage.step("red grating at full opacity", hold=0.5)

    conn.stimuli.set_alpha(handle, 0.5)
    info = conn.stimuli.query(handle)
    assert info.stimulus_type == StimulusType.GRATING
    assert isinstance(info.params, GratingParams)
    assert info.opacity == pytest.approx(0.5, abs=0.01)

    stage.step("the same grating at opacity 0.5")
    conn.stimuli.delete(handle)
