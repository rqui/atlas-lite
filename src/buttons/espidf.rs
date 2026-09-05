//! One owner for all runtime GPIO operations. PinDriver 0.46.2 disables the
//! interrupt before invoking its callback; only this task re-enables it.

use std::sync::{
    atomic::{AtomicI32, AtomicU32, Ordering},
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
    Sleep,
    Stop,
}

// Payload budgets checked by the actual Xtensa compiler, not inferred from host.
const _: () = assert!(core::mem::size_of::<Message>() <= 24);
const _: () = assert!(core::mem::size_of::<CapturedInputEvent>() <= 16);
const _: () = assert!(core::mem::size_of::<CaptureAdapter>() <= 104);

struct Shared {
    raw: Queue<Message>,
    output: Queue<CapturedInputEvent>,
    reply: Queue<core::result::Result<(), EspError>>,
    dropped: AtomicU32,
    lost_keys: AtomicU32,
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
                    let _ = service.reply.send_back(Err(error), 0);
                }
                // Dropping pins unsubscribes callbacks before their queues die.
            })?;
        let result = Self {
            shared,
            worker: Some(worker),
        };
        result.wait_reply()?;
        Ok(result)
    }

    fn check(&self) -> Result<()> {
        let fault = self.shared.fault.load(Ordering::Acquire);
        if fault != 0 {
            return Err(anyhow!("input capture stopped: ESP error {fault}"));
        }
        Ok(())
    }

    fn wait_reply(&self) -> Result<()> {
        let (result, _) = self
            .shared
            .reply
            .recv_front(BLOCK)
            .ok_or_else(|| anyhow!("input service reply unavailable"))?;
        result?;
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

    /// Keeps the existing wake sources under the sole GPIO owner. The broader
    /// panel/network preparation and sleep-entry race remain a separate finding.
    pub fn enter_light_sleep(&self) -> Result<()> {
        self.check()?;
        self.shared.raw.send_back(Message::Sleep, BLOCK)?;
        self.wait_reply()
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
        self.shared.reply.send_back(Ok(()), 0)?;
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
                    Message::Sleep => {
                        // A pending press, unsettled edge or held key cancels
                        // this attempt. UI state/NVS are never owned here.
                        adapter.settle(self, now_ms());
                        if !adapter.busy()
                            && self.shared.raw.peek_front(0).is_none()
                            && self.shared.output.peek_front(0).is_none()
                        {
                            self.sleep()?;
                        }
                        self.shared.reply.send_back(Ok(()), 0)?;
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

    fn sleep(&mut self) -> core::result::Result<(), EspError> {
        let result = (|| {
            let mut sleep = LightSleep::new()?;
            for key in [Key::Up, Key::Select, Key::Down] {
                sleep = sleep.wakeup_on_gpio(&self.pins[key as usize], Level::Low)?;
            }
            sleep.enter()
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
