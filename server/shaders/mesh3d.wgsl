// 3-D meshes: shared unit geometry placed by a per-object model matrix.
//
// Set 0 is the per-frame scene uniform (camera + lighting); per-object data is
// push constants. Both layouts are mirrored in `render/vk/vk_mesh3d_pipeline.rs`
// (`SceneUniform`, `Mesh3dPushConstants`) with size assertions.

struct Scene {
    view_proj:  mat4x4<f32>,
    camera_pos: vec3<f32>, _pad0: f32,
    ambient:    vec3<f32>, _pad1: f32,  // lighting arrives with #71
    sun_dir:    vec3<f32>, _pad2: f32,
    sun_color:  vec3<f32>, _pad3: f32,
}
@group(0) @binding(0) var<uniform> scene: Scene;

struct Object {
    model:    mat4x4<f32>,
    albedo:   vec4<f32>,
    emissive: vec3<f32>,
    shading:  u32,        // 0 = Unlit; Phong arrives with #71
}
var<push_constant> object: Object;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal:   vec3<f32>,
    @location(2) uv:       vec2<f32>,
    @location(3) color:    vec4<f32>,
}
struct VertexOutput {
    @builtin(position) clip_pos:  vec4<f32>,
    @location(0)       world_pos: vec3<f32>,
    @location(1)       normal:    vec3<f32>,
    @location(2)       uv:        vec2<f32>,
    @location(3)       color:     vec4<f32>,
}

// WGSL has no inverse(). Columns a, b, c; the rows of the inverse are the
// pairwise cross products over the determinant.
fn inverse3(m: mat3x3<f32>) -> mat3x3<f32> {
    let a = m[0];
    let b = m[1];
    let c = m[2];
    let r0 = cross(b, c);
    let r1 = cross(c, a);
    let r2 = cross(a, b);
    let inv_det = 1.0 / dot(a, r0);
    return transpose(mat3x3<f32>(r0, r1, r2)) * inv_det;
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    // The unit meshes are scaled non-uniformly into boxes and ellipsoids, so the
    // normal matrix must be the real inverse-transpose, not mat3(model).
    let m = object.model;
    let linear = mat3x3<f32>(m[0].xyz, m[1].xyz, m[2].xyz);
    let world = m * vec4<f32>(in.position, 1.0);

    var out: VertexOutput;
    out.clip_pos  = scene.view_proj * world;
    out.world_pos = world.xyz;
    out.normal    = transpose(inverse3(linear)) * in.normal;
    out.uv        = in.uv;
    out.color     = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Unlit: exactly albedo × vertex colour, so an unlit 3-D surface matches a
    // 2-D shape of the same colour. `normalize(in.normal)` belongs here once
    // shading needs it — interpolation does not preserve length.
    return object.albedo * in.color;
}
