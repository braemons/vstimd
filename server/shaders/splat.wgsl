// 3-D Gaussian splats: one screen-aligned quad per splat, instanced over a
// back-to-front index buffer, each shaped by its projected 2-D covariance.
//
// Set 0: the splats (binding 0) and the draw order (binding 1), both read-only
// storage buffers. Per-draw data is push constants. Layouts are mirrored in
// `splat/mod.rs` (`GpuSplat`) and `render/vk/vk_splat_pipeline.rs`
// (`SplatPushConstants`) with size assertions.
//
// The projection is the EWA approximation used by the 3DGS reference renderer:
// Σ' = J·W·Σ·Wᵀ·Jᵀ, with W the model-view rotation and J the Jacobian of the
// perspective divide at the splat centre.

struct Splat {
    position: vec3<f32>,
    color:    u32,        // RGBA8, little-endian; alpha is the splat opacity
    cov_a:    vec3<f32>,  // xx, xy, xz
    _pad0:    f32,
    cov_b:    vec3<f32>,  // yy, yz, zz
    _pad1:    f32,
}
@group(0) @binding(0) var<storage, read> splats: array<Splat>;
@group(0) @binding(1) var<storage, read> order: array<u32>;

struct Push {
    model_view:  mat4x4<f32>,
    // The non-trivial entries of the perspective projection: x scale, y scale,
    // and the depth row's scale and offset (clip w is −z_view).
    proj:        vec4<f32>,
    viewport_px: vec2<f32>,
    opacity:     f32,
    _pad:        f32,
}
var<push_constant> pc: Push;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0)       color:    vec4<f32>,
    // Position inside the quad, in units where the Gaussian is exp(−|p|²).
    @location(1)       offset:   vec2<f32>,
}

// Quad half-extent in those units: exp(−4) ≈ 0.018 at the edge.
const EXTENT: f32 = 2.0;
// Low-pass added to the 2-D covariance, px², so a splat never covers less than
// about a pixel (the reference renderer's 0.3).
const DILATION: f32 = 0.3;
// Longest drawn axis, px: bounds the cost of a splat that fills the view.
const MAX_AXIS_PX: f32 = 1024.0;

fn culled() -> VertexOutput {
    var out: VertexOutput;
    // Outside the depth range, so the whole degenerate quad is clipped.
    out.clip_pos = vec4<f32>(0.0, 0.0, 2.0, 1.0);
    out.color = vec4<f32>(0.0);
    out.offset = vec2<f32>(0.0);
    return out;
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> VertexOutput {
    let s = splats[order[instance]];
    let t = pc.model_view * vec4<f32>(s.position, 1.0);
    let depth = -t.z;
    let clip = vec4<f32>(pc.proj.x * t.x, pc.proj.y * t.y, pc.proj.z * t.z + pc.proj.w, depth);
    // Behind the near plane, or well outside the view.
    if (clip.z < 0.0 || depth <= 0.0 || abs(clip.x) > 1.3 * clip.w || abs(clip.y) > 1.3 * clip.w) {
        return culled();
    }

    let focal = vec2<f32>(pc.proj.x, pc.proj.y) * pc.viewport_px * 0.5;
    // Columns of the Jacobian of (fx·x/d, fy·y/d) with respect to view (x, y, z).
    let j = mat3x3<f32>(
        vec3<f32>(focal.x / depth, 0.0, 0.0),
        vec3<f32>(0.0, focal.y / depth, 0.0),
        vec3<f32>(focal.x * t.x / (depth * depth), focal.y * t.y / (depth * depth), 0.0),
    );
    let w = mat3x3<f32>(pc.model_view[0].xyz, pc.model_view[1].xyz, pc.model_view[2].xyz);
    let sigma = mat3x3<f32>(
        vec3<f32>(s.cov_a.x, s.cov_a.y, s.cov_a.z),
        vec3<f32>(s.cov_a.y, s.cov_b.x, s.cov_b.y),
        vec3<f32>(s.cov_a.z, s.cov_b.y, s.cov_b.z),
    );
    let m = j * w;
    let cov = m * sigma * transpose(m);
    let a = cov[0][0] + DILATION;
    let b = cov[0][1];
    let c = cov[1][1] + DILATION;

    // Eigen-decomposition of the symmetric 2×2 [[a, b], [b, c]].
    let mid = 0.5 * (a + c);
    let radius = length(vec2<f32>(0.5 * (a - c), b));
    let lambda1 = mid + radius;
    let lambda2 = mid - radius;
    if (lambda2 <= 0.0) {
        return culled();
    }
    var dir = vec2<f32>(1.0, 0.0);
    if (abs(b) > 1e-6) {
        dir = normalize(vec2<f32>(b, lambda1 - a));
    } else if (c > a) {
        dir = vec2<f32>(0.0, 1.0);
    }
    // sqrt(2λ): the offset unit at which exp(−|p|²) is the Gaussian's value.
    let major = min(sqrt(2.0 * lambda1), MAX_AXIS_PX) * dir;
    let minor = min(sqrt(2.0 * lambda2), MAX_AXIS_PX) * vec2<f32>(dir.y, -dir.x);

    // Triangle-strip corners.
    var corners = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0,  1.0),
    );
    let q = corners[vertex] * EXTENT;
    let offset_px = q.x * major + q.y * minor;
    let ndc = clip.xy / clip.w + offset_px * 2.0 / pc.viewport_px;

    let rgba = unpack4x8unorm(s.color);
    var out: VertexOutput;
    out.clip_pos = vec4<f32>(ndc, clip.z / clip.w, 1.0);
    out.color = vec4<f32>(rgba.rgb, rgba.a * pc.opacity);
    out.offset = q;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let power = -dot(in.offset, in.offset);
    if (power < -EXTENT * EXTENT) {
        discard;
    }
    let alpha = min(0.99, exp(power) * in.color.a);
    if (alpha < 1.0 / 255.0) {
        discard;
    }
    // Premultiplied: the pipeline blends ONE, ONE_MINUS_SRC_ALPHA.
    return vec4<f32>(in.color.rgb * alpha, alpha);
}
