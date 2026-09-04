use waveshare_epd397_rust_app::product_power::{IdleDecision, ProductPowerPolicy, WorkInhibitors};

#[test]
fn radio_suspends_after_bounded_idle_and_display_sleeps_later() {
    let policy = ProductPowerPolicy::default();
    assert_eq!(
        policy.decide(14, true, WorkInhibitors::default()),
        IdleDecision::StayAwake
    );
    assert_eq!(
        policy.decide(15, true, WorkInhibitors::default()),
        IdleDecision::SuspendWifi
    );
    assert_eq!(
        policy.decide(180, false, WorkInhibitors::default()),
        IdleDecision::EnterDisplaySleep
    );
}

#[test]
fn durable_work_inhibits_sleep_and_unverified_board_never_enters_deep_sleep() {
    let policy = ProductPowerPolicy::default();
    assert_eq!(
        policy.decide(
            1_000,
            true,
            WorkInhibitors {
                pending_upload: true,
                ..Default::default()
            }
        ),
        IdleDecision::StayAwake
    );
    assert!(!policy.deep_sleep_wake_verified);
    assert_ne!(
        policy.decide(1_000, false, WorkInhibitors::default()),
        IdleDecision::EnterDeepSleep
    );
}
