// Stage 1 of the tile rasteriser: project every splat once, cull it, and count
// the 16×16 tiles it touches.
//
// The hardware path (`splat.wgsl`) does this work in the vertex shader, four
// times per splat — once per quad corner — and then hands the fragment stage a
// quad it must blend in full. Here it happens once, and what comes out is a
// compact record the rasteriser reads from shared memory.
//
// Layouts are mirrored in `render/vk/vk_splat_tile.rs` with size assertions.

struct Splat {
    position: vec3<f32>,
    color:    u32,
    cov_a:    vec3<f32>,
    _pad0:    f32,
    cov_b:    vec3<f32>,
    _pad1:    f32,
}

// One splat as the rasteriser wants it: all scalars, so WGSL's vec3 alignment
// rules cannot pad the stride out from under the Rust side.
struct Projected {
    mean_x:  f32,
    mean_y:  f32,
    // Inverse of the 2-D covariance [[a, b], [b, c]], which is what evaluating
    // the Gaussian at a pixel actually needs.
    conic_a: f32,
    conic_b: f32,
    conic_c: f32,
    // View depth, tested against the mesh depth buffer.
    depth:   f32,
    color:   u32,   // RGBA8; alpha is the splat's own opacity
    alpha:   f32,   // that alpha times the stimulus opacity
    // Inclusive tile rectangle. min > max means culled: no tiles, no work.
    tile_min_x: u32,
    tile_min_y: u32,
    tile_max_x: u32,
    tile_max_y: u32,
}

@group(0) @binding(0) var<storage, read>       splats:    array<Splat>;
@group(0) @binding(1) var<storage, read>       order:     array<u32>;
@group(0) @binding(2) var<storage, read_write> projected: array<Projected>;
@group(0) @binding(3) var<storage, read_write> tile_counts: array<atomic<u32>>;

struct Push {
    model_view:  mat4x4<f32>,
    proj:        vec4<f32>,
    viewport_px: vec2<f32>,
    opacity:     f32,
    alpha_floor: f32,
    tiles:       vec2<u32>,   // tile grid width, height
    count:       u32,         // splats in this cloud
    _pad:        u32,
}
var<push_constant> pc: Push;

const TILE_PX: f32 = 16.0;
const MAX_EXTENT: f32 = 2.0;
const DILATION: f32 = 0.3;

fn cull(out: ptr<function, Projected>) {
    // An empty tile rectangle: the scatter and the rasteriser both skip it.
    (*out).tile_min_x = 1u;
    (*out).tile_max_x = 0u;
    (*out).tile_min_y = 1u;
    (*out).tile_max_y = 0u;
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= pc.count) {
        return;
    }

    var out: Projected;
    out.mean_x = 0.0;
    out.mean_y = 0.0;
    out.conic_a = 0.0;
    out.conic_b = 0.0;
    out.conic_c = 0.0;
    out.depth = 0.0;
    out.color = 0u;
    out.alpha = 0.0;
    cull(&out);

    // `order` is the global back-to-front order the CPU sorter maintains, so
    // slot i holds the i-th splat from the back. The rasteriser walks each
    // tile's list forwards, which is front-to-back, which is what lets it stop
    // early — so the scatter reverses, not this.
    let s = splats[order[i]];
    let t = pc.model_view * vec4<f32>(s.position, 1.0);
    let depth = -t.z;
    let clip = vec4<f32>(pc.proj.x * t.x, pc.proj.y * t.y, pc.proj.z * t.z + pc.proj.w, depth);
    if (clip.z < 0.0 || depth <= 0.0 || abs(clip.x) > 1.3 * clip.w || abs(clip.y) > 1.3 * clip.w) {
        projected[i] = out;
        return;
    }

    let rgba = unpack4x8unorm(s.color);
    let alpha = rgba.a * pc.opacity;
    if (alpha * 255.0 <= pc.alpha_floor) {
        projected[i] = out;
        return;
    }

    // Same EWA projection as the hardware path, so the two agree pixel for
    // pixel where they can: Σ' = J·W·Σ·Wᵀ·Jᵀ.
    let focal = vec2<f32>(pc.proj.x, pc.proj.y) * pc.viewport_px * 0.5;
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

    let det = a * c - b * b;
    if (det <= 0.0) {
        projected[i] = out;
        return;
    }
    // Inverse of [[a, b], [b, c]].
    let inv_det = 1.0 / det;
    out.conic_a = c * inv_det;
    out.conic_b = -b * inv_det;
    out.conic_c = a * inv_det;

    // Screen position, in pixels from the top-left.
    let ndc = clip.xy / clip.w;
    let mean = (ndc * 0.5 + vec2<f32>(0.5, 0.5)) * pc.viewport_px;
    out.mean_x = mean.x;
    out.mean_y = mean.y;
    out.depth = depth;
    out.color = s.color;
    out.alpha = alpha;

    // The radius at which this splat stops being drawable, in pixels. Same
    // rule as the hardware path: exp(−r²)·alpha falls to the floor.
    let extent = min(MAX_EXTENT, sqrt(log(alpha * 255.0 / pc.alpha_floor)));
    // Axis-aligned bound of the ellipse at that radius. Eigenvalues of the
    // covariance give the half-axes; the bound only needs the larger.
    let mid = 0.5 * (a + c);
    let radius = length(vec2<f32>(0.5 * (a - c), b));
    let lambda1 = mid + radius;
    let bound = extent * sqrt(2.0 * lambda1);

    let lo = vec2<f32>(mean.x - bound, mean.y - bound);
    let hi = vec2<f32>(mean.x + bound, mean.y + bound);
    let tiles = vec2<f32>(f32(pc.tiles.x), f32(pc.tiles.y));
    let tmin = clamp(floor(lo / TILE_PX), vec2<f32>(0.0), tiles - vec2<f32>(1.0));
    let tmax = clamp(floor(hi / TILE_PX), vec2<f32>(0.0), tiles - vec2<f32>(1.0));
    // Entirely off-screen: the clamp above collapses it onto an edge tile, so
    // check the unclamped box instead.
    if (hi.x < 0.0 || hi.y < 0.0 || lo.x > tiles.x * TILE_PX || lo.y > tiles.y * TILE_PX) {
        projected[i] = out;
        return;
    }
    out.tile_min_x = u32(tmin.x);
    out.tile_min_y = u32(tmin.y);
    out.tile_max_x = u32(tmax.x);
    out.tile_max_y = u32(tmax.y);
    projected[i] = out;

    // Histogram: one bump per tile this splat lands in. The scan that follows
    // turns these into the per-tile offsets the scatter writes at.
    for (var ty = out.tile_min_y; ty <= out.tile_max_y; ty = ty + 1u) {
        for (var tx = out.tile_min_x; tx <= out.tile_max_x; tx = tx + 1u) {
            atomicAdd(&tile_counts[ty * pc.tiles.x + tx], 1u);
        }
    }
}
