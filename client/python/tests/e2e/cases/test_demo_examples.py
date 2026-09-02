"""E2E tests that actually run the demo tutorial scripts.

Each ``examples/demos/*.py`` script claims to rebuild one of the shipped demo
configs in ``server/config/demos/``, and the matching tutorial page in
``docs/tutorials/`` walks through that script line by line. These tests run
the scripts for real against the server and compare what lands in the scene with
the shipped config, so a tutorial cannot quietly drift away from the demo it
says it builds — or from an API that has moved on.

The comparison ignores what is genuinely runtime state rather than scene
content: stimulus ids, the numeric value of handles, whether an animation is currently
`Armed` or `Running`, the visibility of a stimulus an animation is driving, the
phase of a drifting grating, and the live photodiode level.
"""

from __future__ import annotations

import copy
import json
import pathlib
import subprocess
import sys

import pytest

from vstimd import Connection

from ._helpers import Stage

_PYTHON_CLIENT = pathlib.Path(__file__).parents[3]
_REPO_ROOT = _PYTHON_CLIENT.parents[1]
_EXAMPLES = _PYTHON_CLIENT / "examples" / "demos"
_SHIPPED = _REPO_ROOT / "server" / "config" / "demos"

#: (test id, script stem, shipped demo name, what the demo puts on screen).
#: One entry per tutorial page. The last field is the caption an operator sees
#: while the built scene is held up for inspection.
DEMO_SCRIPTS = [
    ("DEMO-01", "first_light", "first_light",
     "a white fixation dot in the centre and a line of explanatory text"),
    ("DEMO-02", "drifting_grating", "drifting_grating",
     "a masked grating patch drifting steadily, with a fixation dot on top"),
    ("DEMO-03", "gratings_triggered", "gratings_triggered",
     "two gratings, 45° and 135°, each waiting on its own trigger line, "
     "plus a fixation dot and a caption of their own"),
    ("DEMO-04", "moving_target", "moving_target",
     "a target sweeping across the screen along its path"),
    ("DEMO-05", "photodiode_flicker", "photodiode_flicker",
     "a photodiode patch flickering in a screen corner"),
    ("DEMO-06", "trigger_gate", "trigger_gate",
     "a stimulus whose visibility is gated by the level of a trigger line"),
]


# ── Normalisation ─────────────────────────────────────────────────────────────

def _round(value):
    """Round every float in a nested structure, so 0.1 + 0.2 never fails us."""
    if isinstance(value, float):
        return round(value, 4)
    if isinstance(value, dict):
        return {k: _round(v) for k, v in value.items()}
    if isinstance(value, list):
        return [_round(v) for v in value]
    return value


def _driven_handles(anim: dict) -> list[int]:
    """Stimulus handles an animation drives, out of its target.

    The target is a tagged union in the config (`{"kind": "Stimuli", ...}`) so
    that a 3-D camera can become one later; anything that is not stimuli drives
    no stimulus handles.
    """
    target = anim.get("target", {})
    return target.get("handles", []) if target.get("kind") == "Stimuli" else []


# Animations that move their target every frame, so its position is a runtime
# value rather than something the tutorial script sets.
_MOVING_ANIMATIONS = frozenset(
    {"MoveAlongPath2D", "MoveAlongSegments2D", "ExternalPosition2D"}
)


def _moves_its_target(anim: dict) -> bool:
    """True when the animation writes its target's position on every frame."""
    body = anim.get("animation")
    if isinstance(body, dict):
        return bool(_MOVING_ANIMATIONS & body.keys())
    return body in _MOVING_ANIMATIONS


