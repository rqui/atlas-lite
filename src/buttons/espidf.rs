//! One owner for all runtime GPIO operations. PinDriver 0.46.2 disables the
//! interrupt before invoking its callback; only this task re-enables it.

use std::sync::{
    atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering},
    Arc,
};
use std::thread::{Builder, JoinHandle};

use anyhow::{anyhow, Result};
use esp_idf_svc::hal::{
    delay::{TickType, BLOCK},
    gpio::{Input, InterruptType, Level, PinDriver},
    sleep::LightSleep,
    task::{self, queue::Queue},
};
use esp_idf_svc::sys::{self, EspError};

use super::capture::{
    CaptureAdapter, CaptureIo, Key, RawEdge, INPUT_QUEUE_CAPACITY, INPUT_STACK_BYTES, KEYS,
    RAW_QUEUE_CAPACITY,
};
use super::{Buttons, CapturedInput, CapturedInputEvent, LongPressBackButton};

#[derive(Clone, Copy)]
enum Message {
    Edge(RawEdge),
    AttemptLightSleep,
    Stop,
}

/// Result of the serialized GPIO handoff. It deliberately does not create a
/// navigation event: the capture adapter remains the single event source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LightSleepOutcome {
    /// Input was present at the final handoff, so no MCU sleep was attempted.
    CancelledForInput,
    /// ESP-IDF accepted light sleep and returned after a configured wake.
    SleptAndWoke,
}

#[derive(Clone, Copy)]
enum Reply {
    Started(core::result::Result<(), EspError>),
    Sleep(core::result::Result<LightSleepOutcome, EspError>),
}

// Payload budgets checked by the actual Xtensa compiler, not inferred from host.
const _: () = assert!(core::mem::size_of::<Message>() <= 24);
const _: () = assert!(core::mem::size_of::<CapturedInputEvent>() <= 16);
const _: () = assert!(core::mem::size_of::<CaptureAdapter>() <= 104);
const _: () = assert!(core::mem::size_of::<Reply>() <= 16);

struct Shared {
    raw: Queue<Message>,
    output: Queue<CapturedInputEvent>,
    reply: Queue<Reply>,
    dropped: AtomicU32,
    lost_keys: AtomicU32,
    /// Incremented by ISR before queueing any raw edge. The final handoff
    /// checks it after wake sources are armed, rather than trusting an old UI
    /// queue snapshot from before panel/network preparation.
    edge_epoch: AtomicU32,
    started: AtomicBool,
    fault: AtomicI32,
}

fn queue<T: Copy>(capacity: usize) -> Result<Queue<T>> {
    let queue = Queue::new(capacity);
    if queue.as_raw().is_null() {
        // HAL's Drop assumes allocation succeeded. Do not delete a null handle.
        core::mem::forget(queue);
        return Err(anyhow!("input queue allocation failed"));
    }
    Ok(queue)
}

fn now_ms() -> u64 {
    // ESP-IDF's lock-free timer read is permitted in ISR context.
    unsafe { sys::esp_timer_get_time() as u64 / 1_000 }
}

pub struct InputService {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}

impl InputService {
    pub fn start(
        buttons: Buttons<
            PinDriver<'static, Input>,
            PinDriver<'static, Input>,
            PinDriver<'static, Input>,
        >,
        back: LongPressBackButton<PinDriver<'static, Input>>,
    ) -> Result<Self> {
        let shared = Arc::new(Shared {
            raw: queue(RAW_QUEUE_CAPACITY)?,
            output: queue(INPUT_QUEUE_CAPACITY)?,
            reply: queue(1)?,
            dropped: AtomicU32::new(0),
            lost_keys: AtomicU32::new(0),
            edge_epoch: AtomicU32::new(0),
            started: AtomicBool::new(false),
            fault: AtomicI32::new(0),
        });
        let service = shared.clone();
        let worker = Builder::new()
            .name("atlas-input".into())
            .stack_size(INPUT_STACK_BYTES)
            .spawn(move || {
                // Short bursts above the UI priority; indefinite queue wait
                // when idle. No timer, busy loop or light-sleep power lock.
                unsafe { sys::vTaskPrioritySet(core::ptr::null_mut(), 8) };
                let mut gpio = GpioCapture {
                    pins: [buttons.up, buttons.select, buttons.down, back.back],
                    shared: service.clone(),
                    sequence: 0,
                };
                if let Err(error) = gpio.run() {
                    service.fault.store(error.code(), Ordering::Release);
                    // Also wakes the caller if startup or sleep failed.
                    let reply = if service.started.load(Ordering::Acquire) {
                        Reply::Sleep(Err(error))
                    } else {
                        Reply::Started(Err(error))
                    };
                    let _ = service.reply.send_back(reply, 0);
                }
                // Dropping pins unsubscribes callbacks before their queues die.
            })?;
        let result = Self {
            shared,
            worker: Some(worker),
        };
        result.wait_started()?;
        Ok(result)
    }

