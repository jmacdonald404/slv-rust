use slv_rust::rendering::engine::State;
use winit::event_loop::EventLoop;

fn main() {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut state: State = State {
        renderer: None,
        last_light_position: cgmath::Point3::new(0.0, 0.0, 0.0),
        window: None,
    };
    event_loop.run_app(&mut state).expect("Failed to run app");
}