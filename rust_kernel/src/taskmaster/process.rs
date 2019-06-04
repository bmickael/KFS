//! This file contains the process description

mod tss;
use tss::TSS;

use super::SysResult;

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::slice;

use elf_loader::SegmentType;
use errno::Errno;

use fallible_collections::try_vec;

use crate::elf_loader::load_elf;
use crate::memory::mmu::{_enable_paging, _read_cr3};
use crate::memory::tools::{AllocFlags, NbrPages, Page, Virt};
use crate::memory::{VirtualPageAllocator, KERNEL_VIRTUAL_PAGE_ALLOCATOR};
use crate::registers::Eflags;
use crate::system::BaseRegisters;

extern "C" {
    fn _start_process(kernel_esp: u32) -> !;
}

/// Represent all the cpu states of a process according to the TSS context
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct CpuState {
    /// reserved for back trace
    stack_reserved: u32,
    /// current registers
    pub registers: BaseRegisters,
    /// current data DS
    pub ds: u32,
    /// current data ES
    pub es: u32,
    /// current data FS
    pub fs: u32,
    /// current data GS
    pub gs: u32,
    /// current eip
    pub eip: u32,
    /// current CS
    pub cs: u32,
    /// current eflag
    pub eflags: Eflags,
    /// current esp
    pub esp: u32,
    /// current SS
    pub ss: u32,
}

/// Declaration of shared Process trait. Kernel and User processes must implements these methods
pub trait Process {
    /// Return a new process
    unsafe fn new(origin: TaskOrigin) -> SysResult<Box<Self>>;
    /// TSS initialisation method (necessary for ring3 switch)
    unsafe fn init_tss(&self);
    /// Start the process
    unsafe fn start(&self) -> !;
    /// Switch to the current process PD
    unsafe fn context_switch(&self);

    /// Fork the process and return his child
    fn fork(&self, kernel_esp: u32) -> SysResult<Box<Self>>;
}

/// This structure represents an entire process
pub struct UserProcess {
    /// kernel stack
    kernel_stack: Vec<u8>,
    /// Current process ESP on kernel stack
    pub kernel_esp: u32,
    /// Page directory of the process
    pub virtual_allocator: VirtualPageAllocator,
}

/// This structure represents an entire kernel process
pub struct KernelProcess {
    /// kernel stack
    #[allow(dead_code)]
    kernel_stack: Vec<u8>,
    /// Current process ESP on kernel stack
    pub kernel_esp: u32,
}

/// Debug boilerplate for UserProcess
impl core::fmt::Debug for UserProcess {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(
            f,
            "{:#X?} {:#X?} and a kernel_stack",
            unsafe { *(self.kernel_esp as *const CpuState) },
            self.virtual_allocator
        )
    }
}

/// Debug boilerplate for KernelProcess
impl core::fmt::Debug for KernelProcess {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{:#X?} and a kernel_stack", unsafe { *(self.kernel_esp as *const CpuState) })
    }
}

/// This enum describe the origin of the process
#[allow(unused)]
pub enum TaskOrigin {
    /// ELF file
    Elf(&'static [u8]),
    /// Just a dummy function
    Raw(*const u8, usize),
}

/// Main implementation of UserProcess
impl UserProcess {
    const RING0_STACK_SEGMENT: u16 = 0x18;

    const RING3_CODE_SEGMENT: u32 = 0x20;
    const RING3_DATA_SEGMENT: u32 = 0x28;
    const RING3_STACK_SEGMENT: u32 = 0x30;
    const RING3_DPL: u32 = 0b11;

    const RING3_RAW_PROCESS_MAX_SIZE: NbrPages = NbrPages::_64K;
    const RING3_PROCESS_STACK_SIZE: NbrPages = NbrPages::_64K;
    const RING3_PROCESS_KERNEL_STACK_SIZE: NbrPages = NbrPages::_64K;
}

/// Main implementation of KernalProcess
impl KernelProcess {
    const RING0_CODE_SEGMENT: u32 = 0x08;
    const RING0_DATA_SEGMENT: u32 = 0x10;
    const RING0_STACK_SEGMENT: u32 = 0x18;
    const RING0_DPL: u32 = 0b0;

