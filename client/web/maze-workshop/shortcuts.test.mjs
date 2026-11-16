import test from 'node:test';
import assert from 'node:assert/strict';
import { ModifierHelp } from './shortcuts.mjs';
function fixture() {
    let now = 0, id = 0, visible = false, shows = 0;
    const timers = new Map();
    const help = new ModifierHelp(() => { visible = true; shows++; }, () => { visible = false; }, { setTimer: (fn, delay) => { const key = ++id; timers.set(key, { fn, at: now + delay }); return key; }, clearTimer: key => timers.delete(key) });
    const advance = ms => { now += ms; for (const [key, t] of [...timers])
        if (t.at <= now) {
            timers.delete(key);
            t.fn();
        } };
    return { help, advance, get visible() { return visible; }, get shows() { return shows; } };
}
const down = (key, code = key, repeat = false) => ({ key, code, repeat });
test('Command or Option held for 2 seconds shows help once; release closes it', () => {
    for (const [key, code] of [['Meta', 'MetaLeft'], ['Alt', 'AltRight']]) {
        const f = fixture();
        f.help.keydown(down(key, code));
        f.advance(1999);
        assert.equal(f.visible, false);
        f.help.keydown(down(key, code, true));
        f.advance(1);
        assert.equal(f.visible, true);
        assert.equal(f.shows, 1);
        f.advance(3000);
        assert.equal(f.shows, 1);
        f.help.keyup({ key, code });
        assert.equal(f.visible, false);
    }
});
test('short press and ordinary modifier shortcuts never open delayed help', () => {
    const f = fixture();
    f.help.keydown(down('Meta'));
    f.advance(1000);
    f.help.keyup({ key: 'Meta' });
    f.advance(3000);
    assert.equal(f.shows, 0);
    f.help.keydown(down('Meta'));
    f.advance(500);
    f.help.keydown(down('c'));
    f.advance(3000);
    assert.equal(f.shows, 0);
    f.help.keyup({ key: 'c' });
    f.help.keyup({ key: 'Meta' });
    f.help.keydown(down('Alt'));
    f.advance(2000);
    assert.equal(f.visible, true);
    f.help.keydown(down('v'));
    assert.equal(f.visible, false);
    f.advance(3000);
    assert.equal(f.shows, 1);
});
test('blur/reset clears pending timers, and releasing one of two held modifiers keeps help available', () => {
    const f = fixture();
    f.help.keydown(down('Meta'));
    f.advance(1000);
    f.help.reset();
    f.advance(3000);
    assert.equal(f.shows, 0);
    f.help.keydown(down('Meta'));
    f.help.keydown(down('Alt'));
    f.advance(2000);
    assert.equal(f.visible, true);
    f.help.keyup({ key: 'Meta' });
    assert.equal(f.visible, true);
    f.help.reset();
    assert.equal(f.visible, false);
    f.help.keydown(down('Alt'));
    f.advance(2000);
    assert.equal(f.visible, true);
    f.help.keyup({ key: 'Alt' });
    assert.equal(f.visible, false);
});
