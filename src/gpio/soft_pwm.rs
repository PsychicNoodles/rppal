// Copyright (c) 2017-2021 Rene van der Meer
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
// THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

// Prevent warning when casting u32 as i64
#![allow(clippy::cast_lossless)]
#![allow(dead_code)]

use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use libc::{
    self, c_long, CLOCK_MONOTONIC, PR_SET_TIMERSLACK, sched_param, SCHED_RR, time_t, timespec,
};

use super::{Error, GpioState, Result};

// Only call sleep_ns() if we have enough time remaining
const SLEEP_THRESHOLD: i64 = 250_000;
// Reserve some time for busy waiting
const BUSYWAIT_MAX: i64 = 200_000;
// Subtract from the remaining busy wait time to account for get_time_ns() overhead
const BUSYWAIT_REMAINDER: i64 = 100;

const NANOS_PER_SEC: i64 = 1_000_000_000;
const NANOS_PER_SEC_F: f64 = 1_000_000_000.0;

/// `period` indicates the time it takes to complete one cycle.
///
/// `pulse_width` indicates the amount of time the PWM signal is active during a
/// single period.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PwmPulse {
    pub period: Duration,
    pub pulse_width: Duration,
}

impl From<(Duration, Duration)> for PwmPulse {
    fn from((period, pulse_width): (Duration, Duration)) -> Self {
        PwmPulse { period, pulse_width }
    }
}

impl From<PwmPulse> for (i64, i64) {
    fn from(PwmPulse { period, pulse_width }: PwmPulse) -> Self {
        (period.as_nanos() as i64, pulse_width.as_nanos() as i64)
    }
}

/// `frequency` is specified in hertz (Hz).
///
/// `duty_cycle` is specified as a floating point value between `0.0` (0%) and `1.0` (100%).
#[derive(Debug, PartialEq, Clone)]
pub struct PwmFrequency {
    pub frequency: f64,
    pub duty_cycle: f64,
}

impl From<(f64, f64)> for PwmFrequency {
    fn from((frequency, duty_cycle): (f64, f64)) -> Self {
        PwmFrequency { frequency, duty_cycle }
    }
}

impl From<PwmFrequency> for PwmPulse {
    fn from(PwmFrequency { frequency, duty_cycle }: PwmFrequency) -> Self {
        let period = if frequency <= 0.0 {
            0.0
        } else {
            (1.0 / frequency) * NANOS_PER_SEC_F
        };
        let pulse_width = period * duty_cycle.max(0.0).min(1.0);
        PwmPulse {
            period: Duration::from_nanos(period as u64),
            pulse_width: Duration::from_nanos(pulse_width as u64),
        }
    }
}

/// `Pulse` ([`PwmPulse`]):
///
/// `period` indicates the time it takes to complete one cycle.
///
/// `pulse_width` indicates the amount of time the PWM signal is active during a
/// single period.
///
/// `Frequency` ([`PwmFrequency`]):
///
/// `frequency` is specified in hertz (Hz).
///
/// `duty_cycle` is specified as a floating point value between `0.0` (0%) and `1.0` (100%).
///
/// [`PwmPulse`]: ./struct.PwmPulse.html
/// [`PwmFrequency`]: ./struct.PwmFrequency.html
#[derive(Debug, PartialEq, Clone)]
pub enum PwmWave {
    Pulse(PwmPulse),
    Frequency(PwmFrequency),
    Wait(Duration),
}

#[derive(Debug, PartialEq, Clone)]
enum Msg {
    Reconfigure(Vec<PwmWave>, bool),
    Stop,
}

fn prepare_iter<T>(period_pulse_widths: T, repeat_indefinitely: bool) -> Box<dyn Iterator<Item=(i64, i64)>> where T: IntoIterator<Item=PwmWave>, <T as IntoIterator>::IntoIter: Clone + 'static {
    let base_iter = period_pulse_widths.into_iter().map(|wave| From::from(match wave {
        PwmWave::Pulse(pulse) => pulse,
        PwmWave::Frequency(freq) => PwmPulse::from(freq),
        PwmWave::Wait(period) => PwmPulse { period, pulse_width: Duration::ZERO }
    }));
    if repeat_indefinitely {
        Box::new(base_iter.cycle())
    } else {
        Box::new(base_iter)
    }
}

