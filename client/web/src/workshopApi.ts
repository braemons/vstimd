// Maze Workshop uses the public web client and the existing scene-config API.
import { Connection } from "./connection.js";
export interface WorkshopApi {
    connect(): Promise<string>;
    save(name: string, json: string, overwrite: boolean): Promise<void>;
    load(name: string, json: string): Promise<void>;
    close(): void;
}
export function checkWorkshopName(name: string): void {
    const parts = name.split("/");
    if (parts.length !== 2 || parts.some(p => !/^[A-Za-z0-9][A-Za-z0-9_.-]{0,63}$/.test(p))) {
        throw new Error("Use a project and scene name of 1–64 letters, numbers, dots, dashes or underscores; start with a letter or number.");
    }
}
export class WorkshopSession implements WorkshopApi {
    private pending: Promise<Connection> | null = null;
    constructor(private readonly baseUrl: string) { }
    private connection(): Promise<Connection> {
        this.pending ??= Connection.connect(this.baseUrl);
        return this.pending;
    }
    private async run<T>(action: (conn: Connection) => Promise<T>): Promise<T> {
        try {
            return await action(await this.connection());
        }
        catch (error) {
            this.close();
            throw error;
        }
    }
    connect(): Promise<string> {
        return this.run(async (conn) => `Connected to vstimd ${(await conn.system.queryServerInfo()).version}`);
    }
    async save(name: string, json: string, overwrite: boolean): Promise<void> {
        checkWorkshopName(name);
        await this.run(conn => conn.sceneConfig.upload(name, json, { overwrite, applyNow: false, additive: false }));
    }
    async load(name: string, json: string): Promise<void> {
        checkWorkshopName(name);
        const scene = JSON.parse(json) as {
            scene: {
                animations?: Record<string, {
                    animation?: {
                        LinearNav3D?: {
                            source?: {
                                device: string;
                                axis: string;
                            };
                        };
                    };
                }>;
            };
        };
        const sources = Object.values(scene.scene.animations ?? {}).flatMap(a => a.animation?.LinearNav3D?.source ? [a.animation.LinearNav3D.source] : []);
        await this.run(async (conn) => {
            if (sources.length) {
                const devices = await conn.system.listInputDevices();
                for (const source of sources) {
                    const device = devices.find(d => d.name === source.device);
                    const axis = device?.axes.find(a => a.name === source.axis);
                    if (!axis || axis.semantic !== "cumulative" || axis.scale !== 1) {
                        throw new Error(`Wheel input ${source.device}:${source.axis} must be configured as cumulative centimetres with scale 1 in vstimd.`);
                    }
                    if (!device?.connected || device.stale) {
                        throw new Error(`No recent wheel samples from ${source.device}. Check mousewheeld and the USB connection before loading.`);
                    }
                }
            }
            await conn.sceneConfig.load(name, { additive: false });
        });
    }
    close(): void {
        const pending = this.pending;
        this.pending = null;
        void pending?.then(conn => conn.close(), () => undefined);
    }
}
