// Named scene-config persistence. Mirrors vstimd.scene_config (Python).
//
// A scene-config is one experiment: stimuli, animations, background, photodiode
// and the named VTL trigger lines. The server stores each one in a *project* —
// a directory holding everything a study needs — under its --storage-dir, at
// `<storage-dir>/projects/<project>/scene-configs/<name>.config.json`.
//
// Names are `[<project>/]<name>`; an unqualified name means the `default`
// project, so the everyday case stays one word. The shipped demos live in
// `demos`. `retrieve` returns the current scene + I/O config as a JSON string
// (the same format as the on-disk files), which `upload` accepts back.

import { create, type MessageInitShape } from "@bufbuild/protobuf";
import { RequestSchema } from "./_proto/vstimd/v1/service_pb.js";
import type { Send } from "./transport.js";

/** Options for {@link SceneConfigClient.upload}. */
export interface UploadOpts {
  /** Replace an existing scene-config with the same name (default: error if it exists). */
  overwrite?: boolean;
  /** Apply the scene-config immediately after saving. */
  applyNow?: boolean;
  /** Only when `applyNow`: merge into the scene instead of replacing it. */
  additive?: boolean;
}

export class SceneConfigClient {
  constructor(private readonly send: Send) {}

  /**
   * Scene-config names on the server. Without `project`, every project is
   * listed and names outside `default` come back as `<project>/<name>`;
   * naming a project scopes the listing to it and returns bare names.
   */
  async list(opts: { project?: string } = {}): Promise<string[]> {
    const resp = await this.system({
      case: "listSceneConfigs",
      value: { project: opts.project ?? "" },
    });
    return resp.body.case === "sceneConfigList" ? resp.body.value.names : [];
  }

  /**
   * Load a named scene-config. With `additive`, merge stimuli/animations into
   * the current scene (handles remapped); otherwise the scene is cleared first.
   * The I/O config (VTL names) is always fully replaced.
   */
  async load(name: string, opts: { additive?: boolean } = {}): Promise<void> {
    await this.system({
      case: "loadSceneConfig",
      value: { name, additive: opts.additive ?? false },
    });
  }

  /** Return the current scene + I/O config as a JSON string. */
  async retrieve(): Promise<string> {
    const resp = await this.system({ case: "retrieveSceneConfig", value: {} });
    return resp.body.case === "retrievedSceneConfig" ? resp.body.value.json : "";
  }

  /** Upload a scene-config JSON string (as produced by {@link retrieve}) under `name`. */
  async upload(name: string, json: string, opts: UploadOpts = {}): Promise<void> {
    await this.system({
      case: "uploadSceneConfig",
      value: {
        name,
        json,
        overwrite: opts.overwrite ?? false,
        applyNow: opts.applyNow ?? false,
        additive: opts.additive ?? false,
      },
    });
  }

  /** Retrieve the current scene and save it under `name` in one call. */
  async save(name: string, opts: { overwrite?: boolean } = {}): Promise<void> {
    const json = await this.retrieve();
    await this.upload(name, json, { overwrite: opts.overwrite });
  }

  private system(body: MessageInitShape<typeof RequestSchema>["body"]) {
    return this.send(create(RequestSchema, { target: { case: "system", value: {} }, body }));
  }
}