    const KERNEL_RAW_PROCESS_MAX_SIZE: NbrPages = NbrPages::_64K;
    const KERNEL_PROCESS_STACK_SIZE: NbrPages = NbrPages::_64K;
}

/// Main implementation of process trait for UserProcess
impl Process for UserProcess {
    unsafe fn new(origin: TaskOrigin) -> SysResult<Box<Self>> {
        // Store kernel CR3
        let old_cr3 = _read_cr3();
        // Create the process Page directory
        let mut virtual_allocator = VirtualPageAllocator::new_for_process().map_err(|_| Errno::Enomem)?;
        // Switch to this process Page Directory
        virtual_allocator.context_switch();

        let eip = match origin {
            TaskOrigin::Elf(content) => {
                // Parse Elf and generate stuff
                let elf = load_elf(content);
                for h in &elf.program_header_table {
                    if h.segment_type == SegmentType::Load {
                        let segment = {
                            virtual_allocator
                                .alloc_on(
                                    Page::containing(Virt(h.vaddr as usize)),
                                    (h.memsz as usize).into(),
                                    AllocFlags::USER_MEMORY,
                                )
                                .map_err(|_| Errno::Enomem)?
                                .to_addr()
                                .0 as *mut u8;
                            slice::from_raw_parts_mut(h.vaddr as usize as *mut u8, h.memsz as usize)
                        };
                        segment[0..h.filez as usize]
                            .copy_from_slice(&content[h.offset as usize..h.offset as usize + h.filez as usize]);
                        // With BSS (so a NOBITS section), the memsz value exceed the filesz. Setting next bytes as 0
                        segment[h.filez as usize..h.memsz as usize]
                            .as_mut_ptr()
                            .write_bytes(0, h.memsz as usize - h.filez as usize);
                        // Modify the rights on pages by following the ELF specific restrictions
                        virtual_allocator.modify_range_page_entry(
                            Page::containing(Virt(h.vaddr as usize)),
                            (h.memsz as usize).into(),
                            Into::<AllocFlags>::into(h.flags) | AllocFlags::USER_MEMORY,
                        );
                    }
                }
                elf.header.entry_point as u32
            }
            TaskOrigin::Raw(code, code_len) => {
                // Allocate one page for code segment of the Dummy process
                let base_addr = virtual_allocator
                    .alloc(Self::RING3_RAW_PROCESS_MAX_SIZE, AllocFlags::USER_MEMORY)
                    .map_err(|_| Errno::Enomem)?
                    .to_addr()
                    .0 as *mut u8;
                // Copy the code segment
                base_addr.copy_from(code, code_len);
                base_addr as u32
            }
        };

        // Allocate the kernel stack of the process
        let kernel_stack = try_vec![0; Self::RING3_PROCESS_KERNEL_STACK_SIZE.into()]?;
        assert!(kernel_stack.as_ptr() as usize & (Self::RING3_PROCESS_KERNEL_STACK_SIZE.to_bytes() - 1) == 0);

        // Mark the first entry of the kernel stack as read-only, its make an Triple fault when happened
        virtual_allocator.modify_page_entry(
            Virt(kernel_stack.as_ptr() as usize).into(),
            AllocFlags::READ_ONLY | AllocFlags::KERNEL_MEMORY,
        );

        // Generate the start kernel ESP of the new process
        let kernel_esp = kernel_stack
            .as_ptr()
            .add(Into::<usize>::into(Self::RING3_PROCESS_KERNEL_STACK_SIZE) - core::mem::size_of::<CpuState>())
            as u32;

        // Allocate one page for stack segment of the process
        let stack_addr = virtual_allocator
            .alloc(Self::RING3_PROCESS_STACK_SIZE, AllocFlags::USER_MEMORY)
            .map_err(|_| Errno::Enomem)?
            .to_addr()
            .0 as *mut u8;

        // Mark the first entry of the user stack as read-only, this prevent user stack overflow
        virtual_allocator
            .modify_page_entry(Virt(stack_addr as usize).into(), AllocFlags::READ_ONLY | AllocFlags::USER_MEMORY);

        // stack go downwards set esp to the end of the allocation
        let esp = stack_addr.add(Self::RING3_PROCESS_STACK_SIZE.into()) as u32;

        // Create the process identity
        let cpu_state: CpuState = CpuState {
            stack_reserved: 0,
            registers: BaseRegisters { esp, ..Default::default() }, // Be carefull, never trust ESP
            ds: Self::RING3_DATA_SEGMENT + Self::RING3_DPL,
            es: Self::RING3_DATA_SEGMENT + Self::RING3_DPL,
            fs: Self::RING3_DATA_SEGMENT + Self::RING3_DPL,
            gs: Self::RING3_DATA_SEGMENT + Self::RING3_DPL,
            eip,
            cs: Self::RING3_CODE_SEGMENT + Self::RING3_DPL,
            eflags: Eflags::get_eflags().set_interrupt_flag(true),
            esp,
            ss: Self::RING3_STACK_SEGMENT + Self::RING3_DPL,
        };
        // Fill the kernel stack of the new process with start cpu states.
        (kernel_esp as *mut u8).copy_from(&cpu_state as *const _ as *const u8, core::mem::size_of::<CpuState>());

        // Re-enable kernel virtual space memory
        _enable_paging(old_cr3);

        Ok(Box::new(UserProcess { kernel_stack, kernel_esp, virtual_allocator }))
    }

    unsafe fn init_tss(&self) {
        TSS.lock().init(
            self.kernel_stack.as_ptr().add(Self::RING3_PROCESS_KERNEL_STACK_SIZE.into()) as u32,
            Self::RING0_STACK_SEGMENT,
        );
    }

    unsafe fn context_switch(&self) {
        // Switch to the new process PD
        self.virtual_allocator.context_switch();
        // Re-init the TSS block for the new process
        self.init_tss();
    }

