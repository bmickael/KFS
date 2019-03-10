use crate::memory::tools::*;

pub mod page_directory;
pub use page_directory::PageDirectory;

pub mod page_table;
use page_table::PageTable;

mod page_entry;

pub static mut PAGE_TABLES: [PageTable; 1024] = [PageTable::new(); 1024];

extern "C" {
    pub fn _enable_paging(addr: PhysicalAddr);
    fn _enable_pse();
}