//! Native panel refresh coordinator.
//!
//! All e-paper mode decisions remain Rust-owned. SD-loaded applications may
//! submit dirty rectangles and draw intent, but they never select SSD1677
//! commands directly. The coordinator deliberately retains the proven
//! full-screen partial transport until windowed RAM writes receive their own
//! isolated hardware experiment.

/// Periodic ghost-cleanup cadence shared by menus, Reader screens and games.
pub const PANEL_PARTIAL_REFRESH_LIMIT: u8 = 24;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PanelRefreshRequest {
    Normal,
    AfterWake,
    ManualGhostCleanup,
    SafetyFallback,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PanelGlobalReason {
    InitialBoot,
    AfterWake,
    ManualGhostCleanup,
    PeriodicCleanup,
    SafetyFallback,
    SleepImage,
}

impl PanelGlobalReason {
    #[must_use]
    pub const fn marker(self) -> &'static str {
        match self {
            Self::InitialBoot => "initial-boot",
            Self::AfterWake => "after-wake",
            Self::ManualGhostCleanup => "manual-ghost-cleanup",
            Self::PeriodicCleanup => "ghost-cleanup-threshold",
            Self::SafetyFallback => "safety-fallback",
            Self::SleepImage => "sleep-image",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PanelRefreshPlan {
    PartialFullscreen { partial_count: u8 },
    GlobalBase { reason: PanelGlobalReason },
}

/// One counter for every normal UI and game refresh. This replaces the former
/// split between the six-refresh UI counter and the independent game policy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PanelRefreshCoordinator {
    partial_count: u8,
    /// Controller RAM is invalid after deep sleep/rail-off until a real base
    /// frame has completed. Initializing the controller alone is not a base.
    base_required: bool,
}

impl PanelRefreshCoordinator {
    #[must_use]
    pub const fn partial_count(self) -> u8 {
        self.partial_count
    }

    #[must_use]
    pub const fn base_required(self) -> bool {
        self.base_required
    }

    /// Call immediately after controller state may have been lost. This is
    /// deliberately not a counter reset: no display operation occurred yet.
    pub fn require_base_after_controller_loss(&mut self) {
        self.base_required = true;
    }

    /// Record only an actually completed external `show_base` operation.
    pub fn complete_external_global(&mut self, _reason: PanelGlobalReason) {
        self.partial_count = 0;
        self.base_required = false;
    }

    #[must_use]
    pub fn plan(&self, request: PanelRefreshRequest) -> PanelRefreshPlan {
        let forced = match request {
            PanelRefreshRequest::Normal => None,
            PanelRefreshRequest::AfterWake => Some(PanelGlobalReason::AfterWake),
            PanelRefreshRequest::ManualGhostCleanup => Some(PanelGlobalReason::ManualGhostCleanup),
            PanelRefreshRequest::SafetyFallback => Some(PanelGlobalReason::SafetyFallback),
        };
        if let Some(reason) = forced {
            return PanelRefreshPlan::GlobalBase { reason };
        }
        if self.base_required || self.partial_count >= PANEL_PARTIAL_REFRESH_LIMIT {
            return PanelRefreshPlan::GlobalBase {
                reason: if self.base_required {
                    PanelGlobalReason::AfterWake
                } else {
                    PanelGlobalReason::PeriodicCleanup
                },
            };
        }
        PanelRefreshPlan::PartialFullscreen {
            partial_count: self.partial_count.saturating_add(1),
        }
    }

    /// Commit only after the panel operation has returned success. A failed
    /// base leaves `base_required` set so a partial can never follow it.
    pub fn complete(&mut self, plan: PanelRefreshPlan) {
        match plan {
            PanelRefreshPlan::GlobalBase { .. } => {
                self.partial_count = 0;
                self.base_required = false;
            }
            PanelRefreshPlan::PartialFullscreen { partial_count } => {
                self.partial_count = partial_count;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PanelGlobalReason, PanelRefreshCoordinator, PanelRefreshPlan, PanelRefreshRequest,
        PANEL_PARTIAL_REFRESH_LIMIT,
    };

    #[test]
    fn normal_routes_share_one_partial_counter_before_periodic_cleanup() {
        let mut coordinator = PanelRefreshCoordinator::default();
        for partial_count in 1..=PANEL_PARTIAL_REFRESH_LIMIT {
            assert_eq!(
                coordinator.plan(PanelRefreshRequest::Normal),
                PanelRefreshPlan::PartialFullscreen { partial_count }
            );
            coordinator.complete(PanelRefreshPlan::PartialFullscreen { partial_count });
        }
        assert_eq!(
            coordinator.plan(PanelRefreshRequest::Normal),
            PanelRefreshPlan::GlobalBase {
                reason: PanelGlobalReason::PeriodicCleanup
            }
        );
    }

    #[test]
    fn wake_manual_and_safety_requests_force_global_base() {
        let mut coordinator = PanelRefreshCoordinator::default();
        let plan = coordinator.plan(PanelRefreshRequest::Normal);
        coordinator.complete(plan);
        assert_eq!(
            coordinator.plan(PanelRefreshRequest::AfterWake),
            PanelRefreshPlan::GlobalBase {
                reason: PanelGlobalReason::AfterWake
            }
        );
        assert_eq!(coordinator.partial_count(), 1);
        assert_eq!(
            coordinator.plan(PanelRefreshRequest::ManualGhostCleanup),
            PanelRefreshPlan::GlobalBase {
                reason: PanelGlobalReason::ManualGhostCleanup
            }
        );
        assert_eq!(
            coordinator.plan(PanelRefreshRequest::SafetyFallback),
            PanelRefreshPlan::GlobalBase {
                reason: PanelGlobalReason::SafetyFallback
            }
        );
    }

    #[test]
    fn controller_loss_requires_a_successful_base_before_any_partial() {
        let mut coordinator = PanelRefreshCoordinator::default();
        coordinator.require_base_after_controller_loss();
        let base = coordinator.plan(PanelRefreshRequest::Normal);
        assert_eq!(
            base,
            PanelRefreshPlan::GlobalBase {
                reason: PanelGlobalReason::AfterWake
            }
        );
        assert!(coordinator.base_required());
        // A failed physical transfer is deliberately not committed.
        assert_eq!(coordinator.plan(PanelRefreshRequest::Normal), base);
        coordinator.complete(base);
        assert!(!coordinator.base_required());
        assert_eq!(
            coordinator.plan(PanelRefreshRequest::Normal),
            PanelRefreshPlan::PartialFullscreen { partial_count: 1 }
        );
    }
}
