//! This crate provide a kernel module toolkit
#![cfg_attr(not(test), no_std)]
// The Writer give print! && println! macros for modules
#![macro_use]
pub mod writer;
pub use writer::WRITER;
pub mod emergency_writer;
pub use emergency_writer::EMERGENCY_WRITER;
pub mod memory;
pub use memory::RustGlobalAlloc;

pub use irq::Irq;
pub use libc_binding::c_char;
pub use log::Record;
pub use messaging::MessageTo;
pub use time::Date;

use core::sync::atomic::AtomicU32;
use core::{fmt, slice, str};
extern crate alloc;
use alloc::vec::Vec;

/// This structure is passed zhen _start point of the module is invoqued
pub struct SymbolList {
    /// Allow to debug the module
    pub write: fn(&str),
    /// Allow to display critical messages for the module
    pub emergency_write: fn(&str),
    /// Allow modules to allocate or free memory
    pub alloc_tools: ForeignAllocMethods,
    /// Specifics methods given by the kernel
    pub kernel_callback: ModConfig,
    /// Kernel Symbol List
    pub kernel_symbol_list: KernelSymbolList,
}

/// Fixed Virtual address of the modules
#[derive(Debug, Copy, Clone)]
#[repr(u32)]
pub enum ModAddress {
    Dummy = 0xE0000000,
    Keyboard = 0xE1000000,
    RTC = 0xE2000000,
    Syslog = 0xE3000000,
}

/// Errors on module
#[derive(Debug, Copy, Clone)]
pub enum ModError {
    BadIdentification,
    DependencyNotSatisfied,
    OutOfMemory,
}

/// Result of a _start request
pub type ModResult = core::result::Result<ModReturn, ModError>;

/// Status of a given module
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Status {
    /// Module is inactive and unloaded
    Inactive,
    /// Module is active and loaded
    Active,
}

/// Default boilerplate for status
impl Default for Status {
    fn default() -> Self {
        Status::Inactive
    }
}

/// Status of all modules
pub struct ModStatus {
    /// Dummy module status
    pub dummy: Status,
    /// RTC module status
    pub rtc: Status,
    /// Keyboard module status
    pub keybard: Status,
    /// Syslog module status
    pub syslog: Status,
}

/// This enum describes kernel functions specifics that the module could call
pub enum ModConfig {
    /// The dummy module need nothing !
    Dummy,
    /// The RTC module have to set an IDT entry
    RTC(RTCConfig),
    /// The Keyboard module have to set an IDT entry and return kerboard activities with a kernel callback
    Keyboard(KeyboardConfig),
    /// The syslog module need nothing !
    Syslog,
}

/// Initialize basics tools of the module
pub unsafe fn init_config(symtab_list: &SymbolList, global_alloc: &mut RustGlobalAlloc) {
    WRITER.set_write_callback(symtab_list.write);
    EMERGENCY_WRITER.set_write_callback(symtab_list.emergency_write);
    #[cfg(not(test))]
    global_alloc.set_methods(symtab_list.alloc_tools);
}

/// Configuration parameters of the RTC module
pub struct RTCConfig {
    /// Give ability to redirect an IDT entry to a specific function
    pub enable_irq: fn(Irq, unsafe extern "C" fn()),
    /// Give ability to disable IDT entry
    pub disable_irq: fn(Irq),
    /// reference of current_unix_time kernel globale
    pub current_unix_time: &'static AtomicU32,
}

/// Configuration parameters of the Keyboard module
pub struct KeyboardConfig {
    /// Give ability to redirect an IDT entry to a specific function
    pub enable_irq: fn(Irq, unsafe extern "C" fn()),
    /// Give ability to disable IDT entry
    pub disable_irq: fn(Irq),
    /// Keyboard callback given by the kernel
    pub callback: fn(MessageTo),
}

/// Standard mod return
pub struct ModReturn {
    /// Stop the module
    pub stop: fn(),
    /// Configurable callback
    pub configurable_callbacks_opt: Option<Vec<ConfigurableCallback>>,
    /// Specific module return
    pub spec: ModSpecificReturn,
}

