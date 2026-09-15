// A full-screen colour at an alpha, drawn last in the 3-D pass: fades the 3-D
// view towards the background (a finite navigation track's fade) without
// touching the 2-D pass drawn over it. No vertex buffer — one triangle that
// covers the viewport, from the vertex index.

struct Push {
    color: vec4<f32>,  // straight alpha
}
var<push_constant> pc: Push;

@vertex
fn vs_main(@builtin(vertex_index) vertex: u32) -> @builtin(position) vec4<f32> {
    var corners = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    return vec4<f32>(corners[vertex], 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return pc.color;
}
