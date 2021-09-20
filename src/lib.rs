use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{WebGlProgram, WebGl2RenderingContext, WebGlShader};

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let document = web_sys::window().unwrap().document().unwrap();
    let canvas = document.get_element_by_id("canvas").unwrap();
    let canvas: web_sys::HtmlCanvasElement = canvas.dyn_into::<web_sys::HtmlCanvasElement>()?;

    let context = canvas
        .get_context("webgl2")?
        .unwrap()
        .dyn_into::<WebGl2RenderingContext>()?;

    let vert_shader = compile_shader(
        &context,
        WebGl2RenderingContext::VERTEX_SHADER,
        r##"#version 300 es
        in vec2 position;
        const float PI = 3.1415926535897932384626433832795;

        float degToRad(float v) {
            return v * PI / 180.0;
        }

        void main() {
            float lat = degToRad(position.y);
            float lon = degToRad(position.x);

            float z = cos(lat) * cos(lon);
            float x = cos(lat) * sin(lon);
            float y = sin(lat);

            gl_Position = vec4(x, y, z, 1.0);
        }
    "##,
    )?;
    let frag_shader = compile_shader(
        &context,
        WebGl2RenderingContext::FRAGMENT_SHADER,
        r##"#version 300 es
        precision highp float;
        out vec4 outColor;

        void main() {
            if (gl_FragCoord.z > 0.5)
              outColor = vec4(0.0, 0.8, 0.0, 1.0); // green
            else
              outColor = vec4(0.8, 0.8, 0.8, 1.0);

        }
    "##,
    )?;
    let program = link_program(&context, &vert_shader, &frag_shader)?;
    context.use_program(Some(&program));

    let position_attribute_location = context.get_attrib_location(&program, "position");
    let buffer = context.create_buffer().ok_or("failed to create buffer")?;
    context.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&buffer));

    // Note that `Float32Array::view` is somewhat dangerous (hence the
    // `unsafe`!). This is creating a raw view into our module's
    // `WebAssembly.Memory` buffer, but if we allocate more pages for ourself
    // (aka do a memory allocation in Rust) it'll cause the buffer to change,
    // causing the `Float32Array` to be invalid.
    //
    // As a result, after `Float32Array::view` we have to be very careful not to
    // do any memory allocations before it's dropped.
    let vert_count = unsafe {
        let bytes = include_bytes!("../lines.bytes");
        let vertices = std::slice::from_raw_parts(bytes.as_ptr() as *const f32, bytes.len() >> 2);
        let positions_array_buf_view = js_sys::Float32Array::view(&vertices.to_owned());
        // let positions_array_buf_view = js_sys::Float32Array::view_mut_raw(bytes.as_ptr() as *mut f32, bytes.len() >> 2);

        context.buffer_data_with_array_buffer_view(
            WebGl2RenderingContext::ARRAY_BUFFER,
            &positions_array_buf_view,
            WebGl2RenderingContext::STATIC_DRAW,
        );

        bytes.len() as i32 >> 3
    };

    let vao = context
        .create_vertex_array()
        .ok_or("Could not create vertex array object")?;
    context.bind_vertex_array(Some(&vao));

    context.vertex_attrib_pointer_with_i32(0, 2, WebGl2RenderingContext::FLOAT, false, 0, 0);
    context.enable_vertex_attrib_array(position_attribute_location as u32);

    context.bind_vertex_array(Some(&vao));

    context.enable(WebGl2RenderingContext::DEPTH_TEST); // maybe remove this

    draw(&context, vert_count);

    Ok(())
}

fn draw(context: &WebGl2RenderingContext, vert_count: i32) {
    context.clear_color(0.98, 0.98, 0.98, 1.0);
    context.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);

    context.draw_arrays(WebGl2RenderingContext::LINE_STRIP, 0, vert_count);
}

pub fn compile_shader(context: &WebGl2RenderingContext, shader_type: u32, source: &str) -> Result<WebGlShader, String> {
    let shader = context
        .create_shader(shader_type)
        .ok_or_else(|| String::from("Unable to create shader object"))?;
    context.shader_source(&shader, source);
    context.compile_shader(&shader);

    if context
        .get_shader_parameter(&shader, WebGl2RenderingContext::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        Err(context
            .get_shader_info_log(&shader)
            .unwrap_or_else(|| String::from("Unknown error creating shader")))
    }
}

pub fn link_program(context: &WebGl2RenderingContext, vert_shader: &WebGlShader, frag_shader: &WebGlShader) -> Result<WebGlProgram, String> {
    let program = context
        .create_program()
        .ok_or_else(|| String::from("Unable to create shader object"))?;

    context.attach_shader(&program, vert_shader);
    context.attach_shader(&program, frag_shader);
    context.link_program(&program);

    if context
        .get_program_parameter(&program, WebGl2RenderingContext::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(program)
    } else {
        Err(context
            .get_program_info_log(&program)
            .unwrap_or_else(|| String::from("Unknown error creating program object")))
    }
}

