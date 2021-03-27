use cocoa::appkit::NSView;
use metal::*;
use objc::runtime::YES;
use std::error::Error;
use std::mem;
use winit::{
    dpi,
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::macos::WindowExtMacOS,
    window::WindowBuilder,
};

#[repr(C)]
struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[repr(C)]
struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[repr(C)]
struct ClearRect {
    pub rect: Rect,
    pub color: Color,
}

fn prepare_pipeline_state<'a>(
    device: &DeviceRef,
    library: &LibraryRef,
    vertex_shader: &str,
    fragment_shader: &str,
) -> RenderPipelineState {
    let vert = library.get_function(vertex_shader, None).unwrap();
    let frag = library.get_function(fragment_shader, None).unwrap();

    let pipeline_state_descriptor = RenderPipelineDescriptor::new();
    pipeline_state_descriptor.set_vertex_function(Some(&vert));
    pipeline_state_descriptor.set_fragment_function(Some(&frag));
    let attachment = pipeline_state_descriptor
        .color_attachments()
        .object_at(0)
        .unwrap();
    attachment.set_pixel_format(MTLPixelFormat::BGRA8Unorm);

    attachment.set_blending_enabled(true);
    attachment.set_rgb_blend_operation(metal::MTLBlendOperation::Add);
    attachment.set_alpha_blend_operation(metal::MTLBlendOperation::Add);
    attachment.set_source_rgb_blend_factor(metal::MTLBlendFactor::SourceAlpha);
    attachment.set_source_alpha_blend_factor(metal::MTLBlendFactor::SourceAlpha);
    attachment.set_destination_rgb_blend_factor(metal::MTLBlendFactor::OneMinusSourceAlpha);
    attachment.set_destination_alpha_blend_factor(metal::MTLBlendFactor::OneMinusSourceAlpha);

    device
        .new_render_pipeline_state(&pipeline_state_descriptor)
        .unwrap()
}

fn prepare_render_pass_descriptor(descriptor: &RenderPassDescriptorRef, texture: &TextureRef) {
    let color_attachment = descriptor.color_attachments().object_at(0).unwrap();

    color_attachment.set_texture(Some(texture));
    color_attachment.set_load_action(MTLLoadAction::Clear);
    color_attachment.set_clear_color(MTLClearColor::new(0.0, 0.0, 0.0, 1.0));
    color_attachment.set_store_action(MTLStoreAction::Store);
}

fn main() -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_inner_size(dpi::LogicalSize::new(512, 512))
        .with_resizable(false)
        .build(&event_loop)?;

    let device = Device::system_default().ok_or("")?;

    let layer = MetalLayer::new();
    layer.set_device(&device);
    layer.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
    layer.set_presents_with_transaction(false);
    layer.set_framebuffer_only(false);

    unsafe {
        let view = window.ns_view() as cocoa::base::id;
        view.setWantsLayer(YES);
        view.setLayer(mem::transmute(layer.as_ref()));
    }

    layer.set_drawable_size(CGSize::new(
        window.inner_size().width as f64,
        window.inner_size().height as f64,
    ));
    let source = std::fs::read_to_string("src/gpu/main.metal")?;
    let compile = metal::CompileOptions::new();
    let library = device.new_library_with_source(source.as_str(), &compile)?;

    let triangle_pipeline_state =
        prepare_pipeline_state(&device, &library, "triangle_vertex", "triangle_fragment");
    let clear_rect_pipeline_state = prepare_pipeline_state(
        &device,
        &library,
        "clear_rect_vertex",
        "clear_rect_fragment",
    );

    let command_queue = device.new_command_queue();

    let vbuf = {
        let vertex_data = [
            0.0f32, 0.5, 1.0, 0.0, 0.0, -0.5, -0.5, 0.0, 1.0, 0.0, 0.5, 0.5, 0.0, 0.0, 1.0,
        ];

        device.new_buffer_with_data(
            vertex_data.as_ptr() as *const _,
            (vertex_data.len() * mem::size_of::<f32>()) as u64,
            MTLResourceOptions::CPUCacheModeDefaultCache | MTLResourceOptions::StorageModeManaged,
        )
    };

    let mut r = 0.0f32;

    let clear_rect = vec![ClearRect {
        rect: Rect {
            x: -1.0,
            y: -1.0,
            w: 2.0,
            h: 2.0,
        },
        color: Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
    }];

    let clear_rect_buffer = device.new_buffer_with_data(
        clear_rect.as_ptr() as *const _,
        mem::size_of::<ClearRect>() as u64,
        MTLResourceOptions::CPUCacheModeDefaultCache | MTLResourceOptions::StorageModeManaged,
    );

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        match event {
            Event::WindowEvent { event, window_id } if window_id == window.id() => {
                match event {
                    WindowEvent::CloseRequested
                    | WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    } => *control_flow = ControlFlow::Exit,
                    WindowEvent::Resized(size) => {
                        layer.set_drawable_size(CGSize::new(size.width as f64, size.height as f64));
                    }
                    _ => (),
                };
            }
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::RedrawRequested(_) => {
                let p = vbuf.contents();
                let vertex_data = [
                    0.0f32,
                    0.5,
                    1.0,
                    0.0,
                    0.0,
                    -0.5 + (r.cos() / 2. + 0.5),
                    -0.5,
                    0.0,
                    1.0,
                    0.0,
                    0.5 - (r.cos() / 2. + 0.5),
                    -0.5,
                    0.0,
                    0.0,
                    1.0,
                ];

                unsafe {
                    std::ptr::copy(
                        vertex_data.as_ptr(),
                        p as *mut f32,
                        (vertex_data.len() * mem::size_of::<f32>()) as usize,
                    );
                }

                vbuf.did_modify_range(crate::NSRange::new(
                    0 as u64,
                    (vertex_data.len() * mem::size_of::<f32>()) as u64,
                ));

                let drawable = match layer.next_drawable() {
                    Some(drawable) => drawable,
                    None => return,
                };

                let render_pass_descriptor = RenderPassDescriptor::new();

                prepare_render_pass_descriptor(&render_pass_descriptor, drawable.texture());

                let command_buffer = command_queue.new_command_buffer();
                let encoder = command_buffer.new_render_command_encoder(&render_pass_descriptor);

                encoder.set_scissor_rect(MTLScissorRect {
                    x: 20,
                    y: 20,
                    width: 100,
                    height: 100,
                });
                encoder.set_render_pipeline_state(&clear_rect_pipeline_state);
                encoder.set_vertex_buffer(0, Some(&clear_rect_buffer), 0);
                encoder.draw_primitives_instanced(metal::MTLPrimitiveType::TriangleStrip, 0, 4, 1);
                let physical_size = window.inner_size();
                encoder.set_scissor_rect(MTLScissorRect {
                    x: 0,
                    y: 0,
                    width: physical_size.width as _,
                    height: physical_size.height as _,
                });

                encoder.set_render_pipeline_state(&triangle_pipeline_state);
                encoder.set_vertex_buffer(0, Some(&vbuf), 0);
                encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, 3);
                encoder.end_encoding();

                command_buffer.present_drawable(&drawable);
                command_buffer.commit();

                r += 0.01f32;
            }
            _ => (),
        }
    });
}
