//! Boot-time polling and independent runtime capture for the active-low keys.

use core::fmt::Debug;

use anyhow::{anyhow, Result};
use embedded_hal::{delay::DelayNs, digital::InputPin};

pub mod capture;
#[cfg(target_os = "espidf")]
mod espidf;
#[cfg(target_os = "espidf")]
pub use espidf::{InputService, LightSleepOutcome};

const DEBOUNCE_MS: u32 = 25;
/// Hold duration required for GPIO0 BOOT to navigate one hierarchy level back.
pub const BOOT_BACK_LONG_PRESS_MS: u32 = 900;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ButtonEvent {
    Up,
    Select,
    Down,
}

/// Fixed-size debounced event written by the input service and consumed by the
/// UI task. It is intentionally Copy and allocation-free.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapturedInputEvent {
    pub sequence: u32,
    pub timestamp_ms: u64,
    pub input: CapturedInput,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapturedInput {
    Navigation(ButtonEvent),
    BootPressed,
    BootReleased,
}

/// Bounded FIFO used by host adapters (ESP-IDF uses a FreeRTOS queue). Overflow is
/// visible instead of silently reordering or allocating from an interrupt.
#[derive(Debug)]
pub struct InputEventQueue<const CAPACITY: usize> {
    entries: [Option<CapturedInputEvent>; CAPACITY],
    head: usize,
    len: usize,
    next_sequence: u32,
    dropped: u32,
}

impl<const CAPACITY: usize> InputEventQueue<CAPACITY> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: [None; CAPACITY],
            head: 0,
            len: 0,
            next_sequence: 0,
            dropped: 0,
        }
    }

    pub fn push(&mut self, input: CapturedInput, timestamp_ms: u64) -> bool {
        if CAPACITY == 0 || self.len == CAPACITY {
            self.dropped = self.dropped.saturating_add(1);
            return false;
        }
        let index = (self.head + self.len) % CAPACITY;
        let event = CapturedInputEvent {
            sequence: self.next_sequence,
            timestamp_ms,
            input,
        };
        self.next_sequence = self.next_sequence.wrapping_add(1);
        self.entries[index] = Some(event);
        self.len += 1;
        true
    }

    pub fn pop(&mut self) -> Option<CapturedInputEvent> {
        if self.len == 0 {
            return None;
        }
        let event = self.entries[self.head].take();
        self.head = (self.head + 1) % CAPACITY;
        self.len -= 1;
        event
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
    #[must_use]
    pub const fn dropped(&self) -> u32 {
        self.dropped
    }
}

impl<const CAPACITY: usize> Default for InputEventQueue<CAPACITY> {
    fn default() -> Self {
        Self::new()
    }
}

/// Non-blocking BOOT press classifier. The UI task supplies the captured
/// edges, so a held key never spins while display or network work is active.
#[derive(Clone, Copy, Debug, Default)]
pub struct BootPressTracker {
    pressed_at_ms: Option<u64>,
}

impl BootPressTracker {
    pub fn consume(&mut self, event: CapturedInputEvent) -> Option<BootButtonEvent> {
        match event.input {
            CapturedInput::BootPressed => {
                self.pressed_at_ms = Some(event.timestamp_ms);
                None
            }
            CapturedInput::BootReleased => self.pressed_at_ms.take().map(|pressed| {
                if event.timestamp_ms.saturating_sub(pressed) >= u64::from(BOOT_BACK_LONG_PRESS_MS)
                {
                    BootButtonEvent::LongPress
                } else {
                    BootButtonEvent::ShortPress
                }
            }),
            CapturedInput::Navigation(_) => None,
        }
    }
}

/// Boot-time/legacy polling adapter. Runtime transfers the pins to InputService.
pub struct Buttons<UP, SELECT, DOWN> {
    up: UP,
    select: SELECT,
    down: DOWN,
}

impl<UP, SELECT, DOWN> Buttons<UP, SELECT, DOWN>
where
    UP: InputPin,
    UP::Error: Debug,
    SELECT: InputPin,
    SELECT::Error: Debug,
    DOWN: InputPin,
    DOWN::Error: Debug,
{
    #[must_use]
    pub fn new(up: UP, select: SELECT, down: DOWN) -> Self {
        Self { up, select, down }
    }

    /// Return one debounced press. Keys are active low on the Waveshare board.
    pub fn poll<D: DelayNs>(&mut self, delay: &mut D) -> Result<Option<ButtonEvent>> {
        if self.is_pressed(ButtonEvent::Up)? {
            return self.confirm(delay, ButtonEvent::Up);
        }
        if self.is_pressed(ButtonEvent::Select)? {
            return self.confirm(delay, ButtonEvent::Select);
        }
        if self.is_pressed(ButtonEvent::Down)? {
            return self.confirm(delay, ButtonEvent::Down);
        }
        Ok(None)
    }

    fn confirm<D: DelayNs>(
        &mut self,
        delay: &mut D,
        event: ButtonEvent,
    ) -> Result<Option<ButtonEvent>> {
        delay.delay_ms(DEBOUNCE_MS);
        if !self.is_pressed(event)? {
            return Ok(None);
        }

        // Do not generate repeated UI events while the panel is refreshing.
        while self.is_pressed(event)? {
            delay.delay_ms(10);
        }
        Ok(Some(event))
    }

    fn is_pressed(&mut self, event: ButtonEvent) -> Result<bool> {
        match event {
            ButtonEvent::Up => self
                .up
                .is_low()
                .map_err(|error| anyhow!("GPIO4 UP read failed: {error:?}")),
            ButtonEvent::Select => self
                .select
                .is_low()
                .map_err(|error| anyhow!("GPIO5 SELECT read failed: {error:?}")),
            ButtonEvent::Down => self
                .down
                .is_low()
                .map_err(|error| anyhow!("GPIO6 DOWN read failed: {error:?}")),
        }
    }
}

