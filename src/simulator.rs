#[cfg(test)]
mod tests {
    use super::{
        AtlasConnectionState, BatteryState, SdState, SemanticInput, SimulatedHardware, Simulator,
        SimulatorKey, WifiState, LOGICAL_HEIGHT, LOGICAL_WIDTH, NATIVE_FRAMEBUFFER_SIZE,
    };
    use crate::app::{router::AtlasRoute, ScreenRoute};

    #[test]
    fn semantic_input_translation_is_independent_of_physical_keys() {
        assert_eq!(
            SimulatorKey::ArrowUp.semantic_input(),
            Some(SemanticInput::Up)
        );
        assert_eq!(
            SimulatorKey::ArrowDown.semantic_input(),
            Some(SemanticInput::Down)
        );
        assert_eq!(
            SimulatorKey::Enter.semantic_input(),
            Some(SemanticInput::Select)
        );
        assert_eq!(
            SimulatorKey::Escape.semantic_input(),
            Some(SemanticInput::Back)
        );
        assert_eq!(SimulatorKey::H.semantic_input(), Some(SemanticInput::Home));
        assert_eq!(
            SimulatorKey::Home.semantic_input(),
            Some(SemanticInput::Home)
        );
        assert_eq!(SimulatorKey::P.semantic_input(), Some(SemanticInput::Power));
        assert_eq!(SimulatorKey::Other.semantic_input(), None);
    }

    #[test]
    fn simulator_reuses_portrait_product_renderer_and_native_framebuffer() {
        let mut simulator = Simulator::default();
        let first = simulator.render().unwrap().to_vec();
        let second = simulator.render().unwrap().to_vec();
        assert_eq!(first, second);
        assert_eq!(first.len(), NATIVE_FRAMEBUFFER_SIZE);
        assert_eq!(simulator.logical_size(), (LOGICAL_WIDTH, LOGICAL_HEIGHT));
    }

    #[test]
    fn semantic_navigation_reaches_shell_routes_and_back() {
        let mut simulator = Simulator::default();
        for _ in 0..5 {
            simulator.handle_key(SimulatorKey::ArrowDown).unwrap();
            simulator.handle_key(SimulatorKey::Enter).unwrap();
            assert_eq!(simulator.state().active_route(), ScreenRoute::Home);
            assert_ne!(simulator.state().atlas_route(), AtlasRoute::Home);
            simulator.handle_key(SimulatorKey::Escape).unwrap();
            assert_eq!(simulator.state().atlas_route(), AtlasRoute::Home);
        }
    }

    #[test]
    fn selected_rows_change_the_real_rendered_frame() {
        let mut simulator = Simulator::default();
        let home = simulator.render().unwrap().to_vec();
        simulator.handle_key(SimulatorKey::ArrowDown).unwrap();
        let next = simulator.render().unwrap().to_vec();
        assert_ne!(home, next);
    }

    #[test]
    fn hardware_model_has_bounded_deterministic_states_without_secrets() {
        let hardware = SimulatedHardware::default();
        assert_eq!(hardware.wifi.label(), "connected");
        assert_eq!(hardware.battery.label(), "100%");
        assert_eq!(hardware.sd.label(), "mounted");
        assert_eq!(hardware.atlas.label(), "unconfigured");
        assert_eq!(
            hardware.diagnostic_labels(),
            [
                "display=ready",
                "input=ready",
                "sd=mounted",
                "wifi=connected",
                "battery=100%",
                "rtc=ready",
                "atlas=unconfigured",
            ]
        );
        assert!(!hardware.redacted_summary().contains("password"));
        let _ = [
            WifiState::Connected,
            WifiState::Connecting,
            WifiState::Offline,
            WifiState::Failed,
        ];
        let _ = [
            BatteryState::Percent100,
            BatteryState::Percent50,
            BatteryState::Percent10,
        ];
        let _ = [SdState::Mounted, SdState::Missing, SdState::Error];
        let _ = [
            AtlasConnectionState::Unconfigured,
            AtlasConnectionState::Connecting,
            AtlasConnectionState::Connected,
            AtlasConnectionState::Unauthorized,
            AtlasConnectionState::Forbidden,
            AtlasConnectionState::Timeout,
            AtlasConnectionState::ServerError,
            AtlasConnectionState::Offline,
        ];
    }

    #[test]
    fn hardware_diagnostics_follow_selected_fake_states() {
        let hardware = SimulatedHardware {
            sd: SdState::Error,
            wifi: WifiState::Offline,
            battery: BatteryState::Percent10,
            rtc: super::RtcState::IntegrityLost,
            atlas: AtlasConnectionState::ServerError,
            ..SimulatedHardware::default()
        };
        assert_eq!(hardware.diagnostic_labels()[2], "sd=error");
        assert_eq!(hardware.diagnostic_labels()[3], "wifi=offline");
        assert_eq!(hardware.diagnostic_labels()[4], "battery=10%");
        assert_eq!(hardware.diagnostic_labels()[5], "rtc=integrity_lost");
        assert_eq!(hardware.diagnostic_labels()[6], "atlas=server_error");
    }
}
/// Host-only simulator core for Atlas Lite application and hardware seams.
use core::convert::Infallible;

use crate::{
    app::{render_current_screen, AppState},
    buttons::ButtonEvent,
    framebuffer::{FrameBuffer, FRAMEBUFFER_SIZE},
};

