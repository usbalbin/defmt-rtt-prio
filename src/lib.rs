//! [`defmt-rtt-prio`](https://github.com/usbalbin/defmt-rtt-prio) global lock free logger over RTT.
//!
//! This is based on defmt-rtt from [knurling-rs/defmt](https://github.com/knurling-rs/defmt). However
//! `defmt-rtt-prio` avoids any critical sections by exploiting the fact that interrupts of the same
//! priority can not interrupt each other. We setup one RTT UP channel per NVIC priority level. By
//! mapping each priority to its own RTT channel, we can guarantee that there will be no problems.
//!
//! NOTE when using this crate it's not possible to use (link to) the
//! `defmt-rtt` or `rtt-target` crates
//!
//! To use this crate, link to it by importing it somewhere in your project.
//!
//! ```
//! // src/main.rs or src/bin/my-app.rs
//! use defmt_rtt_prio as _;
//! ```
//!
//! # Blocking/Non-blocking and losing data
//!
//! This crate will never block if the buffer fills up. Instead the oldest data will be overwritten.
//! If losing data is a problem then `defmt-rtt` might be a better option.
//!
#![no_std]

#[cfg(not(any(
    feature = "prio_bits_2",
    feature = "prio_bits_3",
    feature = "prio_bits_4",
    feature = "prio_bits_5",
    feature = "prio_bits_6",
    feature = "prio_bits_7",
    feature = "prio_bits_8"
)))]
compile_error!(
    "Please select number of interrupt priority bits using one of the `prio_bits_X` features:
* `prio_bits_2`
* `prio_bits_3`
* `prio_bits_4`
* `prio_bits_5`
* `prio_bits_6`
* `prio_bits_7`
* `prio_bits_8`
"
);

// Ensure exactly one prio_bits feature is selected
#[cfg(any(
    all(
        feature = "prio_bits_2",
        any(
            feature = "prio_bits_3",
            feature = "prio_bits_4",
            feature = "prio_bits_5",
            feature = "prio_bits_6",
            feature = "prio_bits_7",
            feature = "prio_bits_8"
        )
    ),
    all(
        feature = "prio_bits_3",
        any(
            feature = "prio_bits_4",
            feature = "prio_bits_5",
            feature = "prio_bits_6",
            feature = "prio_bits_7",
            feature = "prio_bits_8"
        )
    ),
    all(
        feature = "prio_bits_4",
        any(
            feature = "prio_bits_5",
            feature = "prio_bits_6",
            feature = "prio_bits_7",
            feature = "prio_bits_8"
        )
    ),
    all(
        feature = "prio_bits_5",
        any(
            feature = "prio_bits_6",
            feature = "prio_bits_7",
            feature = "prio_bits_8"
        )
    ),
    all(
        feature = "prio_bits_6",
        any(feature = "prio_bits_7", feature = "prio_bits_8")
    ),
    all(feature = "prio_bits_7", feature = "prio_bits_8"),
))]
compile_error!("Only one `prio_bits_X` feature may be enabled at a time");

mod channel;
mod consts;

use core::{
    arch::asm,
    cell::UnsafeCell,
    sync::atomic::{AtomicU32, Ordering},
};

use crate::{
    channel::Channel,
    consts::{BUF_SIZE, PRIO_BITS, UP_CHANNELS},
};

/// The relevant bits in the mode field in the Header
const MODE_MASK: u32 = 0b11;

/// Block the application if the RTT buffer is full, wait for the host to read data.
const MODE_BLOCK_IF_FULL: u32 = 2;

/// Don't block if the RTT buffer is full. Truncate data to output as much as fits.
const MODE_NON_BLOCKING_TRIM: u32 = 1;

/// The defmt global logger
///
/// The defmt crate requires that this be a unit type, so our state is stored in
/// [`RTT_ENCODER`] instead.
#[defmt::global_logger]
struct Logger;

/// Our defmt encoder state
static RTT_ENCODER: RttEncoder = RttEncoder::new();

