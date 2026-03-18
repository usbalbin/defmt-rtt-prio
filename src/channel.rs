use core::{
    ptr,
    sync::atomic::{AtomicU32, Ordering},
};

use crate::{consts::BUF_SIZE, MODE_BLOCK_IF_FULL, MODE_MASK};

/// RTT Up channel
#[repr(C)]
pub(crate) struct Channel {
    /// Name of the channel (null terminated)
    pub name: *const u8,
    /// Pointer to the RTT buffer.
    pub buffer: *mut u8,
    /// Size, in bytes, of the RTT buffer
    pub size: u32,
    /// Written by the target.
    pub write: AtomicU32,
    /// Written by the host.
    pub read: AtomicU32,
    /// Channel properties.
    ///
    /// Currently, only the lowest 2 bits are used to set the channel mode (see constants below).
    pub flags: AtomicU32,
}

impl Channel {
    pub const fn zero() -> Self {
        Self {
            name: ptr::null(),
            buffer: ptr::null_mut(),
            size: 0,
            write: AtomicU32::new(0),
            read: AtomicU32::new(0),
            flags: AtomicU32::new(0),
        }
    }

    pub fn write_all(&self, mut bytes: &[u8]) {
        // the host-connection-status is only modified after RAM initialization while the device is
        // halted, so we only need to check it once before the write-loop
        let write = Self::nonblocking_write;

        while !bytes.is_empty() {
            let consumed = write(self, bytes);
            if consumed != 0 {
                bytes = &bytes[consumed..];
            }
        }
    }

    fn nonblocking_write(&self, bytes: &[u8]) -> usize {
        let write = self.write.load(Ordering::Acquire) as usize;

        // NOTE truncate at BUF_SIZE to avoid more than one "wrap-around" in a single `write` call
        self.write_impl(bytes, write, BUF_SIZE)
    }

    fn write_impl(&self, bytes: &[u8], cursor: usize, available: usize) -> usize {
        let len = bytes.len().min(available);

        // copy `bytes[..len]` to the RTT buffer
        unsafe {
            if cursor + len > BUF_SIZE {
                // split memcpy
                let pivot = BUF_SIZE - cursor;
                ptr::copy_nonoverlapping(bytes.as_ptr(), self.buffer.add(cursor), pivot);
                ptr::copy_nonoverlapping(bytes.as_ptr().add(pivot), self.buffer, len - pivot);
            } else {
                // single memcpy
                ptr::copy_nonoverlapping(bytes.as_ptr(), self.buffer.add(cursor), len);
            }
        }

        // adjust the write pointer, so the host knows that there is new data
        self.write.store(
            (cursor.wrapping_add(len) % BUF_SIZE) as u32,
            Ordering::Release,
        );

        // return the number of bytes written
        len
    }

    pub fn flush(&self) {
        // return early, if host is disconnected
        if !self.host_is_connected() {
            return;
        }

        // busy wait, until the read- catches up with the write-pointer
        let read = || self.read.load(Ordering::Relaxed);
        let write = || self.write.load(Ordering::Relaxed);
        while read() != write() {}
    }

    fn host_is_connected(&self) -> bool {
        // we assume that a host is connected if we are in blocking-mode. this is what probe-run does.
        self.flags.load(Ordering::Relaxed) & MODE_MASK == MODE_BLOCK_IF_FULL
    }
}
