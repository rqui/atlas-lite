use waveshare_epd397_rust_app::product_power::{IdleDecision, ProductPowerPolicy, WorkInhibitors};

#[test]
fn radio_suspends_after_15_seconds_and_light_sleep_starts_at_60() {
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
        policy.decide(60, false, WorkInhibitors::default()),
        IdleDecision::EnterLightSleep
    );
}

#[test]
fn active_or_unsafe_work_inhibits_sleep_but_durable_pending_upload_does_not() {
    let policy = ProductPowerPolicy::default();
    for inhibitors in [
        WorkInhibitors {
            recording: true,
            ..Default::default()
        },
        WorkInhibitors {
            playback: true,
            ..Default::default()
        },
        WorkInhibitors {
            wav_finalizing: true,
            ..Default::default()
        },
        WorkInhibitors {
            nvs_write: true,
            ..Default::default()
        },
        WorkInhibitors {
            sd_write: true,
            ..Default::default()
        },
        WorkInhibitors {
            http_in_flight: true,
            ..Default::default()
        },
        WorkInhibitors {
            pairing: true,
            ..Default::default()
        },
        WorkInhibitors {
            ota: true,
            ..Default::default()
        },
        WorkInhibitors {
            panel_refresh: true,
            ..Default::default()
        },
        WorkInhibitors {
            pending_input: true,
            ..Default::default()
        },
        WorkInhibitors {
            usb_development: true,
            ..Default::default()
        },
    ] {
        assert_eq!(
            policy.decide(600, true, inhibitors),
            IdleDecision::StayAwake
        );
    }
    assert_eq!(
        policy.decide(600, false, WorkInhibitors::default()),
        IdleDecision::EnterLightSleep
    );
}
