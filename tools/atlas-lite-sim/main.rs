//! Headless host entry point for the Atlas Lite simulator.

use std::io::{self, Read};

use waveshare_epd397_rust_app::simulator::{AtlasConnectionState, Simulator, SimulatorKey};

fn main() {
    let mut simulator = Simulator::default();
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .expect("read simulator input");

    for line in input.lines() {
        if let Some(state) = parse_atlas_state(line) {
            simulator.set_atlas_connection_state(state);
        } else if let Some(key) = parse_key(line) {
            simulator
                .handle_key(key)
                .expect("semantic input cannot fail");
        }
    }
    let route = simulator.state().active_route().label();
    let atlas_route = simulator.state().atlas_route().label();
    let atlas_status = simulator.state().atlas.connection.label();
    let frame = simulator.render().expect("renderer cannot fail");
    let checksum: u64 = frame.iter().map(|byte| u64::from(*byte)).sum();
    println!(
        "atlas-lite-sim route={} atlas={} atlas-status={} frame-bytes={} checksum={checksum}",
        route,
        atlas_route,
        atlas_status,
        frame.len(),
    );
}

fn parse_key(line: &str) -> Option<SimulatorKey> {
    Some(match line.trim() {
        "up" => SimulatorKey::ArrowUp,
        "down" => SimulatorKey::ArrowDown,
        "enter" => SimulatorKey::Enter,
        "b" | "boot" => SimulatorKey::B,
        "esc" | "escape" => SimulatorKey::Escape,
        "h" => SimulatorKey::H,
        "home" => SimulatorKey::Home,
        "p" => SimulatorKey::P,
        _ => return None,
    })
}

fn parse_atlas_state(line: &str) -> Option<AtlasConnectionState> {
    Some(match line.trim() {
        "atlas=unconfigured" => AtlasConnectionState::Unconfigured,
        "atlas=connecting" => AtlasConnectionState::Connecting,
        "atlas=connected" => AtlasConnectionState::Connected,
        "atlas=unauthorized" => AtlasConnectionState::Unauthorized,
        "atlas=forbidden" => AtlasConnectionState::Forbidden,
        "atlas=timeout" => AtlasConnectionState::Timeout,
        "atlas=server_error" => AtlasConnectionState::ServerError,
        "atlas=offline" => AtlasConnectionState::Offline,
        _ => return None,
    })
}
