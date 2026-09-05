//! Execute the SAME adapter used by ESP-IDF against one-shot GPIOs. The fake
//! disables an interrupt before notifying, just like PinDriver::handle_isr.
//! UI consumption is independent of the capture task's deadlines/rearming.
use std::collections::VecDeque;

use waveshare_epd397_rust_app::buttons::{
    capture::{CaptureAdapter, CaptureIo, Key, RawEdge, INPUT_QUEUE_CAPACITY, KEYS},
    BootButtonEvent, BootPressTracker, ButtonEvent, CapturedInput, CapturedInputEvent,
    InputEventQueue,
};

struct OneShotGpio {
    levels: [bool; 4],
    enabled: [bool; 4],
    rearms: [usize; 4],
    raw: VecDeque<RawEdge>,
    ui: InputEventQueue<INPUT_QUEUE_CAPACITY>,
    in_isr: bool,
    fail_rearm: bool,
}

impl Default for OneShotGpio {
    fn default() -> Self {
        Self {
            levels: [false; 4],
            enabled: [false; 4],
            rearms: [0; 4],
            raw: VecDeque::new(),
            ui: InputEventQueue::new(),
            in_isr: false,
            fail_rearm: false,
        }
    }
}

impl OneShotGpio {
    fn physical_edge(&mut self, key: Key, pressed: bool, timestamp_ms: u64) {
        let i = key as usize;
        if self.levels[i] == pressed {
            return;
        }
        self.levels[i] = pressed;
        if self.enabled[i] {
            self.in_isr = true;
            self.enabled[i] = false; // HAL disables BEFORE invoking callback.
            self.raw.push_back(RawEdge {
                key,
                pressed,
                timestamp_ms,
            });
            self.in_isr = false;
        }
    }
}

impl CaptureIo for OneShotGpio {
    type Error = &'static str;
    fn pressed(&self, key: Key) -> bool {
        self.levels[key as usize]
    }
    fn rearm(&mut self, key: Key) -> Result<(), Self::Error> {
        assert!(!self.in_isr, "HAL forbids enabling GPIO inside ISR");
        if self.fail_rearm {
            return Err("rearm failed");
        }
        self.rearms[key as usize] += 1;
        self.enabled[key as usize] = true;
        Ok(())
    }
    fn emit(&mut self, input: CapturedInput, timestamp_ms: u64) {
        assert!(!self.in_isr);
        self.ui.push(input, timestamp_ms);
    }
}

struct Service {
    adapter: CaptureAdapter,
    gpio: OneShotGpio,
    now: u64,
    deadline_wakes: usize,
}

impl Service {
    fn new() -> Self {
        let mut result = Self {
            adapter: CaptureAdapter::default(),
            gpio: OneShotGpio::default(),
            now: 0,
            deadline_wakes: 0,
        };
        result.adapter.start(&mut result.gpio, 0).unwrap();
        result
    }

    fn worker(&mut self) {
        while let Some(edge) = self.gpio.raw.pop_front() {
            self.adapter.edge(&mut self.gpio, edge).unwrap();
        }
        self.adapter.settle(&mut self.gpio, self.now);
    }

    fn advance(&mut self, until: u64) {
        assert!(until >= self.now);
        while let Some(wait) = self.adapter.wait_ms(self.now) {
            if self.now + wait > until {
                break;
            }
            assert!(wait > 0, "worker must block, not spin");
            self.now += wait;
            self.deadline_wakes += 1;
            self.worker();
        }
        self.now = until;
    }

    fn edge(&mut self, at: u64, key: Key, pressed: bool) {
        self.advance(at);
        self.gpio.physical_edge(key, pressed, at);
        self.worker(); // capture task, NOT the blocked UI
    }

    fn press(&mut self, at: u64, key: Key) {
        self.edge(at, key, true);
        self.edge(at + 60, key, false);
    }

    fn consume_ui(&mut self) -> Vec<CapturedInputEvent> {
        std::iter::from_fn(|| self.gpio.ui.pop()).collect()
    }
}

fn nav(button: ButtonEvent) -> CapturedInput {
    CapturedInput::Navigation(button)
}

#[test]
fn same_gpio_three_presses_during_700ms_ui_block_rearms_each_edge() {
    let mut s = Service::new();
    for at in [50, 230, 440] {
        s.press(at, Key::Down);
    }
    s.advance(700);
    let events = s.consume_ui(); // first UI execution since t=0
    assert_eq!(
        events.iter().map(|e| e.input).collect::<Vec<_>>(),
        vec![nav(ButtonEvent::Down); 3]
    );
    assert_eq!(
        events.iter().map(|e| e.timestamp_ms).collect::<Vec<_>>(),
        [50, 230, 440]
    );
    assert_eq!(s.gpio.rearms[Key::Down as usize], 7);
    assert_eq!(s.gpio.ui.dropped(), 0);
}

#[test]
fn same_gpio_twelve_presses_during_2500ms_global_refresh() {
    let mut s = Service::new();
    for i in 0..12 {
        s.press(50 + i * 200, Key::Up);
    }
    s.advance(2500);
    let events = s.consume_ui();
    assert_eq!(events.len(), 12);
    assert!(events.iter().all(|e| e.input == nav(ButtonEvent::Up)));
    assert_eq!(
        events.iter().map(|e| e.sequence).collect::<Vec<_>>(),
        (0..12).collect::<Vec<_>>()
    );
    assert_eq!(s.gpio.rearms[Key::Up as usize], 25);
    assert_eq!(s.gpio.ui.dropped(), 0);
}

