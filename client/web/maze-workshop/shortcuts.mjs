// A modifier held alone opens transient help; ordinary shortcuts keep working.
export class ModifierHelp {
    constructor(show, hide, { setTimer = setTimeout, clearTimer = clearTimeout } = {}) { this.show = show; this.hide = hide; this.setTimer = setTimer; this.clearTimer = clearTimer; this.held = new Set(); this.timer = null; this.blocked = false; }
    keydown(event) {
        if (!['Meta', 'Alt'].includes(event.key)) {
            if (this.held.size) {
                this.blocked = true;
                this.stop();
            }
            return;
        }
        const code = event.code || event.key;
        if (event.repeat || this.held.has(code))
            return;
        this.held.add(code);
        if (this.timer === null && !this.blocked)
            this.timer = this.setTimer(() => { this.timer = null; if (this.held.size && !this.blocked)
                this.show(); }, 2000);
    }
    keyup(event) { if (!['Meta', 'Alt'].includes(event.key))
        return; this.held.delete(event.code || event.key); if (!this.held.size)
        this.reset(); }
    stop() { if (this.timer !== null)
        this.clearTimer(this.timer); this.timer = null; this.hide(); }
    reset() { this.stop(); this.held.clear(); this.blocked = false; }
}
export function installShortcutHelp(panel) {
    const help = new ModifierHelp(() => { panel.hidden = false; }, () => { panel.hidden = true; });
    window.addEventListener('keydown', e => help.keydown(e));
    window.addEventListener('keyup', e => help.keyup(e));
    window.addEventListener('blur', () => help.reset());
    document.addEventListener('visibilitychange', () => { if (document.hidden)
        help.reset(); });
    return help;
}
