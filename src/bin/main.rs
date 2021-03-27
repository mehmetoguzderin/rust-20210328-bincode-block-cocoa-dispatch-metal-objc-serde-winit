use cocoa::appkit::NSView;
use metal::*;
use std::error::Error;
use std::mem;
use winit::{
    dpi,
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::macos::WindowExtMacOS,
    window::WindowBuilder,
};

fn main() -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_inner_size(dpi::LogicalSize::new(512, 512))
        .build(&event_loop)?;
    let device = Device::system_default().ok_or("")?;
    let layer = MetalLayer::new();
    layer.set_device(&device);
    layer.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
    layer.set_presents_with_transaction(false);
    layer.set_framebuffer_only(false);
    layer.set_drawable_size(CGSize::new(
        window.inner_size().width as f64,
        window.inner_size().height as f64,
    ));
    unsafe {
        let view = window.ns_view() as cocoa::base::id;
        view.setWantsLayer(objc::runtime::YES);
        view.setLayer(mem::transmute(layer.as_ref()));
    }
    let library = {
        let source = std::fs::read_to_string("src/gpu/main.metal")?;
        let options = metal::CompileOptions::new();
        device.new_library_with_source(source.as_str(), &options)?
    };
    let compute_pipeline_state = {
        let compute_function = library.get_function("compute", None)?;
        let descriptor = ComputePipelineDescriptor::new();
        descriptor.set_compute_function(Some(&compute_function));
        let function = descriptor.compute_function().ok_or("")?;
        device.new_compute_pipeline_state_with_function(function)?
    };
    let command_queue = device.new_command_queue();
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
                if let Some(drawable) = layer.next_drawable() {
                    let command_buffer = command_queue.new_command_buffer();
                    let encoder = command_buffer.new_compute_command_encoder();
                    encoder.set_compute_pipeline_state(&compute_pipeline_state);
                    encoder.set_texture(0, Some(drawable.texture()));
                    let thread_groups_per_grid = MTLSize {
                        width: drawable.texture().width() / 16,
                        height: drawable.texture().height() / 16,
                        depth: 1,
                    };
                    let threads_per_thread_group = MTLSize {
                        width: 16,
                        height: 16,
                        depth: 1,
                    };
                    encoder
                        .dispatch_thread_groups(thread_groups_per_grid, threads_per_thread_group);
                    encoder.end_encoding();
                    command_buffer.present_drawable(&drawable);
                    command_buffer.commit();
                }
            }
            _ => (),
        }
    });
}