/// This module describes function specifics that the kernel could call
pub enum ModSpecificReturn {
    /// The kernel cannot ask the Dummy module
    DummyReturn,
    /// The RTC can be stopped and should give the time to the kernel
    RTC(RTCReturn),
    /// The keyboard can be stopped but The kernel cannot ask it
    Keyboard(KeyboardReturn),
    /// The Syslog can be stopped
    Syslog,
}

/// Return parameters of the RTC module
pub struct RTCReturn {
    /// Get the date from the RTC module
    pub read_date: fn() -> Date,
}

/// Return parameters of the Keyboard module
pub struct KeyboardReturn {
    /// Enable to reboot computer with the PS2 controler
    pub reboot_computer: fn(),
}

#[derive(Copy, Clone)]
pub enum KernelEvent {
    /// When a log entry is writed: Expected function prototype is fn(&Record)
    Log,
    /// Call me each second if you can. Expected function prototype is fn()
    Second,
}

/// Unsafe configurable module callback
/// The module asks the kernel to call this function when evt 'when' happened
pub struct ConfigurableCallback {
    /// Type of the Event
    pub when: KernelEvent,
    /// Generique function address
    pub what: u32,
}

/// The allocators methods are passed by the kernel while module is initialized
#[derive(Copy, Clone)]
pub struct ForeignAllocMethods {
    /// KMalloc like a boss
    pub kmalloc: unsafe extern "C" fn(usize) -> *mut u8,
    /// alloc with zeroed
    pub kcalloc: unsafe extern "C" fn(usize, usize) -> *mut u8,
    /// Free Memory
    pub kfree: unsafe extern "C" fn(*mut u8),
    /// Realloc in place if possible
    pub krealloc: unsafe extern "C" fn(*mut u8, usize) -> *mut u8,
}

/// This structure may contains all the kernel symbols list
#[derive(Debug)]
pub struct KernelSymbolList(pub &'static [KernelSymbol]);

/// Main implementation
impl KernelSymbolList {
    // LINKER CANNOT SUPPORT UNDEFINED REFERENCE WHEN LINKING EVEN IF THE
    // FUNCTION `get_primitive_kernel_symbol_list()` IS NOT USED
    // USE `--gc-sections` OPTION TO AVOID THAT PROBLEM
    /// Create a new Kernel symbol List in Rust style
    pub fn new() -> Self {
        let raw_symlist = unsafe { get_primitive_kernel_symbol_list() };
        Self(unsafe { slice::from_raw_parts(raw_symlist.ptr, raw_symlist.len as usize) })
    }

    /// Get a associated address of an entry
    pub fn get_entry(&self, s2: &str) -> Option<u32> {
        for elem in self.0.iter() {
            let s1: &str = unsafe {
                str::from_utf8_unchecked(slice::from_raw_parts(
                    elem.symname as *const u8,
                    elem.len(),
                ))
            };
            if s1 == s2 {
                return Some(elem.address);
            }
        }
        None
    }
}

/// This C item represents a kernel symbol
#[repr(C)]
pub struct KernelSymbol {
    pub address: u32,
    pub symtype: c_char,
    pub symname: *const c_char,
}

/// Kernel Symbol Implementation
impl KernelSymbol {
    #[inline(always)]
    fn len(&self) -> usize {
        let mut len = 0;
        while unsafe { *self.symname.add(len) } != 0 {
            len += 1;
        }
        len
    }
}

/// Debug Boilerplate for a KernelSymbol
impl fmt::Debug for KernelSymbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "addr: {:#X?}, {} {}",
            self.address,
            self.symtype as u8 as char,
            unsafe {
                str::from_utf8_unchecked(slice::from_raw_parts(
                    self.symname as *const u8,
                    self.len(),
                ))
            }
        )
    }
}

/// This C item represents the entire Kernel Symbol List
#[repr(C)]
struct PrimitiveKernelSymbolList {
    len: u32,
    ptr: *const KernelSymbol,
}

// Get the entire symbol List
extern "C" {
    fn get_primitive_kernel_symbol_list() -> PrimitiveKernelSymbolList;
}
