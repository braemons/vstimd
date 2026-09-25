# Colour for mouse vision

**Status:** notes and a proposal. Nothing here is implemented. Written while
building splat corridors (`SPLAT_RECONSTRUCTION_PLAN.md`), because a
photorealistic scene raises the question that abstract stimuli let you dodge:
the colours are realistic *to a human*, and the animal is not one.

**The short version:** a mouse is a dichromat, so two primaries (UV and green)
can reproduce every colour it can tell apart — a *better* match than the three
primaries we give humans. What an ordinary screen cannot do is the UV half. Two
things are needed, and they are independent: a display that emits UV, and scene
content that says what is UV-bright. For an indoor digital twin, the cheapest
answer to the second is to control the lighting of the real room rather than to
measure its ultraviolet.

## 1. What the mouse eye sees

- Two cone opsins: **S at about 360 nm** (UV) and **M at about 508 nm** (green).
- About 5 % are true S cones, concentrated in the ventral retina; the rest
  co-express both opsins along a dorsal–ventral gradient. The ventral retina
  looks at the **upper** visual field, so the sky half of the world is
  UV-dominated — which matches where UV contrast actually is outdoors.
- Rods (about 498 nm) dominate at low light and are a third photoreceptor to
  worry about in the mesopic range. Photopic stimulation keeps the problem
  two-dimensional.
- Acuity is low, roughly 0.5 cycles per degree by optomotor measurements, so
  fine detail in a photorealistic scene is invisible to the animal. The realism
  that can matter is structure, depth, parallax and scene statistics — not
  texture detail.

**Consequence for stimulus design:** matching mouse colour means matching the
**S and M photoisomerisation rates**, not the RGB values. With two primaries
whose spectra are known, that is a 2 × 2 linear problem, and it is invertible as
long as the primaries differ in how they excite the two opsins.

## 2. The display

An LCD or OLED emits essentially nothing below 400 nm; its blue primary (about
450 nm) hits S-opsin only weakly. On a normal screen a mouse is close to an
M-cone monochromat. Options:

| Approach | What it gives | Cost |
|---|---|---|
| **Ordinary screen, green channel only** | M-cone stimulation, no UV | none — what most mouse VR does today |
| **DLP projector with UV + green LEDs** | both cone classes, independently | a projector build; the Euler lab's open stimulator is the reference design |
| **Two-projector or LED-backlit dome** | same, over a wide field | more optics, more calibration |

Practical points for a UV build:
- **Optics:** ordinary glass and plastic lenses absorb UV. UV-transmissive
  optics are needed, and the DMD window must be rated for the wavelength.
- **Screen:** most projection materials absorb UV. PTFE sheet and some white
  paints reflect it well; measure before trusting a surface.
- **Safety and stray light:** UV-A at stimulus levels is not a hazard like UV-B,
  but it must be measured, and it is invisible to the experimenter, so it needs
  an interlock or a marked beam path.
- **Calibration:** a spectrometer per primary, then convert to
  photoisomerisations per cone per second using standard opsin templates. Also
  measure each primary's intensity against drive level: the response is not the
  sRGB curve, and stimuli should be specified in cone activation.

## 3. The content: where does UV information come from?

A splat scene trained from phone photos has **no UV channel**, and UV
reflectance cannot be guessed from visible colour — optical brighteners in
paper and paint, many plastics, and the sky all behave differently in UV than
they look. Four options, cheapest first:

1. **Ignore UV: drive M cones only.** Render from the green channel. Defensible
   when every condition is treated alike, and it is what current mouse VR does.
   What is lost is the UV contrast of the upper visual field, which matters most
   for overhead and looming stimuli.
2. **Light the real room with the display's primaries** (recommended for a
   digital twin). Illuminate the corridor with the same UV and green LEDs the
   projector uses, and photograph it through filters matching those bands. Then
   the capture, the real environment and the display all live in the same
   two-dimensional colour space, and the twin is spectrally matched *by
   construction* rather than by inference. It also removes the mismatch between
   what the mouse sees in the real arena and in its replica — which is the whole
   point of that experiment.
