//! Measurable power policy layered over the preserved Rustmix power/network drivers.

pub const DEFAULT_IDLE_SLEEP_SECONDS: u64 = 60;
pub const DEFAULT_WIFI_IDLE_SECONDS: u64 = 15;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PowerPhase {
    Boot,
    WifiConnect,
    Sync,
    Reading,
    Idle,
    LightSleep,
    Wake,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkInhibitors {
    pub recording: bool,
    pub playback: bool,
    pub wav_finalizing: bool,
    pub nvs_write: bool,
    pub sd_write: bool,
    pub http_in_flight: bool,
    pub pairing: bool,
    pub ota: bool,
    pub panel_refresh: bool,
    pub pending_input: bool,
    /// USB/VBUS keeps the developer console alive unless an explicit hardware
    /// test mode is added. A durable pending upload is deliberately not an
    /// inhibitor: it survives reboot and can retry after wake.
    pub usb_development: bool,
}

impl WorkInhibitors {
    #[must_use]
    pub const fn any(self) -> bool {
        self.recording
            || self.playback
            || self.wav_finalizing
            || self.nvs_write
            || self.sd_write
            || self.http_in_flight
            || self.pairing
            || self.ota
            || self.panel_refresh
            || self.pending_input
            || self.usb_development
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdleDecision {
    StayAwake,
    SuspendWifi,
    EnterLightSleep,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductPowerPolicy {
    pub wifi_idle_seconds: u64,
    pub light_sleep_seconds: u64,
}

impl Default for ProductPowerPolicy {
    fn default() -> Self {
        Self {
            wifi_idle_seconds: DEFAULT_WIFI_IDLE_SECONDS,
            light_sleep_seconds: DEFAULT_IDLE_SLEEP_SECONDS,
        }
    }
}

impl ProductPowerPolicy {
    #[must_use]
    pub const fn decide(
        self,
        idle_seconds: u64,
        wifi_active: bool,
        inhibitors: WorkInhibitors,
    ) -> IdleDecision {
        if inhibitors.any() {
            return IdleDecision::StayAwake;
        }
        if idle_seconds >= self.light_sleep_seconds {
            return IdleDecision::EnterLightSleep;
        }
        if wifi_active && idle_seconds >= self.wifi_idle_seconds {
            return IdleDecision::SuspendWifi;
        }
        IdleDecision::StayAwake
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PowerProfile {
    samples: Vec<(PowerPhase, u32)>,
}

impl PowerProfile {
    pub fn record_milliamps(&mut self, phase: PowerPhase, milliamps: u32) {
        if self.samples.len() < 32 {
            self.samples.push((phase, milliamps));
        }
    }
    #[must_use]
    pub fn samples(&self) -> &[(PowerPhase, u32)] {
        &self.samples
    }
}