fn process_msg(msg: Msg) -> Option<Box<dyn Iterator<Item=(i64, i64)>>> {
    match msg {
        Msg::Reconfigure(period_pulse_widths, repeat_indefinitely) => {
            // Reconfigure period and pulse width
            Some(prepare_iter(period_pulse_widths, repeat_indefinitely))
        }
        Msg::Stop => {
            // The main thread asked us to stop
            None
        }
    }
}

#[derive(Debug)]
pub(crate) struct SoftPwm {
    pwm_thread: Option<thread::JoinHandle<Result<()>>>,
    sender: Sender<Msg>,
    running: Arc<AtomicBool>,
}

impl SoftPwm {
    pub(crate) fn new<T>(pin: u8, gpio_state: Arc<GpioState>, sequence: T, repeat_indefinitely: bool) -> SoftPwm where T: IntoIterator<Item=PwmWave> + Send + Sync + 'static, <T as IntoIterator>::IntoIter: Clone {
        let running = Arc::new(AtomicBool::new(false));
        let (sender, pwm_thread) = SoftPwm::start(pin, gpio_state, sequence, repeat_indefinitely, running.clone());

        SoftPwm {
            pwm_thread: Some(pwm_thread),
            sender,
            running,
        }
    }

    fn start<T>(pin: u8, gpio_state: Arc<GpioState>, period_pulse_widths: T, repeat_indefinitely: bool, running: Arc<AtomicBool>) -> (Sender<Msg>, JoinHandle<Result<()>>) where T: IntoIterator<Item=PwmWave> + Send + Sync + 'static, <T as IntoIterator>::IntoIter: Clone {
        let (sender, receiver): (Sender<Msg>, Receiver<Msg>) = mpsc::channel();

        let pwm_thread = thread::spawn(move || -> Result<()> {
            // Set the scheduling policy to real-time round robin at the highest priority. This
            // will silently fail if we're not running as root.
            #[cfg(target_env = "gnu")]
                let params = sched_param {
                sched_priority: unsafe { libc::sched_get_priority_max(SCHED_RR) },
            };

            #[cfg(target_env = "musl")]
                let params = sched_param {
                sched_priority: unsafe { libc::sched_get_priority_max(SCHED_RR) },
                sched_ss_low_priority: 0,
                sched_ss_repl_period: timespec {
                    tv_sec: 0,
                    tv_nsec: 0,
                },
                sched_ss_init_budget: timespec {
                    tv_sec: 0,
                    tv_nsec: 0,
                },
                sched_ss_max_repl: 0,
            };

            unsafe {
                libc::sched_setscheduler(0, SCHED_RR, &params);
            }

            // Set timer slack to 1 ns (default = 50 µs). This is only relevant if we're unable
            // to set a real-time scheduling policy.
            unsafe {
                libc::prctl(PR_SET_TIMERSLACK, 1);
            }

            let mut ppw_iter = prepare_iter(period_pulse_widths, repeat_indefinitely);
            let mut ppw_next = ppw_iter.next();
            if ppw_next.is_some() {
                running.store(true, Ordering::Release);
            }

            let mut start_ns = get_time_ns();

            loop {
                match ppw_next {
                    Some((period_ns, pulse_width_ns)) => {
                        // PWM active
                        if pulse_width_ns > 0 {
                            gpio_state.gpio_mem.set_high(pin);
                        }

                        // Sleep if we have enough time remaining, while reserving some time
                        // for busy waiting to compensate for sleep taking longer than needed.
                        if pulse_width_ns >= SLEEP_THRESHOLD {
                            sleep_ns(pulse_width_ns - BUSYWAIT_MAX);
                        }

                        // Busy-wait for the remaining active time, minus BUSYWAIT_REMAINDER
                        // to account for get_time_ns() overhead
                        loop {
                            if (pulse_width_ns - (get_time_ns() - start_ns)) <= BUSYWAIT_REMAINDER {
                                break;
                            }
                        }

                        // PWM inactive
                        gpio_state.gpio_mem.set_low(pin);

                        // Get next PWM duration and prepare next period duration
                        while let Ok(msg) = receiver.try_recv() {
                            match process_msg(msg) {
                                Some(_ppw_iter) => ppw_iter = _ppw_iter,
                                None => return Ok(())
                            }
                        }

                        // Buffer next pair
                        ppw_next = ppw_iter.next();

                        let remaining_ns = period_ns - (get_time_ns() - start_ns);

                        // Sleep if we have enough time remaining, while reserving some time
                        // for busy waiting to compensate for sleep taking longer than needed.
                        if remaining_ns >= SLEEP_THRESHOLD {
                            sleep_ns(remaining_ns - BUSYWAIT_MAX);
                        }

                        // Busy-wait for the remaining inactive time, minus BUSYWAIT_REMAINDER
                        // to account for get_time_ns() overhead
                        loop {
                            let current_ns = get_time_ns();
                            if (period_ns - (current_ns - start_ns)) <= BUSYWAIT_REMAINDER {
                                start_ns = current_ns;
                                break;
                            }
                        }
                    }
                    None => {
                        running.store(false, Ordering::Release);
                        match receiver.recv().map(process_msg) {
                            Ok(Some(_ppw_iter)) => ppw_iter = _ppw_iter,
                            // Received a Stop msg or recv returned an error
                            Ok(None) | Err(_) => return Ok(()),
                        }
                        // Get through any other messages that may have been queued up
                        while let Ok(msg) = receiver.try_recv() {
                            match process_msg(msg) {
                                Some(_ppw_iter) => ppw_iter = _ppw_iter,
                                None => return Ok(())
                            }
                        }
                        running.store(true, Ordering::Release);
                    }
                }
            }
        });
        (sender, pwm_thread)
    }

    pub(crate) fn reconfigure<T: IntoIterator<Item=PwmWave>>(&mut self, period_pulse_widths: T, repeat_indefinitely: bool) {
        let _ = self.sender.send(Msg::Reconfigure(period_pulse_widths.into_iter().collect(), repeat_indefinitely));
    }

    pub(crate) fn stop(&mut self) -> Result<()> {
        let _ = self.sender.send(Msg::Stop);
        if let Some(pwm_thread) = self.pwm_thread.take() {
            match pwm_thread.join() {
                Ok(r) => return r,
                Err(_) => return Err(Error::ThreadPanic),
            }
        }

        Ok(())
    }

    pub(crate) fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }
}

impl Drop for SoftPwm {
    fn drop(&mut self) {
        // Don't wait for the pwm thread to exit if the main thread is panicking,
        // because we could potentially block indefinitely while unwinding if the
        // pwm thread doesn't respond to the Stop message for some reason.
        if !thread::panicking() {
            let _ = self.stop();
        }
    }
}

// Required because Sender isn't Sync. Implementing Sync for SoftPwm is
// safe because all usage of Sender::send() is locked behind &mut self.
unsafe impl Sync for SoftPwm {}

#[inline(always)]
fn get_time_ns() -> i64 {
    let mut ts = timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };

    unsafe {
        libc::clock_gettime(CLOCK_MONOTONIC, &mut ts);
    }

    (ts.tv_sec as i64 * NANOS_PER_SEC) + ts.tv_nsec as i64
}

#[inline(always)]
fn sleep_ns(ns: i64) {
    let ts = timespec {
        tv_sec: (ns / NANOS_PER_SEC) as time_t,
        tv_nsec: (ns % NANOS_PER_SEC) as c_long,
    };

    unsafe {
        libc::clock_nanosleep(CLOCK_MONOTONIC, 0, &ts, ptr::null_mut());
    }
}