/// Our shared header structure.
///
/// The host will read this structure so it must be arranged as expected.
///
/// NOTE the `rtt-target` API is too permissive. It allows writing arbitrary
/// data to any channel (`set_print_channel` + `rprint*`) and that can corrupt
/// defmt log frames. So we declare the RTT control block here and make it
/// impossible to use `rtt-target` together with this crate.
#[no_mangle]
static _SEGGER_RTT: Header = Header {
    id: *b"SEGGER RTT\0\0\0\0\0\0",
    max_up_channels: UP_CHANNELS as u32,
    max_down_channels: 0,
    up_channels: {
        let mut chs = [const { Channel::zero() }; UP_CHANNELS];

        let mut i = 0;
        while i < UP_CHANNELS {
            chs[i].name = NAME.as_ptr();
            chs[i].buffer = BUFFERS[i].get();
            chs[i].size = BUF_SIZE as u32;
            chs[i].write = AtomicU32::new(0);
            chs[i].read = AtomicU32::new(0);
            chs[i].flags = AtomicU32::new(MODE_NON_BLOCKING_TRIM);
            i += 1;
        }
        chs
    },
};

/// Report whether the first SEGGER RTT up channel is in blocking mode.
///
/// Returns true if the mode bitfield within the flags value has been set to
/// `SEGGER_RTT_MODE_BLOCK_IF_FIFO_FULL`.
///
/// Currently we start-up in non-blocking mode, so if it's been set to blocking
/// mode then the connected client (e.g. probe-rs) must have done it.
pub fn in_blocking_mode() -> bool {
    (_SEGGER_RTT.up_channels[0].flags.load(Ordering::Relaxed) & MODE_MASK) == MODE_BLOCK_IF_FULL
}

/// Our shared buffer
#[cfg_attr(target_os = "macos", link_section = ".uninit,defmt-rtt.BUFFER")]
#[cfg_attr(not(target_os = "macos"), link_section = ".uninit.defmt-rtt.BUFFER")]
static BUFFERS: [Buffer; UP_CHANNELS] = [const { Buffer::new() }; UP_CHANNELS];

/// The name of our channel.
///
/// This is in a data section, so the whole RTT header can be read from RAM.
/// This is useful if flash access gets disabled by the firmware at runtime.
#[cfg_attr(target_os = "macos", link_section = ".data,defmt-rtt.NAME")]
#[cfg_attr(not(target_os = "macos"), link_section = ".data.defmt-rtt.NAME")]
static NAME: [u8; 6] = *b"defmt\0";

struct RttEncoder {
    /// A defmt::Encoder for encoding frames
    encoders: [UnsafeCell<defmt::Encoder>; UP_CHANNELS],
}

impl RttEncoder {
    /// Create a new semihosting-based defmt-encoder
    const fn new() -> RttEncoder {
        RttEncoder {
            encoders: [const { UnsafeCell::new(defmt::Encoder::new()) }; UP_CHANNELS],
        }
    }

    /// Acquire the defmt encoder.
    fn acquire(&self) {
        let prio = get_priority() as usize;
        if prio >= UP_CHANNELS {
            return;
        }
        // safety: accessing the cell is OK because we are the only one running at this prio
        // and using this channel
        unsafe {
            let encoder: &mut defmt::Encoder = &mut *self.encoders[prio].get();
            encoder.start_frame(|b| {
                _SEGGER_RTT.up_channels[prio].write_all(b);
            });
        }
    }

    /// Write bytes to the defmt encoder.
    ///
    /// # Safety
    ///
    /// Do not call unless you have called `acquire`.
    unsafe fn write(&self, bytes: &[u8]) {
        let prio = get_priority() as usize;
        if prio >= UP_CHANNELS {
            return;
        }
        // safety: accessing the cell is OK because we are the only one running at this prio
        // and using this channel
        unsafe {
            let encoder: &mut defmt::Encoder = &mut *self.encoders[prio].get();
            encoder.write(bytes, |b| {
                _SEGGER_RTT.up_channels[prio].write_all(b);
            });
        }
    }

    /// Flush the encoder
    ///
    /// # Safety
    ///
    /// Do not call unless you have called `acquire`.
    unsafe fn flush(&self) {
        let prio = get_priority() as usize;
        if prio >= UP_CHANNELS {
            return;
        }
        // safety: accessing the cell is OK because we are the only one running at this prio
        // and using this channel
        _SEGGER_RTT.up_channels[prio].flush();
    }

