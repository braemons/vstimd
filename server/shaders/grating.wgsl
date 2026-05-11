// Grating stimulus fragment shader.
//
// The vertex shader emits NDC positions for a simple axis-aligned quad that
// covers the grating patch.  All pattern logic lives in the fragment shader,
// which reconstructs pixel-space coordinates from gl_FragCoord and the push
// constants, so no UV attributes are needed on the vertices.
//
// Push constant layout (80 bytes, std430):
//   screen_half  vec2<f32>   half screen dimensions in pixels (for coord conversion)
//   center_px    vec2<f32>   grating centre in pixel-space (Y-up)
//   half_size    vec2<f32>   patch half-extents [hw, hh] in pixels
//   sf           f32         spatial frequency in cycles/pixel
//   phase        f32         total phase (static + accumulated drift) in [0,1]
//   ori_rad      f32         stripe orientation in radians (CCW from X axis)
//   contrast     f32         [0, 1]
//   color        vec4<f32>   peak colour (rgb) and opacity (a)
//   waveform     u32         0=sin  1=sqr  2=saw  3=tri
//   mask_type    u32         0=none  1=circle  2=gauss
//   _pad         vec2<u32>   alignment padding

struct PushConstants {
    screen_half : vec2<f32>,
    center_px   : vec2<f32>,
    half_size   : vec2<f32>,
    sf          : f32,
    phase       : f32,
    ori_rad     : f32,
    contrast    : f32,
    color       : vec4<f32>,
    waveform    : u32,
    mask_type   : u32,
    _pad        : vec2<u32>,
}

var<push_constant> p: PushConstants;

// ── Vertex stage ──────────────────────────────────────────────────────────────

struct VertexInput {
    @location(0) position : vec3<f32>,
    @location(1) normal   : vec3<f32>,
    @location(2) uv       : vec2<f32>,
    @location(3) color    : vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_pos : vec4<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_pos = vec4<f32>(in.position, 1.0);
    return out;
}

// ── Fragment stage ────────────────────────────────────────────────────────────

const TAU: f32 = 6.283185307179586;

// Carrier waveforms — all return a value in [-1, 1] for argument t in cycles.
fn waveform_sin(t: f32) -> f32 { return sin(t * TAU); }
fn waveform_sqr(t: f32) -> f32 { return sign(sin(t * TAU)); }
fn waveform_saw(t: f32) -> f32 { return 2.0 * fract(t + 0.5) - 1.0; }
fn waveform_tri(t: f32) -> f32 { return 1.0 - 4.0 * abs(fract(t + 0.75) - 0.5); }

fn eval_waveform(t: f32, waveform: u32) -> f32 {
    switch waveform {
        case 1u:       { return waveform_sqr(t); }
        case 2u:       { return waveform_saw(t); }
        case 3u:       { return waveform_tri(t); }
        default:       { return waveform_sin(t); }
    }
}

// Aperture masks — return alpha in [0, 1].
fn mask_circle(d: vec2<f32>, half_size: vec2<f32>) -> f32 {
    let r = min(half_size.x, half_size.y);
    return select(0.0, 1.0, length(d) <= r);
}

fn mask_gauss(d: vec2<f32>, half_size: vec2<f32>) -> f32 {
    let sigma = min(half_size.x, half_size.y) * 0.5;
    return exp(-dot(d, d) / (2.0 * sigma * sigma));
}

@fragment
fn fs_main(@builtin(position) frag_pos: vec4<f32>) -> @location(0) vec4<f32> {
    // Convert viewport coordinates (origin top-left, Y-down) to pixel-space
    // (origin screen-centre, Y-up) to match the stimulus coordinate system.
    let px = vec2<f32>(
        frag_pos.x - p.screen_half.x,
        p.screen_half.y - frag_pos.y,
    );

    // Offset relative to grating centre.
    let d = px - p.center_px;

    // Clip to bounding rectangle (discard outside the patch).
    if abs(d.x) > p.half_size.x || abs(d.y) > p.half_size.y {
        discard;
    }

    // Project onto the grating axis (perpendicular to stripes).
    let cos_a = cos(p.ori_rad);
    let sin_a = sin(p.ori_rad);
    let u = cos_a * d.x + sin_a * d.y;

    // Evaluate carrier: t in cycles, phase in [0, 1] (wraps naturally via sin/fract).
    let t = u * p.sf + p.phase;
    let carrier = eval_waveform(t, p.waveform);  // [-1, 1]

    // Apply contrast: map [-1, 1] → [0, 1] then centre around 0.5.
    let luminance = 0.5 + 0.5 * carrier * p.contrast;

    // Aperture mask.
    var alpha: f32;
    switch p.mask_type {
        case 1u:  { alpha = mask_circle(d, p.half_size); }
        case 2u:  { alpha = mask_gauss(d, p.half_size); }
        default:  { alpha = 1.0; }
    }
    alpha *= p.color.a;

    return vec4<f32>(p.color.rgb * luminance, alpha);
}