pub const LOGICAL_WIDTH: u32 = 480;
pub const LOGICAL_HEIGHT: u32 = 800;
pub const NATIVE_FRAMEBUFFER_SIZE: usize = FRAMEBUFFER_SIZE;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulatorKey {
    ArrowUp,
    ArrowDown,
    Enter,
    Escape,
    H,
    Home,
    P,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticInput {
    Up,
    Down,
    Select,
    Back,
    Home,
    Power,
}

impl SimulatorKey {
    #[must_use]
    pub const fn semantic_input(self) -> Option<SemanticInput> {
        match self {
            Self::ArrowUp => Some(SemanticInput::Up),
            Self::ArrowDown => Some(SemanticInput::Down),
            Self::Enter => Some(SemanticInput::Select),
            Self::Escape => Some(SemanticInput::Back),
            Self::H | Self::Home => Some(SemanticInput::Home),
            Self::P => Some(SemanticInput::Power),
            Self::Other => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WifiState {
    Connected,
    Connecting,
    Offline,
    Failed,
}

impl WifiState {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Connecting => "connecting",
            Self::Offline => "offline",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BatteryState {
    Percent100,
    Percent50,
    Percent10,
}

impl BatteryState {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Percent100 => "100%",
            Self::Percent50 => "50%",
            Self::Percent10 => "10%",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SdState {
    Mounted,
    Missing,
    Error,
}

impl SdState {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Mounted => "mounted",
            Self::Missing => "missing",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtlasConnectionState {
    Unconfigured,
    Connecting,
    Connected,
    Unauthorized,
    Forbidden,
    Timeout,
    ServerError,
    Offline,
}

impl AtlasConnectionState {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Unconfigured => "unconfigured",
            Self::Connecting => "connecting",
            Self::Connected => "connected",
            Self::Unauthorized => "unauthorized",
            Self::Forbidden => "forbidden",
            Self::Timeout => "timeout",
            Self::ServerError => "server_error",
            Self::Offline => "offline",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RtcState {
    Ready,
    Unavailable,
    IntegrityLost,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulatedDisplay {
    pub logical_width: u32,
    pub logical_height: u32,
    pub native_width: u32,
    pub native_height: u32,
}

impl Default for SimulatedDisplay {
    fn default() -> Self {
        Self {
            logical_width: LOGICAL_WIDTH,
            logical_height: LOGICAL_HEIGHT,
            native_width: 800,
            native_height: 480,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SimulatedInput {
    pub last: Option<SemanticInput>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulatedHardware {
    pub display: SimulatedDisplay,
    pub input: SimulatedInput,
    pub sd: SdState,
    pub wifi: WifiState,
    pub battery: BatteryState,
    pub rtc: RtcState,
    pub atlas: AtlasConnectionState,
}

impl Default for SimulatedHardware {
    fn default() -> Self {
        Self {
            display: SimulatedDisplay::default(),
            input: SimulatedInput::default(),
            sd: SdState::Mounted,
            wifi: WifiState::Connected,
            battery: BatteryState::Percent100,
            rtc: RtcState::Ready,
            atlas: AtlasConnectionState::Unconfigured,
        }
    }
}

impl SimulatedHardware {
    #[must_use]
    pub fn diagnostic_labels(&self) -> [String; 7] {
        [
            "display=ready".into(),
            "input=ready".into(),
            format!("sd={}", self.sd.label()),
            format!("wifi={}", self.wifi.label()),
            format!("battery={}", self.battery.label()),
            format!(
                "rtc={}",
                match self.rtc {
                    RtcState::Ready => "ready",
                    RtcState::Unavailable => "unavailable",
                    RtcState::IntegrityLost => "integrity_lost",
                }
            ),
            format!("atlas={}", self.atlas.label()),
        ]
    }

    #[must_use]
    pub fn redacted_summary(&self) -> String {
        self.diagnostic_labels().join(" ")
    }
}

#[derive(Debug)]
pub struct Simulator {
    state: AppState,
    hardware: SimulatedHardware,
    frame: FrameBuffer,
}

impl Default for Simulator {
    fn default() -> Self {
        Self {
            state: AppState::default(),
            hardware: SimulatedHardware::default(),
            frame: FrameBuffer::new_white(),
        }
    }
}

impl Simulator {
    #[must_use]
    pub const fn state(&self) -> &AppState {
        &self.state
    }

    #[must_use]
    pub const fn hardware(&self) -> &SimulatedHardware {
        &self.hardware
    }

    pub fn set_hardware(&mut self, hardware: SimulatedHardware) {
        self.hardware = hardware;
    }

    #[must_use]
    pub const fn logical_size(&self) -> (u32, u32) {
        (LOGICAL_WIDTH, LOGICAL_HEIGHT)
    }

    pub fn render(&mut self) -> Result<&[u8], Infallible> {
        render_current_screen(&mut self.frame, &self.state)?;
        Ok(self.frame.as_bytes())
    }

    pub fn handle_key(&mut self, key: SimulatorKey) -> Result<(), Infallible> {
        let Some(input) = key.semantic_input() else {
            return Ok(());
        };
        self.hardware.input.last = Some(input);
        match input {
            SemanticInput::Up => self.state.apply(ButtonEvent::Up),
            SemanticInput::Down => self.state.apply(ButtonEvent::Down),
            SemanticInput::Select => self.state.apply(ButtonEvent::Select),
            SemanticInput::Back => self.state.back(),
            SemanticInput::Home => {
                while self.state.active_route() != crate::app::ScreenRoute::Home
                    || self.state.atlas_route() != crate::app::router::AtlasRoute::Home
                {
                    self.state.back();
                }
            }
            SemanticInput::Power => self.state.open_power_key_menu(),
        }
        Ok(())
    }
}
