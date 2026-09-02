"""E2E tests for config persistence (ConfigClient).

Most of these save and reload the whole scene, so the on-screen caption is
declared ``deferred``: it would otherwise be captured into the saved config and
come back as a duplicate on load. Each test puts its caption up itself once it
is past the point where the scene has to be pristine.
"""
from __future__ import annotations

import json

import pytest

from vstimd import (
    SceneConfigAlreadyExistsError,
    SceneConfigFormatError,
    SceneConfigNotFoundError,
    Connection,
)
from vstimd.stimuli import RectParams

from ._helpers import Stage


@pytest.mark.onscreen(
    "CFG-01",
    "nothing on screen: the current scene is retrieved as JSON and checked for "
    "a version-5 envelope with scene and io sections",
)
def test_retrieve_returns_valid_json(conn: Connection, stage: Stage) -> None:
    """retrieve() returns a non-empty string that parses as JSON."""
    raw = conn.scene_config.retrieve()
    assert isinstance(raw, str) and len(raw) > 0
    data = json.loads(raw)
    assert data["version"] == 5
    assert "scene" in data
    assert "io" in data
    stage.hold(0.3)


@pytest.mark.onscreen(
    "CFG-02",
    "nothing on screen: the retrieved JSON carries the background, stimuli and "
    "animations a scene is made of",
)
def test_retrieve_scene_structure(conn: Connection, stage: Stage) -> None:
    """retrieve() JSON contains expected scene keys."""
    data = json.loads(conn.scene_config.retrieve())
    scene = data["scene"]
    assert "background" in scene
    assert "stimuli" in scene
    assert "animations" in scene
    stage.hold(0.3)


@pytest.mark.onscreen(
    "CFG-03",
    "nothing on screen: the scene is uploaded as 'e2e_test_list' and then "
    "found in the server's list of saved configs",
)
def test_upload_and_list(conn: Connection, stage: Stage) -> None:
    """Uploaded config appears in list_configs()."""
    raw = conn.scene_config.retrieve()
    conn.scene_config.upload("e2e_test_list", raw, overwrite=True)
    names = conn.scene_config.list_scene_configs()
    assert "e2e_test_list" in names
    stage.hold(0.3)


@pytest.mark.onscreen(
    "CFG-04",
    "nothing on screen: save() does retrieve() plus upload() in one call and "
    "leaves 'e2e_test_save' on the server",
)
def test_save_convenience(conn: Connection, stage: Stage) -> None:
    """save() is equivalent to retrieve() + upload()."""
    conn.scene_config.save("e2e_test_save", overwrite=True)
    assert "e2e_test_save" in conn.scene_config.list_scene_configs()
    stage.hold(0.3)


@pytest.mark.onscreen(
    "CFG-05",
    "a named 50×50 px rect appears, the scene is saved and then cleared "
    "(screen goes blank), and loading the config brings the rect back",
    deferred=True,
)
def test_upload_and_load_roundtrip(conn: Connection, stage: Stage) -> None:
    """A config saved via upload() is restored correctly via load()."""
    # Create a rect, save config, delete everything, load back.
    h = conn.stimuli.shapes.create_rect(
        name="cfg_roundtrip_rect",
        params=RectParams(width_px=50, height_px=50),
    )
    conn.scene_config.save("e2e_test_roundtrip", overwrite=True)
    conn.system.clear_all()

    stim_handles_before = {e.handle for e in conn.system.list_stimuli()}
    assert h not in stim_handles_before

    conn.scene_config.load("e2e_test_roundtrip")
    entries = conn.system.list_stimuli()
    names = {e.name for e in entries}
    assert "cfg_roundtrip_rect" in names

    stage.step("the 50×50 px rect is back, restored from the saved config")


@pytest.mark.onscreen(
    "CFG-06",
    "one rect on an otherwise empty screen becomes two identical rects: an "
    "additive load appends the saved copy instead of replacing the scene",
    deferred=True,
)
def test_load_additive(conn: Connection, stage: Stage) -> None:
    """load(additive=True) appends to the existing scene without clearing it."""
    conn.system.clear_all()
    h_existing = conn.stimuli.shapes.create_rect(name="existing_stim")

    conn.scene_config.save("e2e_test_additive", overwrite=True)

    # Load the saved config additively (it contains "existing_stim").
    conn.scene_config.load("e2e_test_additive", additive=True)

    names = {e.name for e in conn.system.list_stimuli()}
    # The original stimulus is still there AND the loaded one is added.
    assert "existing_stim" in names
    # Two entries named "existing_stim": the original and the loaded copy.
    count = sum(1 for e in conn.system.list_stimuli() if e.name == "existing_stim")
    assert count == 2

    stage.step("two copies of the same rect, exactly on top of each other")
    conn.system.clear_all()


@pytest.mark.onscreen(
    "CFG-07",
    "nothing on screen: uploading over an existing config name without "
    "overwrite=True is refused with SceneConfigAlreadyExistsError",
)
def test_upload_overwrite_false_raises(conn: Connection, stage: Stage) -> None:
    """Uploading a config that already exists without overwrite=True raises."""
    raw = conn.scene_config.retrieve()
    conn.scene_config.upload("e2e_test_no_overwrite", raw, overwrite=True)
    with pytest.raises(SceneConfigAlreadyExistsError):
        conn.scene_config.upload("e2e_test_no_overwrite", raw, overwrite=False)
    stage.hold(0.3)


@pytest.mark.onscreen(
    "CFG-08",
    "nothing on screen: loading a config name that was never saved is refused "
    "with SceneConfigNotFoundError",
)
def test_load_nonexistent_raises(conn: Connection, stage: Stage) -> None:
    """Loading a config that does not exist raises SceneConfigNotFoundError."""
    with pytest.raises(SceneConfigNotFoundError):
        conn.scene_config.load("this_name_does_not_exist_xyz123")
    stage.hold(0.3)


@pytest.mark.onscreen(
    "CFG-09",
    "nothing on screen: uploading a string that is not JSON is refused with "
    "SceneConfigFormatError, and the scene is left alone",
)
def test_upload_invalid_json_raises(conn: Connection, stage: Stage) -> None:
    """Uploading a malformed JSON string raises SceneConfigFormatError."""
    with pytest.raises(SceneConfigFormatError):
        conn.scene_config.upload("e2e_test_bad_json", "not valid json {{{", overwrite=True)
    stage.hold(0.3)


@pytest.mark.onscreen(
    "CFG-10",
    "a named rect appears, the screen is cleared, and uploading the saved "
    "config with apply_now=True puts the rect straight back without a load",
    deferred=True,
)
def test_upload_apply_now(conn: Connection, stage: Stage) -> None:
    """upload(apply_now=True) applies the config immediately."""
    conn.system.clear_all()
    h = conn.stimuli.shapes.create_rect(name="apply_now_rect")
    raw = conn.scene_config.retrieve()

    conn.system.clear_all()
    assert len(conn.system.list_stimuli()) == 0

    conn.scene_config.upload("e2e_test_apply_now", raw, overwrite=True, apply_now=True)
    names = {e.name for e in conn.system.list_stimuli()}
    assert "apply_now_rect" in names

    stage.step("the rect is on screen again, applied straight from the upload")
    conn.system.clear_all()