def _canonical(cfg: dict) -> dict:
    """Reduce a config to the parts a tutorial is responsible for reproducing."""
    scene = cfg["scene"]
    entries = scene["stimuli"]
    animations = scene["animations"]

    # Handle *values* are incidental, but their order is not: the scene stores
    # stimuli in an IndexMap, so handle order is draw order — part of what a
    # tutorial has to reproduce. Keep the sequence, drop the numbers.
    by_handle = sorted(entries, key=int)
    name_of = {h: entries[h]["name"] for h in by_handle}
    driven = {str(h) for a in animations.values() for h in _driven_handles(a)}
    moved = {
        str(h)
        for a in animations.values()
        if _moves_its_target(a)
        for h in _driven_handles(a)
    }

    stimuli = []
    for handle in by_handle:
        stim = copy.deepcopy(entries[handle]["stimulus"])
        if handle in driven:
            # An animation owns this stimulus's visibility and may have it in
            # either state at the moment we look.
            stim["common"]["flags"].pop("enabled", None)
        body = stim["body"]
        if handle in moved:
            # A sweep owns this stimulus's position: by the time the config is
            # retrieved the animation has already moved it some way along its
            # path, and where exactly depends on how many frames have passed.
            body.get("transform", {}).pop("pos_px", None)
        if body["type"] == "Grating" and body["params"]["drift_speed_hz"] != 0.0:
            body["params"].pop("phase_cycles", None)   # advances every frame
        stimuli.append([entries[handle]["name"], _round(stim)])

    anims = []
    for handle in sorted(animations, key=int):
        anim = copy.deepcopy(animations[handle])
        anim.pop("state", None)                 # Armed vs Running is runtime
        # Compare by stimulus name: handle values are incidental, the wiring is not.
        anim["target"] = {
            **anim["target"],
            "handles": [name_of[str(h)] for h in _driven_handles(anim)],
        }
        anims.append(_round(anim))
    anims.sort(key=lambda a: a["name"])

    photodiode = {k: v for k, v in scene["photodiode"].items() if k != "lit"}
    lines = cfg["io"]["vtl"]["names"]

    return {
        "background": _round(scene["background"]),
        "photodiode": photodiode,
        "stimuli": stimuli,
        "animations": anims,
        "vtl_names": sorted(
            (line["name"], line["bank"], line["bit"], line["kind"]) for line in lines
        ),
    }


# ── Fixtures ──────────────────────────────────────────────────────────────────

def _reset_photodiode(conn: Connection) -> None:
    """Turn the photodiode patch off again.

    It is a scene setting with no command of its own, so the only way to clear
    it between runs is the same retrieve/patch/upload path the scripts use to
    switch it on. Without this, a demo that turns it on leaks into the next
    script's scene — the scripts themselves never turn it off.
    """
    scene = json.loads(conn.scene_config.retrieve())
    scene["scene"]["photodiode"]["enabled"] = False
    scene["scene"]["photodiode"]["flicker"] = False
    conn.scene_config.upload("e2e_demo_scratch", json.dumps(scene),
                       overwrite=True, apply_now=True)


@pytest.fixture
def scene_cleanup(conn: Connection):
    """Leave the server as we found it — these scripts build a whole scene."""
    _reset_photodiode(conn)
    yield
    conn.system.clear_all()
    for anim in conn.animations.list_animations():
        conn.animations.delete(anim.handle)
    for line in conn.vtl.list_lines():
        conn.vtl.set_line_name(line.bank, line.bit, line.kind, name="")
    conn.system.set_background(0.0, 0.0, 0.0)
    _reset_photodiode(conn)


# ── Running a script ──────────────────────────────────────────────────────────

def _run_demo_script(
    script_stem: str, demo_name: str, server_address: str, *extra_args: str
) -> str:
    """Run one example script the way its Usage block says to, and return the
    config name it saved under."""
    script = _EXAMPLES / f"{script_stem}.py"
    assert script.exists(), f"missing example script: {script}"
    save_as = f"e2e_demo_{demo_name}"

    result = subprocess.run(
        [sys.executable, str(script), server_address,
         "--save-as", save_as, "--overwrite", *extra_args],
        cwd=_PYTHON_CLIENT,
        capture_output=True,
        text=True,
        # The slowest script (--toggle) sleeps its way through a handful of
        # software edges in a few seconds. Anything near this bound means the
        # script is stuck on the server, and we would rather fail than hang the
        # whole e2e run.
        timeout=120,
    )
    assert result.returncode == 0, (
        f"{script.name} failed (exit {result.returncode})\n"
        f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
    )
    return save_as


# ── Tests ─────────────────────────────────────────────────────────────────────

