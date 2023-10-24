use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder
};
use wgpu::util::DeviceExt;

async fn run() {

    let event_loop = EventLoop::new().unwrap();

    let window = WindowBuilder::new()
        .with_title("Rust Window")
        .build(&event_loop)
        .unwrap();

    event_loop.run(move|event, _, control_flow| {
        match event {
            WindowEvent::CloseRequested | WindowEvent::KeyboardInput {

            },
            _ => {}
        }
    }).unwrap();

}