3. **Note that indoor LED light is already UV-poor.** A corridor lit by ordinary
   LED or fluorescent fixtures emits very little UV, so the real scene is close
   to UV-dark for the mouse, and a green-only twin is closer to faithful than it
   sounds. Worth measuring the room's spectrum before building anything: the
   answer may be that there is nothing to reproduce.
4. **Capture UV as a separate channel.** A full-spectrum camera with a UV-pass
   filter, shot from the same positions; the visible images give the poses and
   the geometry, the UV images train an extra colour channel. This needs trainer
   support for more than three channels (gsplat handles arbitrary feature
   dimensions more readily than Brush) and a rig that keeps the two cameras
   registered. Only worth it for daylight or outdoor scenes.

## 4. What vstimd would need

Nothing in the renderer knows about primaries today: stimuli carry linear RGBA
(`daemon/src/color.rs`), and blending happens in display space. Two additions,
in order of usefulness:

1. **A per-rig output transform.** A 3 × 3 matrix (plus per-channel levels)
   in the rig-config, applied once to the finished frame, mapping the scene's
   channels to the display's primaries — for example routing what stimuli call
   "blue" to the UV LED. It belongs to the rig, not the experiment, so it
   belongs in `vstimd-rig-config.toml` beside the display mode.
2. **Cone-space colours.** Let a stimulus specify `s`/`m` cone activation
   instead of RGB, converted at upload through the rig's calibration. This is
   the honest interface for a colour experiment, and it is orthogonal to
   splats — gratings and shapes want it too.

Open questions for that work:

- **Where in the pipeline.** 2-D stimuli render straight into the swapchain
  today, so an output transform means rendering into an offscreen target and
  adding a final full-screen pass. That is one more image and one more pass per
  frame; it must not cost a frame on the Jetson.
- **What must bypass it.** The photodiode patch has to stay at the display's
  maximum output, and the operator overlay should stay readable in human
  colours. Both argue for applying the transform before the overlay and the
  patch are drawn, not to the whole final image.
- **Ordering with blending.** Splats blend premultiplied in linear space; the
  transform has to come after blending, which the offscreen-then-map order gives
  for free.
- **Calibration data.** Where the measured primary spectra and gamma live, and
  how a client can query which colour space the rig is in, so an analysis script
  can record it.

## 5. Controls a reviewer will ask for

- Match **mean cone activation** (S and M separately) across conditions, not
  mean RGB.
- Report stimuli in **photoisomerisations per cone per second**, with the
  measured primaries — that is the field's standard and makes the work
  comparable.
- For "does realism matter" experiments, control for low-level differences:
  phase-scrambled textures, shuffled colours with the same histogram, and a
  depth-flattened version of the same scene.
- State the **rod contribution** at the luminance used, or work photopically.

## 6. Where this sits relative to the splat work

- A **behavioural pilot** (realistic versus abstract corridor) can run on the
  current screens with green-only rendering. No new hardware.
- The **digital twin experiment** is where colour matters, and where option 2 of
  §3 turns a weakness into a design feature.
- The **output transform** (§4.1) is a small, self-contained change to vstimd
  that a UV projector would need on day one; the cone-space colour API is a
  larger piece and should wait until a rig actually has one.

## References

- Franke et al., *An arbitrary-spectrum spatial visual stimulator for vision
  research*, eLife 2019 — the open UV/green DLP design and its calibration
  (<https://elifesciences.org/articles/48779>,
  <https://github.com/eulerlab/open-visual-stimulator>).
- Qiu et al., *Natural environment statistics in the upper and lower visual
  field are reflected in mouse retinal specializations*, Current Biology 2021 —
  UV/green scene statistics from a mouse-perspective camera.
- Nadal-Nicolás et al., *True S-cones are concentrated in the ventral mouse
  retina and wired for color detection in the upper visual field*, eLife 2020 —
  the cone populations and the dorsoventral gradient.
- Rhim et al., *Maps of cone opsin input to mouse V1 and higher visual areas*,
  J Neurophysiol 2017 — a worked UV/green display used for cortical mapping.
- Wang et al., *Spectral and temporal sensitivity of cone-mediated responses in
  mouse retinal ganglion cells*, J Neurosci 2011 — S ≈ 360 nm, M ≈ 508 nm.