/// Dedicated active-low GPIO0 BOOT-button adapter.
///
/// Long presses remain hierarchy-level Back. Short presses are surfaced so
/// route-specific features such as Sudoku axis selection can use BOOT without
/// changing Back behavior elsewhere.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootButtonEvent {
    ShortPress,
    LongPress,
}

pub struct LongPressBackButton<BACK> {
    back: BACK,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_preserves_short_presses_while_the_ui_is_busy() {
        let mut queue = InputEventQueue::<4>::new();
        // These 80--150 ms presses may arrive while a 700 ms refresh or a
        // multi-second clear is running; ISR capture does not wait for release.
        assert!(queue.push(CapturedInput::Navigation(ButtonEvent::Up), 100));
        assert!(queue.push(CapturedInput::Navigation(ButtonEvent::Select), 180));
        assert!(queue.push(CapturedInput::Navigation(ButtonEvent::Down), 330));
        assert_eq!(
            queue.pop().unwrap().input,
            CapturedInput::Navigation(ButtonEvent::Up)
        );
        assert_eq!(
            queue.pop().unwrap().input,
            CapturedInput::Navigation(ButtonEvent::Select)
        );
        assert_eq!(
            queue.pop().unwrap().input,
            CapturedInput::Navigation(ButtonEvent::Down)
        );
    }

    #[test]
    fn queue_is_bounded_and_reports_overflow_without_reordering() {
        let mut queue = InputEventQueue::<2>::new();
        assert!(queue.push(CapturedInput::Navigation(ButtonEvent::Up), 1));
        assert!(queue.push(CapturedInput::Navigation(ButtonEvent::Down), 2));
        assert!(!queue.push(CapturedInput::Navigation(ButtonEvent::Select), 3));
        assert_eq!(queue.dropped(), 1);
        assert_eq!(queue.pop().unwrap().sequence, 0);
        assert_eq!(queue.pop().unwrap().sequence, 1);
    }

    #[test]
    fn boot_press_is_classified_without_busy_waiting_for_release() {
        let mut tracker = BootPressTracker::default();
        assert_eq!(
            tracker.consume(CapturedInputEvent {
                sequence: 0,
                timestamp_ms: 10,
                input: CapturedInput::BootPressed
            }),
            None
        );
        assert_eq!(
            tracker.consume(CapturedInputEvent {
                sequence: 1,
                timestamp_ms: 100,
                input: CapturedInput::BootReleased
            }),
            Some(BootButtonEvent::ShortPress)
        );
        assert_eq!(
            tracker.consume(CapturedInputEvent {
                sequence: 2,
                timestamp_ms: 200,
                input: CapturedInput::BootPressed
            }),
            None
        );
        assert_eq!(
            tracker.consume(CapturedInputEvent {
                sequence: 3,
                timestamp_ms: 1_100,
                input: CapturedInput::BootReleased
            }),
            Some(BootButtonEvent::LongPress)
        );
    }
}

impl<BACK> LongPressBackButton<BACK>
where
    BACK: InputPin,
    BACK::Error: Debug,
{
    #[must_use]
    pub fn new(back: BACK) -> Self {
        Self { back }
    }

    /// Return one BOOT release classified as short or long.
    pub fn poll<D: DelayNs>(&mut self, delay: &mut D) -> Result<Option<BootButtonEvent>> {
        if !self.is_pressed()? {
            return Ok(None);
        }
        delay.delay_ms(DEBOUNCE_MS);
        if !self.is_pressed()? {
            return Ok(None);
        }

        let mut held_ms = DEBOUNCE_MS;
        while self.is_pressed()? {
            if held_ms >= BOOT_BACK_LONG_PRESS_MS {
                while self.is_pressed()? {
                    delay.delay_ms(10);
                }
                return Ok(Some(BootButtonEvent::LongPress));
            }
            delay.delay_ms(10);
            held_ms = held_ms.saturating_add(10);
        }
        Ok(Some(BootButtonEvent::ShortPress))
    }

    fn is_pressed(&mut self) -> Result<bool> {
        self.back
            .is_low()
            .map_err(|error| anyhow!("GPIO0 BOOT read failed: {error:?}"))
    }
}
