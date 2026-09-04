//! Measurable power policy layered over the preserved Rustmix power/network drivers.

pub const DEFAULT_IDLE_SLEEP_SECONDS: u64 = 180;
pub const DEFAULT_WIFI_IDLE_SECONDS: u64 = 15;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PowerPhase {
    Boot,
    WifiConnect,
    Sync,
    Reading,
    Idle,
    Sleep,
    DeepSleep,
    Wake,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkInhibitors {
    pub recording: bool,
    pub pending_upload: bool,
    pub pairing: bool,
    pub ota: bool,
}

impl WorkInhibitors {
    #[must_use]
    pub const fn any(self) -> bool {
        self.recording || self.pending_upload || self.pairing || self.ota
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdleDecision {
    StayAwake,
    SuspendWifi,
    EnterDisplaySleep,
    EnterDeepSleep,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductPowerPolicy {
    pub wifi_idle_seconds: u64,
    pub display_sleep_seconds: u64,
    /// Must remain false until GPIO45 RTC and Power-key wake are physically verified.
    pub deep_sleep_wake_verified: bool,
}

impl Default for ProductPowerPolicy {
    fn default() -> Self {
        Self {
            wifi_idle_seconds: DEFAULT_WIFI_IDLE_SECONDS,
            display_sleep_seconds: DEFAULT_IDLE_SLEEP_SECONDS,
            deep_sleep_wake_verified: false,
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
        if idle_seconds >= self.display_sleep_seconds {
            return if self.deep_sleep_wake_verified {
                IdleDecision::EnterDeepSleep
            } else {
                IdleDecision::EnterDisplaySleep
            };
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