@pytest.mark.onscreen(
    "DEMO-01…06",
    "each tutorial demo scene, built by its own script and then held on screen "
    "— the per-demo id and caption are set once the scene is built",
    deferred=True,
)
@pytest.mark.parametrize(
    "test_id,script_stem,demo_name,visible",
    DEMO_SCRIPTS,
    ids=[test_id for test_id, *_ in DEMO_SCRIPTS],
)
def test_demo_script_rebuilds_shipped_demo(
    conn: Connection,
    server_address: str,
    scene_cleanup: None,
    stage: Stage,
    test_id: str,
    script_stem: str,
    demo_name: str,
    visible: str,
) -> None:
    """Running the tutorial script reproduces the demo it documents.

    The scene the script builds is compared against the shipped config *and*
    then held on screen, so this is also where an operator gets to look at each
    demo. The comparison is over the whole scene, so the caption cannot be up
    while it runs — hence the deferred stage, put up only afterwards.
    """
    shipped = _SHIPPED / f"{demo_name}.config.json"
    assert shipped.exists(), f"missing shipped demo: {shipped}"

    _run_demo_script(script_stem, demo_name, server_address)

    built = _canonical(json.loads(conn.scene_config.retrieve()))
    expected = _canonical(json.loads(shipped.read_text(encoding="utf-8")))
    assert built == expected

    # The demo scene is live on the server right now; the fixture tears it down
    # the moment this test returns, so this is the only chance to see it.
    stage.test_id = test_id
    stage.step(f"demo '{demo_name}' — {visible}", hold=3.0)


@pytest.mark.onscreen(
    "DEMO-07",
    "the gratings_triggered scene is built by its script, cleared away to a "
    "blank screen, and then rebuilt from the config file it saved",
    deferred=True,
)
def test_demo_script_saves_a_loadable_config(
    conn: Connection, server_address: str, scene_cleanup: None, stage: Stage
) -> None:
    """The config a script saves is listed, and loads back into a cleared scene.

    This is the round trip the gratings/triggers/config tutorial ends on.
    """
    name = _run_demo_script("gratings_triggered", "gratings_triggered", server_address)
    assert name in conn.scene_config.list_scene_configs()

    conn.system.clear_all()
    for anim in conn.animations.list_animations():
        conn.animations.delete(anim.handle)
    assert conn.system.list_stimuli() == []

    conn.scene_config.load(name)
    names = {entry.name for entry in conn.system.list_stimuli()}
    assert {"grating_45deg", "grating_135deg", "fixation_dot", "explanation"} <= names
    assert len(conn.animations.list_animations()) == 2
    assert {line.name for line in conn.vtl.list_lines()} >= {"in_pin11", "out_pin35"}

    stage.step("the demo scene, restored from the config the script saved", hold=2.0)


@pytest.mark.onscreen(
    "DEMO-08…09",
    "a demo script firing its own trigger edges in software — the per-demo id "
    "and caption go up once the script has finished",
    deferred=True,
)
@pytest.mark.parametrize("test_id,script_stem,demo_name,flag", [
    ("DEMO-08", "gratings_triggered", "gratings_triggered", "--fire"),
    ("DEMO-09", "trigger_gate",       "trigger_gate",       "--toggle"),
], ids=["DEMO-08", "DEMO-09"])
def test_demo_script_software_trigger_flag(
    server_address: str,
    scene_cleanup: None,
    stage: Stage,
    test_id: str,
    script_stem: str,
    demo_name: str,
    flag: str,
) -> None:
    """The `--fire` / `--toggle` paths run: a software edge stands in for a DAQ.

    These flags are what make the trigger tutorials runnable with no wiring, so
    they need to keep working even though the scene they leave behind is the
    same one the test above already checks.
    """
    _run_demo_script(script_stem, demo_name, server_address, flag)

    # The script clears the scene as it builds, so the caption only goes up once
    # it has finished — what is left on screen is the demo in its final state.
    stage.test_id = test_id
    stage.step(f"demo '{demo_name}' after its {flag} run: the script fired the "
               "trigger edges itself, with no DAQ wiring involved", hold=2.0)
