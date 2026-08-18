// Grating stimulus client. Exposes friendly string unions for waveform/mask and
// maps them to the proto enums internally.

import { create, type MessageInitShape } from "@bufbuild/protobuf";
import { RequestSchema } from "./_proto/vstimd/v1/service_pb.js";
import { MaskType, WaveformType } from "./_proto/vstimd/v1/stimuli/grating_pb.js";
import type { Send } from "./transport.js";
import type { Color, StimulusHandle, Vec2 } from "./types.js";

export type Waveform = "sin" | "sqr" | "saw" | "tri";
export type GratingMask = "none" | "circle" | "gauss" | "hann" | "raisedCos";

const WAVEFORM: Record<Waveform, WaveformType> = {
  sin: WaveformType.SIN,
  sqr: WaveformType.SQR,
  saw: WaveformType.SAW,
  tri: WaveformType.TRI,
};
const MASK: Record<GratingMask, MaskType> = {
  none: MaskType.NONE,
  circle: MaskType.CIRCLE,
  gauss: MaskType.GAUSS,
  hann: MaskType.HANN,
  raisedCos: MaskType.RAISED_COS,
};

export class GratingClient {
  constructor(private readonly send: Send) {}

  async create(opts: {
    posPx?: Vec2;
    widthPx?: number;
    heightPx?: number;
    sfCyclesPerPx?: number;
    phaseCycles?: number;
    angle?: number;
    contrast?: number;
    foreColor?: Color;
    backColor?: Color;
    waveform?: Waveform;
    mask?: GratingMask;
    driftSpeedHz?: number;
    name?: string;
  } = {}): Promise<StimulusHandle> {
    const {
      posPx = { x: 0, y: 0 }, widthPx = 200, heightPx = 200, sfCyclesPerPx = 0.05, phaseCycles = 0,
      angle = 0, contrast = 1, foreColor, backColor,
      waveform = "sin", mask = "none", driftSpeedHz = 0, name = "",
    } = opts;
    const resp = await this.send(
      create(RequestSchema, {
        target: { case: "system", value: {} },
        body: {
          case: "createGrating",
          value: {
            identity: { name },
            // The rotation is the stripe rotationDeg, not the patch's.
            placement: { posPx, rotationDeg: angle },
            params: {
              widthPx, heightPx, sfCyclesPerPx, phaseCycles, contrast, foreColor, backColor,
              waveform: WAVEFORM[waveform], mask: MASK[mask], driftSpeedHz,
            },
          },
        },
      }),
    );
    return resp.handle;
  }

  setSf(h: StimulusHandle, sfCyclesPerPx: number) { return this.cmd(h, { case: "setGratingSf", value: { sfCyclesPerPx } }); }
  setContrast(h: StimulusHandle, contrast: number) { return this.cmd(h, { case: "setGratingContrast", value: { contrast } }); }
  setPhase(h: StimulusHandle, phaseCycles: number) { return this.cmd(h, { case: "setGratingPhase", value: { phaseCycles } }); }
  setDriftSpeed(h: StimulusHandle, speedHz: number) { return this.cmd(h, { case: "setGratingDriftSpeed", value: { speedHz } }); }
  // Opacity is shared: use `conn.stimuli.setAlpha(handle, opacity)`.
  setWaveform(h: StimulusHandle, w: Waveform) { return this.cmd(h, { case: "setGratingWaveform", value: { waveform: WAVEFORM[w] } }); }
  setMask(h: StimulusHandle, m: GratingMask) { return this.cmd(h, { case: "setGratingMask", value: { mask: MASK[m] } }); }
  setForeColor(h: StimulusHandle, foreColor: Color) { return this.cmd(h, { case: "setGratingForeColor", value: { foreColor } }); }
  setBackColor(h: StimulusHandle, backColor: Color) { return this.cmd(h, { case: "setGratingBackColor", value: { backColor } }); }

  private cmd(handle: StimulusHandle, body: MessageInitShape<typeof RequestSchema>["body"]): Promise<unknown> {
    return this.send(create(RequestSchema, { target: { case: "stimulus", value: handle }, body }));
  }
}
