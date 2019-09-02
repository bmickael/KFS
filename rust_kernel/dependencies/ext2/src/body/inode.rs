//! This file describe all the Inode model

use super::Block;
use crate::tools::IoResult;
use bitflags::bitflags;
use core::mem::size_of;

// Like blocks, each inode has a numerical address. It is extremely important to note that unlike block addresses, inode addresses start at 1.

// With Ext2 versions prior to Major version 1, inodes 1 to 10 are reserved and
// should be in an allocated state. Starting with version 1, the first non-reserved inode
// is indicated via a field in the Superblock. Of the reserved inodes, number 2
// subjectively has the most significance as it is used for the root directory.

// Inodes have a fixed size of either 128 for version 0 Ext2 file systems, or as
// dictated by the field in the Superblock for version 1 file systems. All inodes
// reside in inode tables that belong to block groups. Therefore, looking up an
// inode is simply a matter of determining which block group it belongs to and indexing that block group's inode table.

/// Inode Data Structure
#[derive(Debug, Copy, Clone, Default)]
#[repr(C)]
pub struct Inode {
    /// Type and Permissions (see below)
    /*0 	1       2*/
    pub type_and_perm: TypeAndPerm,
    /// User ID
    /*2 	3       2*/
    pub user_id: u16,
    /// Lower 32 bits of size in bytes
    /*4 	7       4*/
    pub low_size: u32,
    /// Last Access Time (in POSIX time)
    /*8 	11      4*/
    pub last_access_time: u32,
    /// Creation Time (in POSIX time)
    /*12 	15      4*/
    pub creation_time: u32,
    /// Last Modification time (in POSIX time)
    /*16 	19      4*/
    pub last_modification_time: u32,
    /// Deletion time (in POSIX time)
    /*20 	23      4*/
    pub deletion_time: u32,
    /// Group ID
    /*24 	25      2*/
    pub group_id: u16,
    /// Count of hard links (directory entries) to this inode. When this reaches 0, the data blocks are marked as unallocated.
    /*26 	27      2*/
    pub nbr_hard_links: u16,
    /// Count of disk sectors (not Ext2 blocks) in use by this inode, not counting the actual inode structure nor directory entries linking to the inode. (iblocks)
    /*28 	31      4*/
    pub nbr_disk_sectors: u32,
    /// Flags (see below)
    /*32 	35      4*/
    pub flags: InodeFlags,
    /// Operating System Specific value #1
    /*36 	39      4*/
    operating_system_specific_value_1: u32,
    /// Direct Block Pointers
    /*40 	43      4*/
    pub direct_block_pointers: [Block; 12],
    /// Singly Indirect Block Pointer (Points to a block that is a list of block pointers to data)
    /*88 	91      4*/
    pub singly_indirect_block_pointers: Block,
    /// Doubly Indirect Block Pointer (Points to a block that is a list of block pointers to Singly Indirect Blocks)
    /*92 	95      4*/
    pub doubly_indirect_block_pointers: Block,
    /// Triply Indirect Block Pointer (Points to a block that is a list of block pointers to Doubly Indirect Blocks)
    /*96 	99      4*/
    pub triply_indirect_block_pointers: Block,
    /// Generation number (Primarily used for NFS)
    /*100 	103 	4*/
    generation_number: u32,
    /// In Ext2 version 0, this field is reserved. In version >= 1, Extended attribute block (File ACL).
    /*104 	107 	4*/
    extended_attribute_block: u32,
    /// In Ext2 version 0, this field is reserved. In version >= 1, Upper 32 bits of file size (if feature bit set) if it's a file, Directory ACL if it's a directory
    /*108 	111 	4*/
    pub upper_size: u32,
    /// Block address of fragment
    /*112 	115 	4*/
    fragment_addr: Block,
    /// Operating System Specific Value #2
    /*116 	127 	12*/
    operating_system_specific_value_2: u32,
}

impl Inode {
    pub fn new(type_and_perm: TypeAndPerm) -> Self {
        Self {
            //TODO: put the true time
            creation_time: 42,
            nbr_hard_links: 1,
            type_and_perm,
            ..Default::default()
        }
    }

    pub fn is_a_directory(&self) -> bool {
        self.type_and_perm.contains(TypeAndPerm::DIRECTORY)
    }

    pub fn is_a_regular_file(&self) -> bool {
        self.type_and_perm.contains(TypeAndPerm::REGULAR_FILE)
    }

