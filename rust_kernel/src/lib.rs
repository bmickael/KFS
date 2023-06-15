#![cfg_attr(not(test), no_std)]
#![cfg_attr(test, allow(unused_imports))]
#![feature(stmt_expr_attributes)]
#![feature(slice_index_methods)]
#![feature(allocator_api)]
#![feature(try_reserve_kind)]
#![feature(alloc_error_handler)]
#![feature(drain_filter)]
#![feature(core_intrinsics)]
#![allow(unstable_name_collisions)] // for FallibleCollection try_new methods

#[macro_use]
extern crate fallible_collections;
#[macro_use]
extern crate interrupts;
#[macro_use]
extern crate terminal;
extern crate alloc;
extern crate arrayvec;
extern crate derive_is_enum_variant;
extern crate io;
extern crate itertools;
extern crate lazy_static;
extern crate mbr;

#[macro_use]
pub mod utils;
#[macro_use]
pub mod debug;
#[macro_use]
pub mod ffi;
#[macro_use]
pub mod system;
#[macro_use]
pub mod drivers;
pub mod elf_loader;
pub mod math;
pub mod memory;
pub mod multiboot;
pub mod panic;
pub mod rust_main;
pub mod taskmaster;
pub mod test_helpers;
pub mod tests;
pub mod watch_dog;

use crate::memory::RustGlobalAlloc;
pub use sync::{Spinlock, SpinlockGuard};
pub use watch_dog::*;

/// As a matter of fact, we can't declare the MemoryManager inside a submodule.
#[cfg(not(test))]
#[global_allocator]
static MEMORY_MANAGER: RustGlobalAlloc = RustGlobalAlloc;
