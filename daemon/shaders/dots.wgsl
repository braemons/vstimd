// Random dot kinematogram.
//
// One shared unit quad, instanced once per dot. The vertex shader places each
// instance; the fragment shader rounds it and, when asked, cuts it at the aperture.
// Nothing about the field is geometry, so there is no mesh and no tessellation.
//
// Push constant layout (96 bytes, std430) — everything here is per *field*; the
// instance buffer carries only what differs between dots:
//   screen_half        vec2<f32>  half screen dimensions in pixels
//   field_center_px    vec2<f32>  the stimulus position, screen space, Y-up
//   aperture_offset_px vec2<f32>  aperture centre relative to the field centre
//   aperture_half      vec2<f32>  aperture half-extents in pixels
//   dot_radius_px      f32        half of dot_size_px
//   dot_shape          u32        0=round  1=square
//   aperture_shape     u32        0=rect   1=circle
//   aperture_invert    u32        draw outside the aperture instead of inside
//   clip_per_pixel     u32        0 = the CPU already culled whole dots by centre
//   global_opacity     f32        shared per-stimulus alpha multiplier
//   dot_color          vec4<f32>
//   alt_color          vec4<f32>

struct PushConstants {
    screen_half        : vec2<f32>,
    field_center_px    : vec2<f32>,
    aperture_offset_px : vec2<f32>,
    aperture_half      : vec2<f32>,
    dot_radius_px      : f32,
    dot_shape          : u32,
    aperture_shape     : u32,
    aperture_invert    : u32,
    clip_per_pixel     : u32,
    global_opacity     : f32,
    _pad0              : u32,
    _pad1              : u32,
    dot_color          : vec4<f32>,
    alt_color          : vec4<f32>,
}

var<push_constant> p: PushConstants;

// ── Vertex stage ──────────────────────────────────────────────────────────────

struct VertexInput {
    // The shared quad: a corner in [-1, 1]².
    @location(0) position  : vec3<f32>,
    // Per instance: the dot centre, field-local pixels, and which colour it took.
    @location(1) dot_pos   : vec2<f32>,
    @location(2) alt_color : f32,
}

struct VertexOutput {
    @builtin(position) clip_pos : vec4<f32>,
    // Position within this dot, in [-1, 1]² — the round test.
    @location(0) local     : vec2<f32>,
    // Position within the field, in pixels — the aperture test.
    @location(1) field_pos : vec2<f32>,
    @location(2) alt       : f32,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    let field_pos = in.dot_pos + in.position.xy * p.dot_radius_px;
    let pixel_pos = p.field_center_px + field_pos;
    let ndc = pixel_pos / p.screen_half;

    var out: VertexOutput;
    // Clip space is Y-up, matching the grating and text paths — do not negate Y.
    out.clip_pos = vec4<f32>(ndc.x, ndc.y, 0.0, 1.0);
    out.local = in.position.xy;
    out.field_pos = field_pos;
    out.alt = in.alt_color;
    return out;
}

// ── Fragment stage ────────────────────────────────────────────────────────────

// Is this point inside the aperture? Mirrors `Aperture::contains` on the CPU side,
// which is what decides the same question for `ApertureClip::DotCenter`.
fn in_aperture(field_pos: vec2<f32>) -> bool {
    let d = field_pos - p.aperture_offset_px;
    var inside: bool;
    if (p.aperture_shape == 1u) {
        // Circle: aperture_half.x is the radius (the diameter was halved on the way in).
        inside = dot(d, d) <= p.aperture_half.x * p.aperture_half.x;
    } else {
        inside = abs(d.x) <= p.aperture_half.x && abs(d.y) <= p.aperture_half.y;
    }
    return inside != (p.aperture_invert != 0u);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Round dots: the quad is a bounding box, and everything outside the inscribed
    // circle is thrown away. No texture, no alpha mask.
    if (p.dot_shape == 0u && dot(in.local, in.local) > 1.0) {
        discard;
    }
    // ApertureClip::Pixel — cut the dot at the aperture edge. Under DotCenter the
    // CPU has already dropped the dots that fail, and whole dots overhang the edge
    // on purpose, so this test must not run.
    if (p.clip_per_pixel != 0u && !in_aperture(in.field_pos)) {
        discard;
    }
    let base = mix(p.dot_color, p.alt_color, in.alt);
    return vec4<f32>(base.rgb, base.a * p.global_opacity);
}