    /// Release the defmt encoder.
    ///
    /// # Safety
    ///
    /// Do not call unless you have called `acquire`. This will release
    /// your lock - do not call `flush` and `write` until you have done another
    /// `acquire`.
    unsafe fn release(&self) {
        let prio = get_priority() as usize;
        if prio >= UP_CHANNELS {
            return;
        }
        // safety: accessing the cell is OK because we are the only one running at this prio
        // and using this channel
        unsafe {
            let encoder: &mut defmt::Encoder = &mut *self.encoders[prio].get();
            encoder.end_frame(|b| {
                _SEGGER_RTT.up_channels[prio].write_all(b);
            });
        }
    }
}

// See https://github.com/rtic-rs/rtic/pull/495#issuecomment-929332903
/// `0`: Thread context
/// `1`: Interrupt with priority 0 aka least urgent
/// `2`: Interrupt with priority 1
/// ...
/// ...
/// `UP_CHANNELS - 2`: Interrupt with priority `UP_CHANNELS - 3` aka most urgent
/// `UP_CHANNELS - 1`: Hardfault
/// `UP_CHANNELS`: Non maskable interrupt
pub(crate) fn get_priority() -> u8 {
    use cortex_m::peripheral::scb::{Exception, SystemHandler, VectActive};
    use cortex_m::peripheral::{NVIC, SCB};

    #[derive(Copy, Clone)]
    struct InterruptNumber(u8);
    unsafe impl cortex_m::interrupt::InterruptNumber for InterruptNumber {
        fn number(self) -> u16 {
            self.0 as u16
        }
    }

    let ipsr: u32;
    unsafe {
        asm!("mrs {}, IPSR", out(reg) ipsr);
    }

    let vect_active = VectActive::from(ipsr as u8).unwrap_or(VectActive::ThreadMode);
    let prio_count = 1 << PRIO_BITS;

    let prio = match vect_active {
        VectActive::ThreadMode => 0,
        VectActive::Exception(ex) => {
            let sysh = match ex {
                #[cfg(not(armv6m))]
                Exception::MemoryManagement => SystemHandler::MemoryManagement,
                #[cfg(not(armv6m))]
                Exception::BusFault => SystemHandler::BusFault,
                #[cfg(not(armv6m))]
                Exception::UsageFault => SystemHandler::UsageFault,
                #[cfg(any(armv8m, native))]
                Exception::SecureFault => SystemHandler::SecureFault,
                Exception::SVCall => SystemHandler::SVCall,
                #[cfg(not(armv6m))]
                Exception::DebugMonitor => SystemHandler::DebugMonitor,
                Exception::PendSV => SystemHandler::PendSV,
                Exception::SysTick => SystemHandler::SysTick,
                Exception::HardFault => return UP_CHANNELS as u8 - 1,
                Exception::NonMaskableInt => return UP_CHANNELS as u8, // sentinel: skip logging
            };
            prio_count - (SCB::get_priority(sysh) >> (8 - PRIO_BITS))
        }
        VectActive::Interrupt { irqn } => {
            prio_count - (NVIC::get_priority(InterruptNumber(irqn - 16)) >> (8 - PRIO_BITS))
        }
    };

    if prio < UP_CHANNELS as u8 - 1 {
        prio
    } else {
        // TODO: Now we disable for the most urgent interrupts if UP_CHANNELS < prio_count + 2
        // does that make sense?
        UP_CHANNELS as u8 // sentinel: skip logging
    }
}

unsafe impl Sync for RttEncoder {}

unsafe impl defmt::Logger for Logger {
    fn acquire() {
        RTT_ENCODER.acquire();
    }

    unsafe fn write(bytes: &[u8]) {
        unsafe {
            RTT_ENCODER.write(bytes);
        }
    }

    unsafe fn flush() {
        unsafe {
            RTT_ENCODER.flush();
        }
    }

    unsafe fn release() {
        unsafe {
            RTT_ENCODER.release();
        }
    }
}

#[repr(C)]
struct Header {
    id: [u8; 16],
    max_up_channels: u32,
    max_down_channels: u32,
    up_channels: [Channel; UP_CHANNELS],
}

unsafe impl Sync for Header {}

struct Buffer {
    inner: UnsafeCell<[u8; BUF_SIZE]>,
}

impl Buffer {
    const fn new() -> Buffer {
        Buffer {
            inner: UnsafeCell::new([0; BUF_SIZE]),
        }
    }

    const fn get(&self) -> *mut u8 {
        self.inner.get() as _
    }
}

unsafe impl Sync for Buffer {}
