//! Debounced capture adapter shared by ESP-IDF and the one-shot GPIO fake.
//! Only this adapter decides which physical transitions become UI events.

use super::{ButtonEvent, CapturedInput, DEBOUNCE_MS};

pub const INPUT_QUEUE_CAPACITY: usize = 16;
pub const RAW_QUEUE_CAPACITY: usize = 8;
pub const INPUT_STACK_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Key {
    Up,
    Select,
    Down,
    Boot,
}

pub const KEYS: [Key; 4] = [Key::Up, Key::Select, Key::Down, Key::Boot];

#[derive(Clone, Copy, Debug)]
pub struct RawEdge {
    pub key: Key,
    pub pressed: bool,
    pub timestamp_ms: u64,
}

/// Rearm is always called by the service task, never the ISR or UI task.
pub trait CaptureIo {
    type Error;
    fn pressed(&self, key: Key) -> bool;
    fn rearm(&mut self, key: Key) -> Result<(), Self::Error>;
    fn emit(&mut self, input: CapturedInput, timestamp_ms: u64);
}

#[derive(Clone, Copy, Default)]
struct DebouncedKey {
    observed: bool,
    stable: bool,
    since_ms: u64,
    order: u32,
}

#[derive(Default)]
pub struct CaptureAdapter {
    keys: [DebouncedKey; 4],
    next_order: u32,
}

impl CaptureAdapter {
    pub fn start<H: CaptureIo>(&mut self, io: &mut H, now: u64) -> Result<(), H::Error> {
        for key in KEYS {
            io.rearm(key)?;
        }
        self.settle(io, now);
        Ok(())
    }

    /// Consume the timestamped edge before rearming the HAL one-shot.
    /// Draining queued edges before settle() preserves inter-key FIFO order.
    pub fn edge<H: CaptureIo>(&mut self, io: &mut H, edge: RawEdge) -> Result<(), H::Error> {
        self.advance(io, edge.timestamp_ms);
        self.observe(edge.key, edge.pressed, edge.timestamp_ms);
        io.rearm(edge.key)
    }

    /// One level reconciliation on a notification or debounce deadline catches
    /// a transition while the HAL interrupt was disabled. No periodic polling.
    pub fn settle<H: CaptureIo>(&mut self, io: &mut H, now: u64) {
        for key in KEYS {
            self.observe(key, io.pressed(key), now);
        }
        self.advance(io, now);
    }

    fn observe(&mut self, key: Key, pressed: bool, now: u64) {
        let state = &mut self.keys[key as usize];
        if state.observed != pressed {
            state.observed = pressed;
            state.since_ms = now;
            state.order = self.next_order;
            self.next_order = self.next_order.wrapping_add(1);
        }
    }

    fn advance<H: CaptureIo>(&mut self, io: &mut H, now: u64) {
        // At most four transitions; deadline then arrival order, not GPIO index.
        for _ in KEYS {
            let ready = KEYS
                .into_iter()
                .filter(|key| {
                    let s = self.keys[*key as usize];
                    s.observed != s.stable
                        && now.saturating_sub(s.since_ms) >= u64::from(DEBOUNCE_MS)
                })
                .min_by(|a, b| {
                    let a = self.keys[*a as usize];
                    let b = self.keys[*b as usize];
                    a.since_ms
                        .cmp(&b.since_ms)
                        .then_with(|| (a.order.wrapping_sub(b.order) as i32).cmp(&0))
                });
            let Some(key) = ready else { break };
            let state = &mut self.keys[key as usize];
            state.stable = state.observed;
            let input = match (key, state.stable) {
                (Key::Up, true) => Some(CapturedInput::Navigation(ButtonEvent::Up)),
                (Key::Select, true) => Some(CapturedInput::Navigation(ButtonEvent::Select)),
                (Key::Down, true) => Some(CapturedInput::Navigation(ButtonEvent::Down)),
                (Key::Boot, true) => Some(CapturedInput::BootPressed),
                (Key::Boot, false) => Some(CapturedInput::BootReleased),
                (_, false) => None,
            };
            if let Some(input) = input {
                io.emit(input, state.since_ms);
            }
        }
    }

    /// None means block indefinitely until an edge. Held keys do not tick.
    pub fn wait_ms(&self, now: u64) -> Option<u64> {
        self.keys
            .iter()
            .filter(|s| s.observed != s.stable)
            .map(|s| {
                s.since_ms
                    .saturating_add(u64::from(DEBOUNCE_MS))
                    .saturating_sub(now)
            })
            .min()
    }

    pub fn busy(&self) -> bool {
        self.keys.iter().any(|s| s.observed || s.stable)
    }

    /// Shared final sleep handoff predicate. The target calls this immediately
    /// before and after configuring ESP-IDF GPIO wake; host regressions drive
    /// it through the same edge/debounce adapter rather than injecting UI
    /// events. `edge_epoch` is incremented by the real ISR before enqueueing.
    #[must_use]
    pub fn permits_sleep_handoff(
        &self,
        raw_pending: bool,
        semantic_pending: bool,
        armed_epoch: u32,
        edge_epoch: u32,
    ) -> bool {
        !self.busy() && !raw_pending && !semantic_pending && armed_epoch == edge_epoch
    }
}