    unsafe fn start(&self) -> ! {
        // Switch to process Page Directory
        self.virtual_allocator.context_switch();

        // Init the TSS segment
        self.init_tss();

        // Launch the ring3 process on its own kernel stack
        _start_process(self.kernel_esp)
    }

    fn fork(&self, kernel_esp: u32) -> SysResult<Box<Self>> {
        // Create the child kernel stack
        let mut child_kernel_stack = try_vec![0; Self::RING3_PROCESS_KERNEL_STACK_SIZE.into()]?;
        assert!(child_kernel_stack.as_ptr() as usize & (Self::RING3_PROCESS_KERNEL_STACK_SIZE.to_bytes() - 1) == 0);
        child_kernel_stack.as_mut_slice().copy_from_slice(self.kernel_stack.as_slice());

        // Set the kernel ESP of the child. Relative to kernel ESP of the father
        let child_kernel_esp = kernel_esp - self.kernel_stack.as_ptr() as u32 + child_kernel_stack.as_ptr() as u32;

        // Mark child syscall return as 0
        let child_cpu_state: *mut CpuState = child_kernel_esp as *mut CpuState;
        unsafe {
            (*child_cpu_state).registers.eax = 0;
        }

        Ok(Box::new(Self {
            kernel_stack: child_kernel_stack,
            kernel_esp: child_kernel_esp,
            virtual_allocator: self.virtual_allocator.fork().map_err(|_| Errno::Enomem)?,
        }))
    }
}

// Note: It is very really tricky to exit from a kernel process about possibles memory leaks or code corruption
// The using of Syscalls from a kernel process can lead to a lot of undefined behavior. please avoid it
// Maybe we need to check CS of the caller of the TSS segment in the syscall handler to unallow use of them.
/// Main implementation of a KernelProcess
impl Process for KernelProcess {
    unsafe fn new(origin: TaskOrigin) -> SysResult<Box<Self>> {
        let eip = match origin {
            TaskOrigin::Elf(_content) => {
                unimplemented!();
            }
            TaskOrigin::Raw(code, code_len) => {
                // Allocate a chunk of memory for the process code
                let base_addr = KERNEL_VIRTUAL_PAGE_ALLOCATOR
                    .as_mut()
                    .unwrap()
                    .alloc(Self::KERNEL_RAW_PROCESS_MAX_SIZE, AllocFlags::KERNEL_MEMORY)
                    .map_err(|_| Errno::Enomem)?
                    .to_addr()
                    .0 as *mut u8;
                // Copy the code segment
                base_addr.copy_from(code, code_len);
                base_addr as u32
            }
        };

        // Allocate the kernel stack of the process
        let kernel_stack = try_vec![0; Self::KERNEL_PROCESS_STACK_SIZE.into()]?;
        assert!(kernel_stack.as_ptr() as usize & (Self::KERNEL_PROCESS_STACK_SIZE.to_bytes() - 1) == 0);

        // Mark the first entry of the kernel stack as read-only, its make an Triple fault when happened
        KERNEL_VIRTUAL_PAGE_ALLOCATOR.as_mut().unwrap().modify_page_entry(
            Virt(kernel_stack.as_ptr() as usize).into(),
            AllocFlags::READ_ONLY | AllocFlags::KERNEL_MEMORY,
        );

        // Generate the start kernel ESP of the new process
        let kernel_esp = kernel_stack
            .as_ptr()
            .add(Into::<usize>::into(Self::KERNEL_PROCESS_STACK_SIZE) - core::mem::size_of::<CpuState>())
            as u32;

        // Create the process identity
        let cpu_state: CpuState = CpuState {
            stack_reserved: 0,
            registers: BaseRegisters { esp: kernel_esp, ..Default::default() }, // Be carefull, never trust ESP
            ds: Self::RING0_DATA_SEGMENT + Self::RING0_DPL,
            es: Self::RING0_DATA_SEGMENT + Self::RING0_DPL,
            fs: Self::RING0_DATA_SEGMENT + Self::RING0_DPL,
            gs: Self::RING0_DATA_SEGMENT + Self::RING0_DPL,
            eip,
            cs: Self::RING0_CODE_SEGMENT + Self::RING0_DPL,
            eflags: Eflags::get_eflags().set_interrupt_flag(true),
            esp: kernel_esp,
            ss: Self::RING0_STACK_SEGMENT + Self::RING0_DPL,
        };
        // Fill the kernel stack of the new process with start cpu states.
        (kernel_esp as *mut u8).copy_from(&cpu_state as *const _ as *const u8, core::mem::size_of::<CpuState>());

        Ok(Box::new(KernelProcess { kernel_stack, kernel_esp }))
    }

    unsafe fn init_tss(&self) {
        // Initialize the TSS segment is not necessary for a kernel process
    }

    unsafe fn start(&self) -> ! {
        // Launch the kerne; process on its own kernel stack
        _start_process(self.kernel_esp)
    }

    unsafe fn context_switch(&self) {
        // Context_switch is not necessary for a kernel process
    }

    // Note: Forking from a kernel process seams not to be a good idea
    fn fork(&self, _kernel_esp: u32) -> SysResult<Box<Self>> {
        unimplemented!();
    }
}
