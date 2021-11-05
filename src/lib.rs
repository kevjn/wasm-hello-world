use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::CanvasRenderingContext2d;
use web_sys::{WebGlProgram, WebGl2RenderingContext, WebGlShader};

struct Peer {
    lat: f64,
    lon: f64,
    x: f64,
    y: f64
}

impl Peer {
    fn new(lat: f64, lon: f64) -> Peer {
        // convert degrees to radians
        let (lat, lon) = (lat * std::f64::consts::PI / 180.0, lon * std::f64::consts::PI / 180.0);
        // calculate 2d cartesian coordinates
        let (x,y, _) = Peer::cartesian(lat, lon);

        Peer {lat, lon, x, y}
    }

    fn cartesian(lat: f64, lon: f64) -> (f64, f64, f64) {
        (lat.cos() * lon.sin(), lat.sin(), lat.cos() * (lon + std::f64::consts::PI).cos())
    }

    fn rotate(&mut self, lat: f64, lon: f64) {
        let (x, y, z) = Peer::cartesian(self.lat, self.lon);

        self.x = lon.cos() * x - lon.sin() * z;
        self.y = lat.cos() * y + lat.sin() * lon.cos() * z + lat.sin() * lon.sin() * x;
    }

    fn draw(&self, context: &CanvasRenderingContext2d) {
        context.begin_path();
        context.arc(self.x * 360.0 + 360.0, -self.y * 360.0 + 360.0, 4.0, 0.0, std::f64::consts::PI * 2.0).unwrap();
        context.set_fill_style(&JsValue::from_str("red"));
        context.fill();
    }
}

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
        in vec2 pos;
        const float PI = 3.1415926535897932384626433832795;
        uniform vec2 angle;

        float degToRad(float v) {
            return v * PI / 180.0;
        }

        mat4 rotateY(float r) {
            float s = sin(r), c = cos(r);
            // left handed rotation
            return mat4(c,   0.0, -s,  0.0, 
                        0.0, 1.0, 0.0, 0.0, 
                        s,   0.0, c,   0.0, 
                        0.0, 0.0, 0.0, 1.0);
        }

        mat4 rotateX(float r) {
            float s = sin(r), c = cos(r);
            // left handed rotation
            return mat4(1.0, 0.0, 0.0, 0.0, 
                        0.0, c,   s,   0.0, 
                        0.0, -s,  c,   0.0, 
                        0.0, 0.0, 0.0, 1.0);
        }

        void main() {
            float lat = degToRad(pos.y);
            float lon = degToRad(pos.x);

            float x = cos(lat) * sin(lon);
            float y = sin(lat);
            float z = cos(lat) * cos(lon);

            gl_Position = rotateX(angle[0]) * rotateY(angle[1]) * vec4(x, y, z, 1.0);
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

    let position_attribute_location = context.get_attrib_location(&program, "pos");
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

    context.enable(WebGl2RenderingContext::DEPTH_TEST);

    let location = context.get_uniform_location(&program, "angle");
    let angle = Rc::new(Cell::new([0.0,0.0]));
    context.uniform2fv_with_f32_array(location.as_ref(), &angle.get());

    let canvas = document.get_element_by_id("peers_canvas").unwrap();
    let canvas: web_sys::HtmlCanvasElement = canvas.dyn_into::<web_sys::HtmlCanvasElement>()?;

    // add some dummy data
    let peers = vec![
        Peer::new(59.334591, 18.063240),  // Stockholm
        Peer::new(51.509865, -0.118092),  // London
        Peer::new(40.730610, -73.935252), // New York
        Peer::new(-19.002846, 46.460938), // Madagascar
    ];

    let peers = Rc::new(RefCell::new(peers));

    // closure for drawing globe and peers
    let draw = {
        let peers = peers.clone();
        let canvas_context = canvas.get_context("2d")?.unwrap().dyn_into::<web_sys::CanvasRenderingContext2d>()?;
        move |lat: f32, lon: f32| {
            // draw globe using webgl context
            context.uniform2fv_with_f32_array(location.as_ref(), &[lat, lon]);
            context.clear_color(0.98, 0.98, 0.98, 1.0);
            context.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);
            context.draw_arrays(WebGl2RenderingContext::LINE_STRIP, 0, vert_count);

            // draw globe frame
            canvas_context.clear_rect(0.0, 0.0, 720.0, 720.0);
            canvas_context.begin_path();
            canvas_context.arc(360.0, 360.0, 360.0, 0.0, std::f64::consts::PI * 2.0).unwrap();
            canvas_context.stroke();
            canvas_context.close_path();
            
            // draw peers on map
            for p in peers.borrow_mut().iter_mut() {
                p.rotate(lat.into(), lon.into());
                p.draw(&canvas_context);
            }
        }
    };

    draw(0.0, 0.0);

    // add mouse handlers
    let pressed = Rc::new(Cell::new(false));

    // mouse down event
    {
        let pressed = pressed.clone();
        let closure: Closure<dyn FnMut(_)> = Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| {
            pressed.set(true);
        }));
        canvas.add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // mouse move event
    {
        let pressed = pressed.clone();
        let angle = angle.clone();
        let draw = draw.clone();
        let closure: Closure<dyn FnMut(_)> = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
            if pressed.get() {
                let [mut lat, mut lon] = angle.get();
                lat += (event.movement_y() as f32) / 720.0;
                lon += (event.movement_x() as f32) / 720.0;

                // clamp angle for north/south pole
                lat = lat.clamp(-1.0, 1.0);

                angle.set([lat, lon]);

                draw(lat, lon);
            }
        }));
        document.add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // mouse release event
    {
        let pressed = pressed.clone();
        let closure: Closure<dyn FnMut(_)> = Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| {
            pressed.set(false);
        }));
        document.add_event_listener_with_callback("mouseup", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // mouse click event
    {
        let closure: Closure<dyn FnMut(_)> = Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
            peers.borrow().iter().for_each(|peer| {
                let (px, py) = (360.0 + peer.x *  360.0, 360.0 - peer.y * 360.0);
                if (e.client_x() as f64 - px).abs() < 10.0 && (e.client_y() as f64 - py).abs() < 10.0 {
                    let [mut lat, mut lon] = angle.get();
                    let (dlat, dlon) = (peer.lat as f32 - lat, peer.lon as f32 + lon);
                    let angle = angle.clone();

                    // simulate rotation
                    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
                    let g = f.clone();
                    let mut i = 0;
                    {
                        let draw = draw.clone();
                        *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
                            if i > 50 {
                                angle.set([lat, lon]);
                                let _ = f.borrow_mut().take();
                                return;
                            }

                            i += 1;

                            lat += dlat / 50.0;
                            lon -= dlon / 50.0;

                            draw(lat, lon);

                            window().request_animation_frame(f.borrow().as_ref().unwrap().as_ref().unchecked_ref())
                                .expect("failed requesting animation frame");
                        }) as Box<dyn FnMut()>));

                        window().request_animation_frame(g.borrow().as_ref().unwrap().as_ref().unchecked_ref())
                            .expect("failed requesting animation frame");
                    }
                }
            });
        }));

        canvas.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    Ok(())

}

fn window() -> web_sys::Window {
    web_sys::window().expect("no global `window` exists")
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

