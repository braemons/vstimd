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

_PYTHON_CLIENT = pathlib.Path(__file__).parents[3]
_REPO_ROOT = _PYTHON_CLIENT.parents[1]
_EXAMPLES = _PYTHON_CLIENT / "examples" / "demos"
_SHIPPED = _REPO_ROOT / "server" / "config" / "demos"

#: (script stem, shipped demo name). One entry per tutorial page.
DEMO_SCRIPTS = [
    ("first_light",        "first_light"),
    ("drifting_grating",   "drifting_grating"),
    ("gratings_triggered", "gratings_triggered"),
    ("moving_target",      "moving_target"),
    ("photodiode_flicker", "photodiode_flicker"),
    ("trigger_gate",       "trigger_gate"),
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
    driven = {str(h) for a in animations.values() for h in a["stimuli"]}

    stimuli = []
    for handle in by_handle:
        stim = copy.deepcopy(entries[handle]["stimulus"])
        if handle in driven:
            # An animation owns this stimulus's visibility and may have it in
            # either state at the moment we look.
            stim["flags"].pop("enabled", None)
        if stim["type"] == "Grating" and stim["params"]["drift_speed"] != 0.0:
            stim["params"].pop("phase", None)   # advances every frame
        stimuli.append([entries[handle]["name"], _round(stim)])

    anims = []
    for handle in sorted(animations, key=int):
        anim = copy.deepcopy(animations[handle])
        anim.pop("state", None)                 # Armed vs Running is runtime
        anim["stimuli"] = [name_of[str(h)] for h in anim["stimuli"]]
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
    scene = json.loads(conn.config.retrieve())
    scene["scene"]["photodiode"]["enabled"] = False
    scene["scene"]["photodiode"]["flicker"] = False
    conn.config.upload("e2e_demo_scratch", json.dumps(scene),
                       overwrite=True, apply_now=True)


@pytest.fixture
def scene_cleanup(conn: Connection):
    """Leave the server as we found it — these scripts build a whole scene."""
    _reset_photodiode(conn)
    yield
    conn.system.delete_all()
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

@pytest.mark.parametrize("script_stem,demo_name", DEMO_SCRIPTS)
def test_demo_script_rebuilds_shipped_demo(
    conn: Connection,
    server_address: str,
    scene_cleanup: None,
    script_stem: str,
    demo_name: str,
) -> None:
    """Running the tutorial script reproduces the demo it documents."""
    shipped = _SHIPPED / f"vstimd_demo_{demo_name}.config.json"
    assert shipped.exists(), f"missing shipped demo: {shipped}"

    _run_demo_script(script_stem, demo_name, server_address)

    built = _canonical(json.loads(conn.config.retrieve()))
    expected = _canonical(json.loads(shipped.read_text(encoding="utf-8")))
    assert built == expected


def test_demo_script_saves_a_loadable_config(
    conn: Connection, server_address: str, scene_cleanup: None
) -> None:
    """The config a script saves is listed, and loads back into a cleared scene.

    This is the round trip the gratings/triggers/config tutorial ends on.
    """
    name = _run_demo_script("gratings_triggered", "gratings_triggered", server_address)
    assert name in conn.config.list_configs()

    conn.system.delete_all()
    for anim in conn.animations.list_animations():
        conn.animations.delete(anim.handle)
    assert conn.system.list_stimuli() == []

    conn.config.load(name)
    names = {entry.name for entry in conn.system.list_stimuli()}
    assert {"grating_45deg", "grating_135deg", "fixation_dot", "explanation"} <= names
    assert len(conn.animations.list_animations()) == 2
    assert {line.name for line in conn.vtl.list_lines()} >= {"in_pin11", "out_pin35"}


@pytest.mark.parametrize("script_stem,demo_name,flag", [
    ("gratings_triggered", "gratings_triggered", "--fire"),
    ("trigger_gate",       "trigger_gate",       "--toggle"),
])
def test_demo_script_software_trigger_flag(
    server_address: str, scene_cleanup: None, script_stem: str, demo_name: str, flag: str
) -> None:
    """The `--fire` / `--toggle` paths run: a software edge stands in for a DAQ.

    These flags are what make the trigger tutorials runnable with no wiring, so
    they need to keep working even though the scene they leave behind is the
    same one the test above already checks.
    """
    _run_demo_script(script_stem, demo_name, server_address, flag)