    fn check(&self) -> Result<()> {
        let fault = self.shared.fault.load(Ordering::Acquire);
        if fault != 0 {
            return Err(anyhow!("input capture stopped: ESP error {fault}"));
        }
        Ok(())
    }

    fn wait_started(&self) -> Result<()> {
        let (reply, _) = self
            .shared
            .reply
            .recv_front(BLOCK)
            .ok_or_else(|| anyhow!("input service reply unavailable"))?;
        match reply {
            Reply::Started(result) => result?,
            Reply::Sleep(_) => return Err(anyhow!("input service reply ordering failure")),
        }
        self.check()
    }

    pub fn take(&self) -> Result<Option<CapturedInputEvent>> {
        self.check()?;
        Ok(self.shared.output.recv_front(0).map(|(event, _)| event))
    }

    pub fn pending(&self) -> bool {
        self.shared.output.peek_front(0).is_some()
            || self.shared.raw.peek_front(0).is_some()
            || self.shared.fault.load(Ordering::Acquire) != 0
    }

    pub fn dropped(&self) -> u32 {
        self.shared.dropped.load(Ordering::Relaxed)
    }

    /// Serializes final edge draining, wake-source arming and MCU entry in the
    /// GPIO-owner task. It does not synthesize input; callers consume the
    /// adapter's ordinary FIFO after a cancellation or wake.
    pub fn enter_light_sleep(&self) -> Result<LightSleepOutcome> {
        self.check()?;
        self.shared
            .raw
            .send_back(Message::AttemptLightSleep, BLOCK)?;
        let (reply, _) = self
            .shared
            .reply
            .recv_front(BLOCK)
            .ok_or_else(|| anyhow!("input sleep reply unavailable"))?;
        match reply {
            Reply::Sleep(result) => result.map_err(Into::into),
            Reply::Started(_) => Err(anyhow!("input service reply ordering failure")),
        }
    }
}

impl Drop for InputService {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            if !worker.is_finished() {
                let _ = self.shared.raw.send_back(Message::Stop, BLOCK);
            }
            let _ = worker.join();
        }
    }
}

