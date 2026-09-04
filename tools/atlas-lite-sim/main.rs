//! Headless host entry point for the Atlas Lite simulator.

use std::io::{self, Read};

use waveshare_epd397_rust_app::simulator::{Simulator, SimulatorKey};

fn main() {
    let mut simulator = Simulator::default();
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .expect("read simulator input");

    for key in input.lines().filter_map(parse_key) {
        simulator
            .handle_key(key)
            .expect("semantic input cannot fail");
    }
    let route = simulator.state().active_route().label();
    let atlas_route = simulator.state().atlas_route().label();
    let frame = simulator.render().expect("renderer cannot fail");
    let checksum: u64 = frame.iter().map(|byte| u64::from(*byte)).sum();
    println!(
        "atlas-lite-sim route={} atlas={} frame-bytes={} checksum={checksum}",
        route,
        atlas_route,
        frame.len(),
    );
}

fn parse_key(line: &str) -> Option<SimulatorKey> {
    Some(match line.trim() {
        "up" => SimulatorKey::ArrowUp,
        "down" => SimulatorKey::ArrowDown,
        "enter" => SimulatorKey::Enter,
        "esc" | "escape" => SimulatorKey::Escape,
        "h" => SimulatorKey::H,
        "home" => SimulatorKey::Home,
        "p" => SimulatorKey::P,
        _ => return None,
    })
}