#[test]
fn down_down_select_keeps_exact_fifo_while_ui_is_blocked() {
    let mut s = Service::new();
    s.press(50, Key::Down);
    s.press(230, Key::Down);
    s.press(440, Key::Select);
    s.advance(700);
    assert_eq!(
        s.consume_ui().iter().map(|e| e.input).collect::<Vec<_>>(),
        [
            nav(ButtonEvent::Down),
            nav(ButtonEvent::Down),
            nav(ButtonEvent::Select),
        ]
    );
}

#[test]
fn boot_both_edges_survive_http_wait_and_classify_without_blocking() {
    let mut s = Service::new();
    s.edge(100, Key::Boot, true);
    s.press(200, Key::Up);
    s.edge(1300, Key::Boot, false);
    s.advance(2500);
    let events = s.consume_ui();
    assert_eq!(
        events.iter().map(|e| e.input).collect::<Vec<_>>(),
        [
            CapturedInput::BootPressed,
            nav(ButtonEvent::Up),
            CapturedInput::BootReleased,
        ]
    );
    let mut tracker = BootPressTracker::default();
    assert_eq!(
        events
            .into_iter()
            .filter_map(|e| tracker.consume(e))
            .collect::<Vec<_>>(),
        [BootButtonEvent::LongPress]
    );
    assert_eq!(s.gpio.rearms[Key::Boot as usize], 3);
    s.press(2600, Key::Boot);
    s.advance(2700);
    assert_eq!(
        s.consume_ui()
            .into_iter()
            .filter_map(|e| tracker.consume(e))
            .collect::<Vec<_>>(),
        [BootButtonEvent::ShortPress]
    );
}

#[test]
fn debounce_release_and_held_keys_do_not_duplicate_or_poll() {
    for key in KEYS {
        let mut s = Service::new();
        for (at, pressed) in [
            (100, true),
            (105, false),
            (110, true),
            (118, false),
            (120, true),
        ] {
            s.edge(at, key, pressed);
        }
        s.advance(200);
        assert_eq!(s.consume_ui().len(), 1);
        assert_eq!(s.adapter.wait_ms(s.now), None); // held keys block indefinitely
        let wakes = s.deadline_wakes;
        s.advance(30_000);
        assert_eq!(s.deadline_wakes, wakes);
        assert!(s.consume_ui().is_empty());
        for (at, pressed) in [(30_100, false), (30_105, true), (30_110, false)] {
            s.edge(at, key, pressed);
        }
        s.advance(30_200);
        let released = s.consume_ui();
        assert_eq!(released.len(), usize::from(key == Key::Boot));
        s.press(30_300, key);
        s.advance(30_400);
        assert_eq!(s.consume_ui().len(), if key == Key::Boot { 2 } else { 1 });
        assert!(!s.adapter.busy());
        assert_eq!(s.adapter.wait_ms(s.now), None);
    }
}

#[test]
fn fifo_is_arrival_order_not_gpio_order_even_at_equal_timestamps() {
    let mut s = Service::new();
    for key in [Key::Down, Key::Boot, Key::Select, Key::Up] {
        s.gpio.physical_edge(key, true, 100);
    }
    assert!(s.gpio.enabled.iter().all(|enabled| !enabled));
    s.now = 100;
    s.worker();
    assert!(s.gpio.enabled.iter().all(|enabled| *enabled));
    s.advance(200);
    assert_eq!(
        s.consume_ui().iter().map(|e| e.input).collect::<Vec<_>>(),
        [
            nav(ButtonEvent::Down),
            CapturedInput::BootPressed,
            nav(ButtonEvent::Select),
            nav(ButtonEvent::Up),
        ]
    );
}

#[test]
fn overflow_is_observable_bounded_and_does_not_stop_rearm() {
    let mut s = Service::new();
    for i in 0..20 {
        s.press(100 + i * 100, Key::Down);
    }
    s.advance(2200);
    let events = s.consume_ui();
    assert_eq!(events.len(), INPUT_QUEUE_CAPACITY);
    assert_eq!(s.gpio.ui.dropped(), 4);
    assert_eq!(s.gpio.rearms[Key::Down as usize], 41);
    assert_eq!(
        events.iter().map(|e| e.timestamp_ms).collect::<Vec<_>>(),
        (0..16).map(|i| 100 + i * 100).collect::<Vec<_>>()
    );
    s.press(2300, Key::Down);
    s.advance(2400);
    assert_eq!(s.consume_ui().len(), 1);
    assert_eq!(s.gpio.ui.dropped(), 4);
}

#[test]
fn one_shot_fake_loses_notifications_until_adapter_rearms_it() {
    let mut s = Service::new();
    // No service scheduled yet: falling and rising edges while disabled.
    s.gpio.physical_edge(Key::Down, true, 10);
    s.gpio.physical_edge(Key::Down, false, 15);
    assert_eq!(s.gpio.raw.len(), 1);
    assert!(!s.gpio.enabled[Key::Down as usize]);
    s.now = 20;
    s.worker(); // rearm + level reconciliation reject this short bounce
    s.advance(100);
    assert!(s.consume_ui().is_empty());
    s.press(200, Key::Down);
    s.advance(300);
    assert_eq!(s.consume_ui().len(), 1);
}

#[test]
fn rearm_errors_propagate_instead_of_silently_stalling_or_spinning() {
    let mut s = Service::new();
    s.gpio.physical_edge(Key::Select, true, 100);
    let edge = s.gpio.raw.pop_front().unwrap();
    s.gpio.fail_rearm = true;
    assert_eq!(s.adapter.edge(&mut s.gpio, edge), Err("rearm failed"));
    assert!(!s.gpio.enabled[Key::Select as usize]);
}

#[test]
fn payload_memory_is_bounded() {
    assert!(std::mem::size_of::<RawEdge>() <= 16);
    assert!(std::mem::size_of::<CapturedInputEvent>() <= 16);
    assert!(std::mem::size_of::<CaptureAdapter>() <= 104);
}
