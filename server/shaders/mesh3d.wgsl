// 3-D meshes: shared unit geometry placed by a per-object model matrix.
//
// Set 0 is the per-frame scene uniform (camera + lighting); per-object data is
// push constants. Both layouts are mirrored in `render/vk/vk_mesh3d_pipeline.rs`
// (`SceneUniform`, `Mesh3dPushConstants`) with size assertions.

struct Scene {
    view_proj:  mat4x4<f32>,
    camera_pos: vec3<f32>, _pad0: f32,
    ambient:    vec3<f32>, _pad1: f32,
    sun_dir:    vec3<f32>, _pad2: f32,  // unit, the direction light travels
    sun_color:  vec3<f32>, _pad3: f32,
}
@group(0) @binding(0) var<uniform> scene: Scene;

struct Object {
    model:    mat4x4<f32>,
    albedo:   vec4<f32>,
    emissive: vec3<f32>,
    shading:  u32,        // 0 = Unlit, 1 = Phong
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

// Blinn-Phong specular exponent and weight. Fixed: materials carry no
// shininess, because this is not a PBR renderer.
const SPECULAR_EXPONENT: f32 = 32.0;
const SPECULAR_WEIGHT: f32 = 0.25;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let base = object.albedo * in.color;

    // Unlit is exactly albedo × vertex colour (plus emissive, zero by default):
    // no ambient, no scaling, no encoding. That is what makes an unlit 3-D
    // surface measure the same luminance as a 2-D shape of the same colour.
    if (object.shading == 0u) {
        return vec4<f32>(base.rgb + object.emissive, base.a);
    }

    // Interpolation shortens normals, and the normal matrix scales them.
    let n = normalize(in.normal);
    let l = -scene.sun_dir;
    let v = normalize(scene.camera_pos - in.world_pos);
    let h = normalize(l + v);

    let diffuse = max(dot(n, l), 0.0);
    // step(): no highlight on a surface facing away from the light.
    let specular = pow(max(dot(n, h), 0.0), SPECULAR_EXPONENT) * step(1e-6, diffuse);

    let lit = base.rgb * (scene.ambient + scene.sun_color * diffuse)
            + scene.sun_color * specular * SPECULAR_WEIGHT;
    return vec4<f32>(lit + object.emissive, base.a);
}
