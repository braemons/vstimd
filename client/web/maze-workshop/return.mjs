// Monotonic timestamps are injected so the 2 s hold can be tested without sleeps.
export class ReturnHold {
    constructor() { this.deadline = null; }
    get active() { return this.deadline !== null; }
    begin(now) { if (!this.active)
        this.deadline = now + 2000; }
    finish(now) { if (!this.active || now < this.deadline)
        return false; this.deadline = null; return true; }
    cancel() { this.deadline = null; }
}
