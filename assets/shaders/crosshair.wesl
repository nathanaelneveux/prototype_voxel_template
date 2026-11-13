import bevy_ui_render::ui_vertex_output::UiVertexOutput;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(floor(in.uv * 15.0));
    let offset = abs(pixel - vec2<i32>(7));
    if !((offset.x == 0 && offset.y <= 4) || (offset.y == 0 && offset.x <= 4)) {
        discard;
    }
    return vec4<f32>(1.0);
}
