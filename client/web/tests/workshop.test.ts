// Public web API, real isolated server, and an actual editor-exported scene.
import { spawn, type ChildProcess } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterAll, beforeAll, expect, it } from "vitest";
import { Connection, ConfigAlreadyExistsError } from "../src/index.js";
import { WorkshopSession, checkWorkshopName } from "../src/workshopApi.js";
const port = 8167;
const origin = `http://127.0.0.1:${port}`;
let server: ChildProcess;
let storage: string;
let conn: Connection;
let session: WorkshopSession;
const exported = JSON.parse(readFileSync(new URL("../../../daemon/tests/configs/editor_collision_v6.config.json", import.meta.url), "utf8"));
const staticScene = structuredClone(exported);
staticScene.scene.animations = {};
const json = JSON.stringify(staticScene);
beforeAll(async () => {
    if (!process.env.VSTIMD_BIN)
        throw new Error("Set VSTIMD_BIN to a vstimd binary built with embed-ui");
    storage = mkdtempSync(join(tmpdir(), "vstimd-workshop-test-"));
    const rig = join(storage, "rig.toml");
    writeFileSync(rig, `[vtl]\nshm_name="/workshop_test_${process.pid}"\n[[input.device]]\nname="wheel"\nshm="/missing_workshop_${process.pid}"\n[[input.device.axis]]\nname="wheel"\nsemantic="cumulative"\nscale=1.0\n`);
    server = spawn(process.env.VSTIMD_BIN, ["--null", "--no-mdns", "--no-events", "--web-port", String(port), "--zmq-port", "5767", "--rig-config", rig, "--storage-dir", storage], { stdio: "ignore" });
    for (let i = 0; i < 60; i++) {
        try {
            if ((await fetch(origin)).ok)
                break;
        }
        catch { /* starting */ }
        await new Promise(resolve => setTimeout(resolve, 100));
    }
    conn = await Connection.connect(`ws://127.0.0.1:${port}`);
    session = new WorkshopSession(`ws://127.0.0.1:${port}`);
}, 20000);
afterAll(() => { session?.close(); conn?.close(); server?.kill("SIGTERM"); if (storage)
    rmSync(storage, { recursive: true, force: true }); });
it("serves the editor at its own route with all referenced local assets", async () => {
    for (const route of ["/maze-workshop", "/maze-workshop/", "/maze-workshop/index.html"]) {
        const response = await fetch(origin + route);
        const html = await response.text();
        expect(response.status).toBe(200);
        expect(html).toContain("MAZE WORKSHOP");
        expect(html).toContain('id="rig-save"');
        expect(response.headers.get("cache-control")).toContain("no-cache");
        for (const match of html.matchAll(/(?:src|href)="(\/assets\/[^\"]+)"/g))
            expect((await fetch(origin + match[1])).status).toBe(200);
    }
});
it("connects, saves without changing the visible scene, and loads by replacement", async () => {
    expect(await session.connect()).toContain("Connected to vstimd");
    const before = JSON.parse(await conn.sceneConfig.retrieve());
    await session.save("workshop/integration", json, false);
    expect(await conn.sceneConfig.list({ project: "workshop" })).toContain("integration");
    expect(JSON.parse(await conn.sceneConfig.retrieve()).scene.stimuli).toEqual(before.scene.stimuli);
    await session.load("workshop/integration", json);
    const actual = JSON.parse(await conn.sceneConfig.retrieve());
    expect(Object.keys(actual.scene.stimuli)).toHaveLength(Object.keys(staticScene.scene.stimuli).length);
    expect(actual.scene.camera_movement.end_blackout_ms).toBe(2000);
});
it("refuses duplicate save until explicit overwrite, and reconnects after the refusal", async () => {
    await expect(session.save("workshop/integration", json, false)).rejects.toBeInstanceOf(ConfigAlreadyExistsError);
    await expect(session.save("workshop/integration", json, true)).resolves.toBeUndefined();
});
it("refuses unsafe names and refuses wheel loading without a live input", async () => {
    for (const name of ["../x", "x/../../y", "_private/scene", "project/name/extra", "project/"])
        expect(() => checkWorkshopName(name)).toThrow();
    const devices = await conn.system.listInputDevices();
    expect(devices[0].axes[0]).toMatchObject({ name: "wheel", semantic: "cumulative", scale: 1 });
    await session.save("workshop/wheel", JSON.stringify(exported), false);
    await expect(session.load("workshop/wheel", JSON.stringify(exported))).rejects.toThrow("No recent wheel samples");
    expect(Object.keys(JSON.parse(await conn.sceneConfig.retrieve()).scene.animations)).toHaveLength(0);
});
it("detects a closed connection and permits reconnecting explicitly", async () => {
    conn.close();
    await expect(conn.system.queryServerInfo()).rejects.toThrow(/closed/i);
    conn = await Connection.connect(`ws://127.0.0.1:${port}`);
    expect((await conn.system.queryServerInfo()).frameRateHz).toBeGreaterThan(0);
});
