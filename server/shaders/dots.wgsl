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
//   dot_shape          u32        0=round  1=square  2=round, anti-aliased edge
//   aperture_shape     u32        0=rect   1=ellipse
//   aperture_invert    u32        draw outside the aperture instead of inside
//   clip_per_pixel     u32        0 = the CPU already culled whole dots by centre
//   global_opacity     f32        shared per-stimulus alpha multiplier
//   pixel_snap         u32        centre each dot on a pixel centre
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
    pixel_snap         : u32,
    _pad               : u32,
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
    // Offset from the dot centre, in pixels — the shape test.
    @location(0) local_px  : vec2<f32>,
    // Position within the field, in pixels — the aperture test.
    @location(1) field_pos : vec2<f32>,
    @location(2) alt       : f32,
    // The dot centre in framebuffer coordinates, for the snapped shape test.
    @location(3) @interpolate(flat) centre_fb : vec2<f32>,
}

// Room outside the radius for the anti-aliased edge's half-pixel ramp.
const SMOOTH_PAD_PX: f32 = 1.0;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    // Screen space, origin at the centre, in pixels. The viewport has a positive
    // height, so framebuffer coordinates are exactly this plus `screen_half`.
    var centre = p.field_center_px + in.dot_pos;
    if (p.pixel_snap != 0u) {
        // Onto the centre of the pixel the dot centre falls in: what a script that
        // rounds a position and blits a mask at that pixel draws.
        centre = floor(centre + p.screen_half) + 0.5 - p.screen_half;
    }
    var extent = p.dot_radius_px;
    if (p.dot_shape == 2u) {
        extent = extent + SMOOTH_PAD_PX;
    }
    let local_px = in.position.xy * extent;
    let pixel_pos = centre + local_px;

    var out: VertexOutput;
    // Clip space is Y-up, matching the grating and text paths — do not negate Y.
    out.clip_pos = vec4<f32>(pixel_pos / p.screen_half, 0.0, 1.0);
    out.local_px = local_px;
    out.field_pos = pixel_pos - p.field_center_px;
    out.alt = in.alt_color;
    out.centre_fb = centre + p.screen_half;
    return out;
}

// ── Fragment stage ────────────────────────────────────────────────────────────

// Is this point inside the aperture? Mirrors `Aperture::contains` on the CPU side,
// which is what decides the same question for `ApertureClip::DotCenter`.
fn in_aperture(field_pos: vec2<f32>) -> bool {
    let d = field_pos - p.aperture_offset_px;
    var inside: bool;
    if (p.aperture_shape == 1u) {
        // Ellipse: normalised to the unit circle by both half-extents.
        let n = d / p.aperture_half;
        inside = dot(n, n) <= 1.0;
    } else {
        inside = abs(d.x) <= p.aperture_half.x && abs(d.y) <= p.aperture_half.y;
    }
    return inside != (p.aperture_invert != 0u);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let r = p.dot_radius_px;
    var coverage = 1.0;
    if (p.pixel_snap != 0u) {
        // Both ends sit on pixel centres, so this offset is a whole number of pixels,
        // exactly — the interpolated `local_px` would carry rounding, and `3-4-5`
        // offsets land right on the radius. Strictly inside, as `dist < radius` is.
        let d = in.clip_pos.xy - in.centre_fb;
        if (p.dot_shape != 1u && dot(d, d) >= r * r) {
            discard;
        }
    } else if (p.dot_shape == 0u) {
        // Round: the quad is a bounding box, and everything outside the inscribed
        // circle is thrown away. No texture, no alpha mask.
        if (dot(in.local_px, in.local_px) > r * r) {
            discard;
        }
    } else if (p.dot_shape == 2u) {
        // Anti-aliased: coverage ramps across the pixel straddling the radius.
        coverage = clamp(r - length(in.local_px) + 0.5, 0.0, 1.0);
        if (coverage <= 0.0) {
            discard;
        }
    }
    // ApertureClip::Pixel — cut the dot at the aperture edge. Under DotCenter the
    // CPU has already dropped the dots that fail, and whole dots overhang the edge
    // on purpose, so this test must not run.
    if (p.clip_per_pixel != 0u && !in_aperture(in.field_pos)) {
        discard;
    }
    let base = mix(p.dot_color, p.alt_color, in.alt);
    return vec4<f32>(base.rgb, base.a * coverage * p.global_opacity);
}
