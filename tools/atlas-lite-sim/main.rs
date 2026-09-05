//! Headless host entry point for the Atlas Lite simulator.

use std::{
    env,
    fs,
    io::{self, Read},
    path::Path,
};

use waveshare_epd397_rust_app::simulator::{
    AtlasConnectionState, Simulator, SimulatorHomeFixture, SimulatorKey, SimulatorLibraryFixture,
    SimulatorNoteFixture, SimulatorSearchFixture, SimulatorViewsFixture,
};

fn main() {
    let framebuffer_pgm = parse_framebuffer_pgm_argument();
    let mut simulator = Simulator::default();
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .expect("read simulator input");

    for line in input.lines() {
        if let Some(state) = parse_atlas_state(line) {
            simulator.set_atlas_connection_state(state);
        } else if apply_fixture(&mut simulator, line) {
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
    if let Some(path) = framebuffer_pgm {
        write_logical_pgm(&path, frame).expect("write framebuffer PGM");
    }
    let checksum: u64 = frame.iter().map(|byte| u64::from(*byte)).sum();
    println!(
        "atlas-lite-sim route={} atlas={} atlas-status={} frame-bytes={} checksum={checksum}",
        route,
        atlas_route,
        atlas_status,
        frame.len(),
    );
}

fn parse_framebuffer_pgm_argument() -> Option<std::path::PathBuf> {
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--framebuffer-pgm" {
            return arguments.next().map(Into::into);
        }
        if let Some(path) = argument.strip_prefix("--framebuffer-pgm=") {
            return Some(path.into());
        }
    }
    None
}

fn apply_fixture(simulator: &mut Simulator, line: &str) -> bool {
    match line.trim() {
        "fixture=home" => simulator.apply_home_fixture(SimulatorHomeFixture::Normal),
        "fixture=library" => simulator.apply_library_fixture(SimulatorLibraryFixture::Normal),
        "fixture=search" => simulator.apply_search_fixture(SimulatorSearchFixture::Success),
        "fixture=views" => {
            for _ in 0..2 {
                simulator
                    .handle_key(SimulatorKey::ArrowDown)
                    .expect("views navigation cannot fail");
            }
            simulator
                .handle_key(SimulatorKey::Enter)
                .expect("views navigation cannot fail");
            simulator.apply_views_fixture(SimulatorViewsFixture::Success);
        }
        "fixture=note" => simulator.apply_note_fixture(SimulatorNoteFixture::Loaded),
        "fixture=settings" => {
            for _ in 0..4 {
                simulator
                    .handle_key(SimulatorKey::ArrowDown)
                    .expect("settings navigation cannot fail");
            }
            simulator
                .handle_key(SimulatorKey::Enter)
                .expect("settings navigation cannot fail");
        }
        _ => return false,
    }
    true
}

/// Save the actual portrait-oriented 1-bpp framebuffer as a portable greyscale
/// image for review. This is host-only evidence; firmware never writes it.
fn write_logical_pgm(path: &Path, native_frame: &[u8]) -> io::Result<()> {
    const LOGICAL_WIDTH: usize = 480;
    const LOGICAL_HEIGHT: usize = 800;
    const NATIVE_WIDTH: usize = 800;
    const NATIVE_HEIGHT: usize = 480;
    const NATIVE_ROW_BYTES: usize = NATIVE_WIDTH / 8;

    if native_frame.len() != NATIVE_ROW_BYTES * NATIVE_HEIGHT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unexpected native framebuffer size",
        ));
    }
    let mut output = Vec::with_capacity(16 + LOGICAL_WIDTH * LOGICAL_HEIGHT);
    output.extend_from_slice(b"P5\n480 800\n255\n");
    for logical_y in 0..LOGICAL_HEIGHT {
        for logical_x in 0..LOGICAL_WIDTH {
            let native_x = logical_y;
            let native_y = NATIVE_HEIGHT - 1 - logical_x;
            let byte = native_frame[native_y * NATIVE_ROW_BYTES + native_x / 8];
            let black = byte & (0x80 >> (native_x % 8)) == 0;
            output.push(if black { 0 } else { 0xff });
        }
    }
    fs::write(path, output)
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