struct GpioCapture {
    pins: [PinDriver<'static, Input>; 4],
    shared: Arc<Shared>,
    sequence: u32,
}

impl GpioCapture {
    fn run(&mut self) -> core::result::Result<(), EspError> {
        for key in KEYS {
            let pin = &mut self.pins[key as usize];
            pin.set_interrupt_type(InterruptType::AnyEdge)?;
            let number = pin.pin();
            let shared = self.shared.clone();
            // SAFETY: callback owns its queue reference, performs only a GPIO
            // read, timer read, ISR-safe queue copy/yield and atomic bookkeeping.
            // No rearm, allocation, mutex, logging, display or network in ISR.
            unsafe {
                pin.subscribe(move || {
                    shared.edge_epoch.fetch_add(1, Ordering::Release);
                    let edge = RawEdge {
                        key,
                        pressed: sys::gpio_get_level(number.into()) == 0,
                        timestamp_ms: now_ms(),
                    };
                    match shared.raw.send_back(Message::Edge(edge), 0) {
                        Ok(true) => task::do_yield(),
                        Ok(false) => {}
                        Err(_) => {
                            shared.dropped.fetch_add(1, Ordering::Relaxed);
                            shared.lost_keys.fetch_or(1 << key as u8, Ordering::Release);
                        }
                    }
                })?;
            }
        }
        let mut adapter = CaptureAdapter::default();
        adapter.start(self, now_ms())?;
        self.shared.started.store(true, Ordering::Release);
        self.shared.reply.send_back(Reply::Started(Ok(())), 0)?;
        loop {
            let timeout = adapter
                .wait_ms(now_ms())
                .map(|ms| TickType::new_millis(ms.max(1)).into())
                .unwrap_or(BLOCK);
            let mut message = self.shared.raw.recv_front(timeout);
            while let Some((item, _)) = message {
                match item {
                    Message::Edge(edge) => adapter.edge(self, edge)?,
                    Message::Stop => return Ok(()),
                    Message::AttemptLightSleep => {
                        // This command is ordered after every raw edge already
                        // copied by an ISR. Reconcile once more, then record
                        // the epoch that covers panel/network preparation.
                        adapter.settle(self, now_ms());
                        let observed_epoch = self.shared.edge_epoch.load(Ordering::Acquire);
                        let outcome = if adapter.permits_sleep_handoff(
                            self.shared.raw.peek_front(0).is_some(),
                            self.shared.output.peek_front(0).is_some(),
                            observed_epoch,
                            self.shared.edge_epoch.load(Ordering::Acquire),
                        ) {
                            self.sleep(&adapter, observed_epoch)
                        } else {
                            Ok(LightSleepOutcome::CancelledForInput)
                        };
                        // `sleep` has restored AnyEdge/rearm. Settle catches
                        // its wake key; do not manufacture a duplicate event.
                        adapter.settle(self, now_ms());
                        self.shared.reply.send_back(Reply::Sleep(outcome), 0)?;
                    }
                }
                message = self.shared.raw.recv_front(0);
            }
            // At most four one-shot GPIO notifications plus one UI command can
            // normally be outstanding. Still fail observably on any overflow,
            // and never leave that GPIO permanently disabled.
            let lost = self.shared.lost_keys.swap(0, Ordering::AcqRel);
            for key in KEYS {
                if lost & (1 << key as u8) != 0 {
                    self.rearm(key)?;
                }
            }
            adapter.settle(self, now_ms());
        }
    }

    fn sleep(
        &mut self,
        adapter: &CaptureAdapter,
        observed_epoch: u32,
    ) -> core::result::Result<LightSleepOutcome, EspError> {
        let result = (|| {
            let mut sleep = LightSleep::new()?;
            for key in [Key::Up, Key::Select, Key::Down] {
                sleep = sleep.wakeup_on_gpio(&self.pins[key as usize], Level::Low)?;
            }
            // Check after arming the actual ESP-IDF wake sources. An edge
            // during preparation is visible either in this epoch/queue or as
            // the configured low-level wake from esp_light_sleep_start.
            if !adapter.permits_sleep_handoff(
                self.shared.raw.peek_front(0).is_some(),
                self.shared.output.peek_front(0).is_some(),
                observed_epoch,
                self.shared.edge_epoch.load(Ordering::Acquire),
            ) {
                return Ok(LightSleepOutcome::CancelledForInput);
            }
            sleep.enter()?;
            Ok(LightSleepOutcome::SleptAndWoke)
        })();
        // GPIO wake configuration uses level interrupts. Restore both edges,
        // including releases, outside ISR even if sleep was rejected.
        for key in KEYS {
            self.pins[key as usize].set_interrupt_type(InterruptType::AnyEdge)?;
            self.rearm(key)?;
        }
        result
    }
}

impl CaptureIo for GpioCapture {
    type Error = EspError;

    fn pressed(&self, key: Key) -> bool {
        self.pins[key as usize].is_low()
    }

    fn rearm(&mut self, key: Key) -> core::result::Result<(), EspError> {
        self.pins[key as usize].enable_interrupt()
    }

    fn emit(&mut self, input: CapturedInput, timestamp_ms: u64) {
        let event = CapturedInputEvent {
            sequence: self.sequence,
            timestamp_ms,
            input,
        };
        if self.shared.output.send_back(event, 0).is_err() {
            self.shared.dropped.fetch_add(1, Ordering::Relaxed);
        } else {
            self.sequence = self.sequence.wrapping_add(1);
        }
    }
}
