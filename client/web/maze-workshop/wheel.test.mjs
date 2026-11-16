import test from 'node:test';
import assert from 'node:assert/strict';
import { validateNavigation, wheelAnimation, suggestTrackLength } from './wheel.mjs';
import { template, object, exportScene, validate } from './model.mjs';
const settings = (patch = {}) => ({ mode: 'wheel', device: 'wheel', axis: 'wheel', loop: true, track_length_cm: 400, fade_frames: 15, ...patch });
test('existing projects remain static unless wheel navigation is configured', () => {
    const p = template();
    assert.deepEqual(exportScene(p).scene.animations, {});
    p.navigation = { mode: 'static' };
    assert.deepEqual(exportScene(p).scene.animations, {});
    assert.equal(wheelAnimation(undefined), null);
});
test('wheel export uses a native camera animation, zero autonomous speed and the named input', () => {
    const p = template();
    p.navigation = settings();
    const before = JSON.stringify(p), scene = exportScene(p).scene;
    const animation = scene.animations[1];
    assert.equal(animation.state, 'Armed');
    assert.deepEqual(animation.target, { kind: 'Camera' });
    assert.equal(animation.start_trigger, null);
    assert.deepEqual(animation.animation.LinearNav3D, { speed_cm_per_s: 0, wrap_period_cm: null, source: { device: 'wheel', axis: 'wheel' }, track: { length_cm: 400, fade_frames: 0 } });
    assert.equal(scene.next_anim_handle, 2);
    assert.equal(JSON.stringify(p), before);
});
test('navigation settings round trip, retain initial camera heading and support continuous movement', () => {
    const p = template();
    p.camera.yaw = 90;
    p.navigation = settings({ device: 'encoder', axis: 'distance', loop: false });
    const restored = validate(JSON.parse(JSON.stringify(p))), scene = exportScene(restored).scene;
    assert.equal(scene.camera.yaw_deg, -90);
    assert.equal(scene.animations[1].animation.LinearNav3D.track, null);
    assert.deepEqual(scene.animations[1].animation.LinearNav3D.source, { device: 'encoder', axis: 'distance' });
});
test('track length suggestion uses floor extent along the initial heading, ignoring distant cues', () => {
    const p = template();
    assert.equal(suggestTrackLength(p.objects, p.camera), 400);
    assert.equal(suggestTrackLength([object('floor', { x: 200, s: 0, w: 400, d: 40 })], { x: 0, s: 0, yaw: 90 }), 400);
    assert.equal(suggestTrackLength([], { x: 0, s: 0, yaw: 0 }), 400);
});
test('invalid wheel names, lengths and non-integer fade are rejected before export', () => {
    for (const patch of [{ device: '' }, { axis: 'bad/name' }, { loop: 'true' }, { track_length_cm: 0 }, { track_length_cm: NaN }, { fade_frames: 1.5 }, { fade_frames: -1 }, { mode: 'network' }]) {
        assert.throws(() => validateNavigation(settings(patch)));
        const p = template();
        p.navigation = settings(patch);
        assert.throws(() => exportScene(p));
    }
    assert.doesNotThrow(() => validateNavigation(settings({ fade_frames: 0 })));
});
