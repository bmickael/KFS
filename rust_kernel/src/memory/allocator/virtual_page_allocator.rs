use super::physical_page_allocator::{AllocFlags, PHYSICAL_ALLOCATOR};
use crate::memory::mmu::invalidate_page;
use crate::memory::mmu::Entry;
use crate::memory::mmu::PageDirectory;
use crate::memory::tools::*;
use crate::memory::BuddyAllocator;
use alloc::boxed::Box;

/// A Physical Allocator must be registered to work
pub struct VirtualPageAllocator {
    pub virt: BuddyAllocator<Virt>,
    pub mmu: Box<PageDirectory>,
}

impl VirtualPageAllocator {
    pub fn new(virt: BuddyAllocator<Virt>, mmu: Box<PageDirectory>) -> Self {
        Self { virt, mmu }
    }

    pub fn reserve(&mut self, vaddr: Page<Virt>, paddr: Page<Phys>, size: NbrPages) -> Result<()> {
        let physical_allocator = unsafe { PHYSICAL_ALLOCATOR.as_mut().unwrap() };

        unsafe {
            self.virt.reserve_exact(vaddr, size.into())?;
            let res = physical_allocator.reserve(paddr, size.into());

            match res {
                Ok(_) => (),
                Err(MemoryError::OutOfBound) => (), //Todo fix this eventually ?
                Err(e) => {
                    self.virt
                        .free_reserve(vaddr, size.into())
                        .expect("Could not free memory reserved on VirtualPageAllocator");
                    return Err(e);
                }
            }
            self.mmu.map_range_page(vaddr, paddr, size, Entry::READ_WRITE | Entry::PRESENT)?;
        }
        Ok(())
    }

    /// Map a ranged physical area and return a virtual address associated
    /// notice: fn(Phys(physical_address_to_map).into(), size.into()) -> Some stuff
    pub fn map_addr(&mut self, paddr: Page<Phys>, size: NbrPages) -> Result<Page<Virt>> {
        let order = size.into();

        // get a new chunk on kernel virtual buddy
        let vaddr = self.virt.alloc(order)?;

        unsafe {
            // map this virtual chunk with the associated physical address
            self.mmu.map_range_page(vaddr, paddr, order.into(), Entry::READ_WRITE | Entry::PRESENT).map_err(|e| {
                self.virt.free(vaddr, order).unwrap();
                e
            })?;
        }
        Ok(vaddr)
    }

    /// UnMap a ranged virtual area
    /// notice: fn(Virt(physical_address_to_map).into()) -> Result
    pub fn unmap_addr(&mut self, vaddr: Page<Virt>, size: NbrPages) -> Result<()> {
        let order = size.into();

        // release the chunk on kernel virtual buddy
        self.virt.free(vaddr, order)?;

        // unmap this vitual chunk
        unsafe { self.mmu.unmap_range_page(vaddr, order.into()) }
    }

    pub fn alloc(&mut self, size: NbrPages) -> Result<Page<Virt>> {
        let order = size.into();
        let vaddr = self.virt.alloc(order)?;
        let physical_allocator = unsafe { PHYSICAL_ALLOCATOR.as_mut().unwrap() };

        unsafe {
            let paddr = physical_allocator.alloc(size, AllocFlags::KERNEL_MEMORY).map_err(|e| {
                self.virt
                    .free(vaddr, order)
                    .expect("Failed to free allocated virtual page after physical allocator failed");
                e
            })?;
            self.mmu.map_range_page(vaddr, paddr, order.into(), Entry::READ_WRITE | Entry::PRESENT).map_err(|e| {
                self.virt.free(vaddr, order).unwrap();
                physical_allocator.free(paddr).unwrap();
                e
            })?;
        }
        Ok(vaddr.into())
    }

    pub fn valloc(&mut self, size: NbrPages) -> Result<Page<Virt>> {
        let order = size.into();
        let vaddr = self.virt.alloc(order)?;

        unsafe {
            self.mmu.map_range_page(vaddr, Page::new(0), order.into(), Entry::READ_WRITE | Entry::VALLOC).map_err(
                |e| {
                    self.virt.free(vaddr, order).expect("Failed to free virtual page after mapping failed");
                    e
                },
            )?;
        }
        Ok(vaddr)
    }

    pub fn valloc_handle_page_fault(&mut self, cr2: u32) -> Result<()> {
        let p = Page::containing(Virt(cr2 as usize));
        let physical_allocator = unsafe { PHYSICAL_ALLOCATOR.as_mut().unwrap() };
        // TODO: remove this unwrap
        let entry = self.mmu.get_entry_mut(p).unwrap();

        if entry.contains(Entry::VALLOC) {
            let paddr = physical_allocator.alloc(NbrPages(1), AllocFlags::KERNEL_MEMORY).map_err(|e| e)?;
            entry.set_entry_page(paddr);
            *entry |= Entry::PRESENT;
            Ok(())
        } else {
            Err(MemoryError::PageFault)
        }
    }

    pub fn ksize(&mut self, vaddr: Page<Virt>) -> Result<NbrPages> {
        Ok(self.virt.ksize(vaddr)?.nbr_pages())
    }

    pub fn free(&mut self, vaddr: Page<Virt>) -> Result<()> {
        let size = self.ksize(vaddr)?;
        let order = size.into();
        let physical_allocator = unsafe { PHYSICAL_ALLOCATOR.as_mut().unwrap() };
        self.virt.free(vaddr, order)?;

        self.mmu.get_entry(vaddr).ok_or(MemoryError::NotPhysicallyMapped).and_then(|entry| {
            if entry.contains(Entry::VALLOC) {
                // Free of Valloced memory
                for virtp in (vaddr..vaddr + size).iter() {
                    let entry = self.mmu.get_entry_mut(virtp).unwrap();
                    if entry.contains(Entry::PRESENT) {
                        physical_allocator.free(entry.entry_page())?;
                        invalidate_page(virtp);
                    }
                    *entry = Default::default();
                }
            } else {
                // Free of Alloced memory
                physical_allocator.free(entry.entry_page())?;
                unsafe { self.mmu.unmap_range_page(vaddr, size)? }
            }
            Ok(())
        })
    }
}

pub static mut KERNEL_VIRTUAL_PAGE_ALLOCATOR: Option<VirtualPageAllocator> = None;
