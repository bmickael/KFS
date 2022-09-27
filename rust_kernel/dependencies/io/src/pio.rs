//! See [Port_IO](https://wiki.osdev.org/Port_IO)
use super::Io;
use core::arch::asm;
use core::marker::PhantomData;

/// This waits one IO cycle.
/// Most likely useless on most modern hardware.
/// Wait one io cycle by outb'ing at unused port (Needs a way to ensure it is unused)
#[no_mangle]
#[inline(always)]
pub extern "C" fn io_wait() {
    unsafe {
        asm!("out dx, al", in("dx") 0x80, in("al") 0x42_u8);
    }
}

/// This is a generic structure to represent IO ports
/// It implements the IO Trait for u8, u16 and u32
#[derive(Debug)]
pub struct Pio<T> {
    port: u16,
    value: PhantomData<T>,
}

impl<T> Pio<T> {
    /// Returns a new Pio assigned to the port `port`
    pub const fn new(port: u16) -> Self {
        Pio {
            port,
            value: PhantomData,
        }
    }
}

impl Io for Pio<u8> {
    type Value = u8;

    fn read(&self) -> Self::Value {
        let result: Self::Value;
        unsafe {
            asm!("in al, dx", in("dx") self.port, out("al") result);
        }
        result
    }

    fn write(&mut self, value: Self::Value) {
        unsafe {
            asm!("out dx, al", in("dx") self.port, in("al") value);
        }
    }
}

impl Io for Pio<u16> {
    type Value = u16;

    fn read(&self) -> Self::Value {
        let result: Self::Value;
        unsafe {
            asm!("in ax, dx", in("dx") self.port, out("ax") result);
        }
        result
    }

    fn write(&mut self, value: Self::Value) {
        unsafe {
            asm!("out dx, ax", in("dx") self.port, in("ax") value);
        }
    }
}

impl Io for Pio<u32> {
    type Value = u32;

    fn read(&self) -> Self::Value {
        let result: Self::Value;
        unsafe {
            asm!("in eax, dx", in("dx") self.port, out("eax") result);
        }
        result
    }

    fn write(&mut self, value: Self::Value) {
        unsafe {
            asm!("out dx, eax", in("dx") self.port, in("eax") value);
        }
    }
}