    pub fn get_size(&self) -> u64 {
        if self.is_a_directory() {
            self.low_size as u64
        } else {
            self.low_size as u64 + ((self.upper_size as u64) << 32)
        }
    }

    pub fn update_size(&mut self, new_size: u64, block_size: u32) {
        self.low_size = new_size as u32;
        self.upper_size = (new_size >> 32) as u32;

        let block_size = block_size as u64;
        let multiplier = block_size / 512;
        let block_off = if new_size == 0 {
            0
        } else {
            (new_size - 1) / block_size as u64
        };
        let blocknumber_per_block = block_size as u64 / size_of::<Block>() as u64;

        /* Very complex calcul to compute the number of disk_sector use by the data of the inode */
        let block_data = if new_size == 0 {
            0
        } else {
            /* SIMPLE ADDRESSING */
            let mut offset_start = 0;
            let mut offset_end = 12;
            let mut block_data = 0;

            if block_off >= offset_start {
                block_data = (block_off + 1) * multiplier;
            }
            /* SINGLY INDIRECT ADDRESSING */
            offset_start = offset_end;
            offset_end += blocknumber_per_block;
            if block_off >= offset_start {
                block_data += multiplier
            }
            /* DOUBLY INDIRECT ADDRESSING */
            offset_start = offset_end;
            offset_end += blocknumber_per_block * blocknumber_per_block;
            if block_off >= offset_start {
                block_data += multiplier
                    + ((block_off - offset_start) / blocknumber_per_block + 1) * multiplier
            }

            // Triply Indirect Addressing
            offset_start = offset_end;
            //offset_end += blocknumber_per_block * blocknumber_per_block * blocknumber_per_block;
            if block_off >= offset_start {
                block_data += multiplier
                    + (((block_off - offset_start)
                        / (blocknumber_per_block * blocknumber_per_block))
                        + 1)
                        * multiplier
            }
            block_data
        };
        self.nbr_disk_sectors = block_data as u32;
    }
    pub fn unlink(&mut self) -> IoResult<()> {
        unimplemented!()
    }
}

// The type indicator occupies the top hex digit (bits 15 to 12) of this 16-bit field
// Permissions occupy the bottom 12 bits of this 16-bit field
bitflags! {
    #[derive(Default)]
    pub struct TypeAndPerm: u16 {
        const FIFO = 0x1000;
        const CHARACTER_DEVICE = 0x2000;
        const DIRECTORY = 0x4000;
        const BLOCK_DEVICE = 0x6000;
        const REGULAR_FILE = 0x8000;
        const SYMBOLIC_LINK = 0xA000;
        const UNIX_SOCKET = 0xC000;
        const OTHER_EXECUTE_PERMISSION = 0o0001;
        const OTHER_WRITE_PERMISSION = 0o0002;
        const OTHER_READ_PERMISSION = 0o0004;
        const GROUP_EXECUTE_PERMISSION = 0o0010;
        const GROUP_WRITE_PERMISSION = 0o0020;
        const GROUP_READ_PERMISSION = 0o0040;
        const USER_EXECUTE_PERMISSION = 0o0100;
        const USER_WRITE_PERMISSION = 0o0200;
        const USER_READ_PERMISSION = 0o0400;
        const STICKY_BIT = 0o1000;
        const SET_GROUP_ID = 0o2000;
        const SET_USER_ID = 0o4000;
    }
}

// Inode flags
bitflags! {
    #[derive(Default)]
    pub struct InodeFlags: u32 {
        const SECURE_DELETION = 0x00000001;
        const KEEP_A_COPY_OF_DATA_WHEN_DELETED = 0x00000002;
        const FILE_COMPRESSION = 0x00000004;
        const SYNCHRONOUS_UPDATES_NEW_DATA_IS_WRITTEN_IMMEDIATELY_TO_DISK = 0x00000008;
        const IMMUTABLE_FILE = 0x00000010;
        const APPEND_ONLY = 0x00000020;
        const FILE_IS_NOT_INCLUDED_IN_DUMP_COMMAND = 0x00000040;
        const LAST_ACCESSED_TIME_SHOULD_NOT_UPDATED = 0x00000080;
        const HASH_INDEXED_DIRECTORY = 0x00010000;
        const AFS_DIRECTORY = 0x00020000;
        const JOURNAL_FILE_DATA = 0x00040000;
    }
}
