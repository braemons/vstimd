// Text stimulus client.

import { create } from "@bufbuild/protobuf";
import { RequestSchema } from "./_proto/vstimd/v1/service_pb.js";
import type { Send } from "./transport.js";
import type { Color, StimulusHandle, Vec2 } from "./types.js";

export class TextClient {
  constructor(private readonly send: Send) {}

  async create(opts: {
    text: string;
    posPx?: Vec2;
    font?: string;
    letterHeightPx?: number;
    color?: Color;
    name?: string;
  }): Promise<StimulusHandle> {
    const { text, posPx = { x: 0, y: 0 }, font = "", letterHeightPx = 32, color, name = "" } = opts;
    const resp = await this.send(
      create(RequestSchema, {
        target: { case: "system", value: {} },
        body: {
          case: "createText",
          value: {
            identity: { name },
            placement: { posPx },
            params: { text, font, letterHeightPx, textColor: color },
          },
        },
      }),
    );
    return resp.handle;
  }

  async setText(handle: StimulusHandle, text: string): Promise<void> {
    await this.send(
      create(RequestSchema, {
        target: { case: "stimulus", value: handle },
        body: { case: "setText", value: { text } },
      }),
    );
  }

  async setColor(handle: StimulusHandle, color: Color): Promise<void> {
    await this.send(
      create(RequestSchema, {
        target: { case: "stimulus", value: handle },
        body: { case: "setTextColor", value: { color } },
      }),
    );
  }
}
