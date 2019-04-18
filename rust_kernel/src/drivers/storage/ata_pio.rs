//! This files contains the code related to the ATA / IDE CONTROLER
/// See https://wiki.osdev.org/ATA_PIO_Mode
#[deny(missing_docs)]
use io::{Io, Pio};

use bit_field::BitField;
use bitflags::bitflags;

use alloc::vec::Vec;

use core::slice;

/// Global structure
#[derive(Debug, Copy, Clone, Default)]
pub struct AtaPio {
    primary_master: Option<Drive>,
    secondary_master: Option<Drive>,
    primary_slave: Option<Drive>,
    secondary_slave: Option<Drive>,

    selected_drive: Option<Rank>,
}

/// Global disk characteristics
#[derive(Debug, Copy, Clone)]
struct Drive {
    command_register: u16,
    control_register: u16,
    capabilities: Capabilities,
    sector_capacity: NbrSectors,
    udma_support: u16,
    rank: Rank,
}

/// AtaResult is just made to handle module errors
pub type AtaResult<T> = core::result::Result<T, AtaError>;

/// Common errors for this module
#[derive(Debug, Copy, Clone)]
pub enum AtaError {
    /// Not a valid position
    DeviceNotFound,
    /// Common error Variant
    NotSupported,
    /// Out of bound like always
    OutOfBound,
    /// There is nothing to do
    NothingToDo,
    /// IO error
    IoError,
}

/// Rank
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Rank {
    Primary(Hierarchy),
    Secondary(Hierarchy),
}

/// Is it a Slave or a Master ?
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Hierarchy {
    Master,
    Slave,
}

/// new type representing a number of sectors
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct NbrSectors(pub u64);

impl Into<usize> for NbrSectors {
    fn into(self) -> usize {
        self.0 as usize * 512
    }
}

impl From<usize> for NbrSectors {
    fn from(u: usize) -> Self {
        Self((u / 512 + if u % 512 != 0 { 1 } else { 0 }) as u64)
    }
}

use core::ops::Add;

/// Add  boilerplate for Sector + NbrSectors
impl Add<NbrSectors> for Sector {
    type Output = Sector;

    fn add(self, other: NbrSectors) -> Self::Output {
        Self(self.0 + other.0)
    }
}

/// new type representing the start sector
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct Sector(pub u64);

/// Disk access capabilities
#[derive(Debug, Copy, Clone)]
enum Capabilities {
    Lba48,
    Lba28,
    Chs,
}

// Some errors may occured
bitflags! {
    struct ErrorRegister: u8 {
        const ADDRESS_MARK_NOT_FOUND = 1 << 0;
        const TRACK_ZERO_NOT_FOUND = 1 << 1;
        const ABORTED_COMMAND = 1 << 2;
        const MEDIA_CHANGE_REQUEST = 1 << 3;
        const ID_MOT_FOUND = 1 << 4;
        const MEDIA_CHANGED = 1 << 5;
        const UNCORRECTABLE_DATA_ERROR = 1 << 6;
        const BAD_BLOCK_DETECTED = 1 << 7;
    }
}

// We need always check status register
bitflags! {
    struct StatusRegister: u8 {
        const ERR = 1 << 0; // Indicates an error occurred. Send a new command to clear it (or nuke it with a Software Reset).
        const IDX = 1 << 1; // Index. Always set to zero.
        const CORR = 1 << 2; // Corrected data. Always set to zero.
        const DRQ = 1 << 3; // Set when the drive has PIO data to transfer, or is ready to accept PIO data.
        const SRV = 1 << 4; // Overlapped Mode Service Request.
        const DF = 1 << 5; // Drive Fault Error (does not set ERR).
        const RDY = 1 << 6; // Bit is clear when drive is spun down, or after an error. Set otherwise.
        const BSY = 1 << 7; //Indicates the drive is preparing to send/receive data (wait for it to clear). In case of 'hang' (it never clears), do a software reset
    }
}

// Necessary to set some advanced features
bitflags! {
    struct DeviceControlRegister: u8 {
        const NIEN = 1 << 1; // Set this to stop the current device from sending interrupts.
        const SRST = 1 << 2; // Set, then clear (after 5us), this to do a "Software Reset" on all ATA drives on a bus, if one is misbehaving.
        const HOB = 1 << 7; // Set this to read back the High Order Byte of the last LBA48 value sent to an IO port.
    }
}

/// 0x01F0-0x01F7 The primary ATA hard-disk controller. 0x03F6-0x03F7 The control register, pop on IRQ14,
/// 0x0170-0x0177 The secondary ATA hard-disk controller. 0x0376-0x0377 The control register, pop on IRQ15
impl Drive {
    /// *** These below constants are expressed with offset from base register ***
    /// Data Register: Read/Write PIO data bytes. (read/write) (16-bit / 16-bit)
    const DATA: u16 = 0x0;

    /// Error Register: Used to retrieve any error generated by the last ATA command executed. (read) (8-bit / 16-bit)
    /// Features Register: Used to control command specific interface features. (write) (8-bit / 16-bit)
    const ERROR: u16 = 0x1;
    const _FEATURES: u16 = 0x1;

    /// Sector Count Register:  Number of sectors to read/write (0 is a special value). (read/write) (8-bit / 16-bit)
    const SECTOR_COUNT: u16 = 0x2;

    /// Sector Number Register or LBA low. (read/write) (8-bit / 16-bit)
    const L1_SECTOR: u16 = 0x3;

    /// Cylinder Low Register or LBA mid. (read/write) (8-bit / 16-bit)
    const L2_CYLINDER: u16 = 0x4;

    /// Cylinder High Register or LBA high. (read/write) (8-bit / 16-bit)
    const L3_CYLINDER: u16 = 0x5;

    /// Drive / Head Register: Used to select a drive and/or head. Supports extra address/flag bits. (read/write) (8-bit / 8-bit)
    const SELECTOR: u16 = 0x6;

    /// Status Register: Used to read the current status. (read) (8-bit / 8-bit)
    /// Command Register:  Used to send ATA commands to the device. (write) (8-bit / 8-bit)
    const STATUS: u16 = 0x7;
    const COMMAND: u16 = 0x7;

    /// *** These below constants are expressed with offset from control register ***
    /// A duplicate of the Status Register which does not affect interrupts. (read) (8-bit / 8-bit)
    /// Used to reset the bus or enable/disable interrupts. (write) (8-bit / 8-bit)
    const ALTERNATE_STATUS: u16 = 0x0;
    const DEVICE_CONTROL: u16 = 0x0;

    /// Provides drive select and head select information. (read) (8-bit / 8-bit)
    const _DRIVE_ADDRESS: u16 = 0x1;

    /// Check if the selected IDE device is present, return characteristics if it is
    fn identify(rank: Rank, command_register: u16, control_register: u16) -> Option<Drive> {
        let target = match rank {
            Rank::Primary(Hierarchy::Master) => 0xA0,
            Rank::Primary(Hierarchy::Slave) => 0xB0,
            Rank::Secondary(Hierarchy::Master) => 0xA0,
            Rank::Secondary(Hierarchy::Slave) => 0xB0,
        };

        // select a target drive by sending 0xA0 for the master drive, or 0xB0 for the slave
        Pio::<u8>::new(command_register + Self::SELECTOR).write(target);

        // set the Sectorcount, LBAlo, LBAmid, and LBAhi IO ports to 0
        Pio::<u8>::new(command_register + Self::SECTOR_COUNT).write(0);
        Pio::<u8>::new(command_register + Self::L1_SECTOR).write(0);
        Pio::<u8>::new(command_register + Self::L2_CYLINDER).write(0);
        Pio::<u8>::new(command_register + Self::L3_CYLINDER).write(0);

        // send the IDENTIFY command (0xEC) to the Command IO port (0x1F7)
        Pio::<u8>::new(command_register + Self::COMMAND).write(0xEC);

        // read the Status port (0x1F7). If the value read is 0, the drive does not exist
        if Pio::<u8>::new(command_register + Self::STATUS).read() == 0 {
            return None;
        }

        // For any other value: poll the Status port (0x1F7) until bit 7 (BSY, value = 0x80) clears
        while (StatusRegister::from_bits_truncate(Pio::<u8>::new(command_register + Self::STATUS).read()))
            .contains(StatusRegister::BSY)
        {}

        // Continue polling one of the Status ports until bit 3 (DRQ, value = 8) sets, or until bit 0 (ERR, value = 1) sets.
        while !(StatusRegister::from_bits_truncate(Pio::<u8>::new(command_register + Self::STATUS).read()))
            .intersects(StatusRegister::ERR | StatusRegister::DRQ)
        {}

        // If ERR is set, it is a failure
        if (StatusRegister::from_bits_truncate(Pio::<u8>::new(command_register + Self::STATUS).read()))
            .contains(StatusRegister::ERR)
        {
            eprintln!(
                "unexpected error while polling status of {:?} err: {:?}",
                rank,
                ErrorRegister::from_bits_truncate(Pio::<u8>::new(command_register + Self::ERROR).read())
            );
            return None;
        }

        // if ERR is clear, the data is ready to read from the Data port (0x1F0). Read 256 16-bit values, and store them.
        let mut v = Vec::new();

        for _i in 0..256 {
            v.push(Pio::<u16>::new(command_register + Self::DATA).read());
        }

        // Bit 10 is set if the drive supports LBA48 mode.
        // 100 through 103 taken as a uint64_t contain the total number of 48 bit addressable sectors on the drive. (Probably also proof that LBA48 is supported.)
        if v[83] & (1 << 10) != 0 {
            Some(Drive {
                capabilities: Capabilities::Lba48,
                sector_capacity: NbrSectors(
                    v[100] as u64 + ((v[101] as u64) << 16) + ((v[102] as u64) << 32) + ((v[103] as u64) << 48),
                ),
                // The bits in the low byte tell you the supported UDMA modes, the upper byte tells you which UDMA mode is active.
                udma_support: v[88],
                command_register,
                control_register,
                rank,
            })
        // 60 & 61 taken as a uint32_t contain the total number of 28 bit LBA addressable sectors on the drive. (If non-zero, the drive supports LBA28.)
        } else if v[60] != 0 || v[61] != 0 {
            Some(Drive {
                capabilities: Capabilities::Lba28,
                sector_capacity: NbrSectors(v[60] as u64 + ((v[61] as u64) << 16)),
                udma_support: v[88],
                command_register,
                control_register,
                rank,
            })
        } else {
            Some(Drive {
                capabilities: Capabilities::Chs,
                sector_capacity: NbrSectors(0),
                udma_support: v[88],
                command_register,
                control_register,
                rank,
            })
        }
    }

    /// drive specific READ method
    fn read(&self, start_sector: Sector, nbr_sectors: NbrSectors, buf: *mut u8) -> AtaResult<()> {
        check_bounds(start_sector, nbr_sectors, self.sector_capacity)?;

        let s = unsafe { slice::from_raw_parts_mut(buf, nbr_sectors.into()) };

        match self.capabilities {
            Capabilities::Lba48 => {
                // Do disk operation for each 'chunk_size' bytes
                const CHUNK_SIZE: usize = 1024;

                for (i, chunk) in s.chunks_mut(CHUNK_SIZE).enumerate() {
                    let sectors_to_read = chunk.len().into();

                    self.init_lba48(start_sector + (i * CHUNK_SIZE).into(), sectors_to_read);

                    // Send the "READ SECTORS EXT" command (0x24) to port 0x1F7: outb(0x1F7, 0x24)
                    self.wait_available();
                    Pio::<u8>::new(self.command_register + Self::COMMAND).write(0x24);

                    // Read n sectors and put them into buf
                    self.read_sectors(sectors_to_read, chunk.as_mut_ptr())?;
                }
                Ok(())
            }
            // I experiment a lack of documentation about this mode
            Capabilities::Lba28 => Err(AtaError::NotSupported),
            // I experiment a lack of documentation about this mode
            Capabilities::Chs => Err(AtaError::NotSupported),
        }
    }

    /// Drive specific WRITE method
    fn write(&self, start_sector: Sector, nbr_sectors: NbrSectors, buf: *const u8) -> AtaResult<()> {
        check_bounds(start_sector, nbr_sectors, self.sector_capacity)?;

        let s = unsafe { slice::from_raw_parts(buf, nbr_sectors.into()) };

        match self.capabilities {
            Capabilities::Lba48 => {
                // Do disk operation for each 'chunk_size' bytes
                const CHUNK_SIZE: usize = 1024;

                for (i, chunk) in s.chunks(CHUNK_SIZE).enumerate() {
                    let sectors_to_write = chunk.len().into();

                    self.init_lba48(start_sector + (i * CHUNK_SIZE).into(), sectors_to_write);

                    // Send the "WRITE SECTORS EXT" command (0x34) to port 0x1F7: outb(0x1F7, 0x24)
                    self.wait_available();
                    Pio::<u8>::new(self.command_register + Self::COMMAND).write(0x34);

                    // Write n sectors from buf to disk
                    self.write_sectors(sectors_to_write, chunk.as_ptr())?;

                    // Fflush write cache
                    self.fflush_write_cache();
                }
                Ok(())
            }
            // I experiment a lack of documentation about this mode
            Capabilities::Lba28 => Err(AtaError::NotSupported),
            // I experiment a lack of documentation about this mode
            Capabilities::Chs => Err(AtaError::NotSupported),
        }
    }

    /// Read n_sectors, store them into buf
    fn read_sectors(&self, nbr_sectors: NbrSectors, buf: *const u8) -> AtaResult<()> {
        for sector in 0..nbr_sectors.0 as usize {
            // Wait for end of Busy state and DRQ ready
            self.busy_wait()?;

            let p = buf as *mut u16;
            for i in 0..256 {
                unsafe { *p.add(i + sector * 256) = Pio::<u16>::new(self.command_register + Self::DATA).read() }
            }
        }
        Ok(())
    }

    /// Write n sectors from buf
    fn write_sectors(&self, nbr_sectors: NbrSectors, buf: *const u8) -> AtaResult<()> {
        for sector in 0..nbr_sectors.0 as usize {
            // Wait for end of Busy state and DRQ ready
            self.busy_wait()?;

            let p = buf as *const u16;
            for i in 0..256 {
                unsafe { Pio::<u16>::new(self.command_register + Self::DATA).write(*p.add(i + sector * 256)) }
            }
        }
        Ok(())
    }

    /// Wait for end of Busy state and DRQ ready
    fn busy_wait(&self) -> AtaResult<()> {
        loop {
            let r = StatusRegister::from_bits_truncate(Pio::<u8>::new(self.command_register + Self::STATUS).read());
            if r.contains(StatusRegister::ERR) {
                eprintln!(
                    "unexpected error while busy of {:?} err: {:?}",
                    self.rank,
                    ErrorRegister::from_bits_truncate(Pio::<u8>::new(self.command_register + Self::ERROR).read())
                );
                return Err(AtaError::IoError);
            }
            if !r.contains(StatusRegister::BSY) && r.contains(StatusRegister::DRQ) {
                break;
            }
        }
        Ok(())
    }

    /// On some drives it is necessary to "manually" flush the hardware write cache after every write command.
    /// This is done by sending the 0xE7 command to the Command Register (then waiting for BSY to clear).
    /// If a driver does not do this, then subsequent write commands can fail invisibly,
    /// or "temporary bad sectors" can be created on your disk.
    fn fflush_write_cache(&self) {
        Pio::<u8>::new(self.command_register + Self::COMMAND).write(0xE7);

        let p = Pio::<u8>::new(self.command_register + Self::STATUS);
        while StatusRegister::from_bits_truncate(p.read()).contains(StatusRegister::BSY) {}
    }

    /// The method suggested in the ATA specs for sending ATA commands tells you to check the BSY and DRQ bits before trying to send a command
    fn wait_available(&self) {
        // Continue polling one of the Status ports until bit 3 (DRQ, value = 8) sets, or until bit 0 (BSY, value = 7) sets.
        while StatusRegister::from_bits_truncate(Pio::<u8>::new(self.control_register + Self::ALTERNATE_STATUS).read())
            .intersects(StatusRegister::BSY | StatusRegister::DRQ)
        {}
    }

    /// Init read or write sequence for lba48 mode
    fn init_lba48(&self, start_sector: Sector, nbr_sectors: NbrSectors) {
        let lba_low = start_sector.0.get_bits(0..32) as u32;
        let lba_high = start_sector.0.get_bits(32..48) as u32;

        // Send 0x40 for the "master" or 0x50 for the "slave" to port 0x1F6: outb(0x1F6, 0x40 | (slavebit << 4))
        self.wait_available();
        match self.get_hierarchy() {
            Hierarchy::Master => Pio::<u8>::new(self.command_register + Self::SELECTOR).write(0x40),
            Hierarchy::Slave => Pio::<u8>::new(self.command_register + Self::SELECTOR).write(0x50),
        }

        // Outb (0x1F2, sectorcount high byte)
        Pio::<u8>::new(self.command_register + Self::SECTOR_COUNT).write(nbr_sectors.0.get_bits(8..16) as u8);

        // LBA 4
        Pio::<u8>::new(self.command_register + Self::L1_SECTOR).write(lba_low.get_bits(24..32) as u8);
        // LBA 5
        Pio::<u8>::new(self.command_register + Self::L2_CYLINDER).write(lba_high.get_bits(0..8) as u8);
        // LBA 6
        Pio::<u8>::new(self.command_register + Self::L3_CYLINDER).write(lba_high.get_bits(8..16) as u8);

        // outb (0x1F2, sectorcount low byte)
        Pio::<u8>::new(self.command_register + Self::SECTOR_COUNT).write(nbr_sectors.0.get_bits(0..8) as u8);

        // LBA 1
        Pio::<u8>::new(self.command_register + Self::L1_SECTOR).write(lba_low.get_bits(0..8) as u8);
        // LBA 2
        Pio::<u8>::new(self.command_register + Self::L2_CYLINDER).write(lba_low.get_bits(8..16) as u8);
        // LBA 3
        Pio::<u8>::new(self.command_register + Self::L3_CYLINDER).write(lba_low.get_bits(16..24) as u8);
    }

    /// Extract the sub tag hierarchy from rank
    fn get_hierarchy(&self) -> Hierarchy {
        match self.rank {
            Rank::Primary(h) | Rank::Secondary(h) => h,
        }
    }

    /// Select the drive for future read and write operations
    fn select_drive(&self) {
        self.wait_available();
        match self.get_hierarchy() {
            // select a target drive by sending 0xA0 for the master drive, or 0xB0 for the slave
            // I dont think it is necessary or really true
            Hierarchy::Master => Pio::<u8>::new(self.command_register + Self::SELECTOR).write(0xA0),
            Hierarchy::Slave => Pio::<u8>::new(self.command_register + Self::SELECTOR).write(0xB0),
        };
        // Disable interruot bit for the selected drive
        Pio::<u8>::new(self.control_register + Self::DEVICE_CONTROL).write(DeviceControlRegister::NIEN.bits());
    }
}

/// Standard port location, if they are different, probe IDE controller in PCI driver
const PRIMARY_BASE_REGISTER: u16 = 0x01F0;
const SECONDARY_BASE_REGISTER: u16 = 0x0170;
const PRIMARY_CONTROL_REGISTER: u16 = 0x03f6;
const SECONDARY_CONTROL_REGISTER: u16 = 0x376;

impl AtaPio {
    /// Invocation of a new AtaPio-IDE controller
    pub fn new() -> Self {
        Self {
            primary_master: Drive::identify(
                Rank::Primary(Hierarchy::Master),
                PRIMARY_BASE_REGISTER,
                PRIMARY_CONTROL_REGISTER,
            ),
            secondary_master: Drive::identify(
                Rank::Primary(Hierarchy::Master),
                SECONDARY_BASE_REGISTER,
                SECONDARY_CONTROL_REGISTER,
            ),
            primary_slave: Drive::identify(
                Rank::Primary(Hierarchy::Slave),
                PRIMARY_BASE_REGISTER,
                PRIMARY_CONTROL_REGISTER,
            ),
            secondary_slave: Drive::identify(
                Rank::Primary(Hierarchy::Slave),
                SECONDARY_BASE_REGISTER,
                SECONDARY_CONTROL_REGISTER,
            ),
            selected_drive: None,
        }
    }

    /// Select the drive we would like to read or write
    pub fn select_drive(&mut self, rank: Rank) -> AtaResult<()> {
        self.selected_drive = match rank {
            Rank::Primary(Hierarchy::Master) if self.primary_master.is_some() => Some(rank),
            Rank::Primary(Hierarchy::Slave) if self.primary_slave.is_some() => Some(rank),
            Rank::Secondary(Hierarchy::Master) if self.secondary_master.is_some() => Some(rank),
            Rank::Secondary(Hierarchy::Slave) if self.secondary_slave.is_some() => Some(rank),
            _ => None,
        };
        self.get_selected_drive().ok_or(AtaError::DeviceNotFound)?.select_drive();
        Ok(())
    }

    /// Read nbr_sectors after start_sector location and write it into the buf
    pub fn read(&self, start_sector: Sector, nbr_sectors: NbrSectors, buf: *mut u8) -> AtaResult<()> {
        self.get_selected_drive().ok_or(AtaError::DeviceNotFound).and_then(|d| d.read(start_sector, nbr_sectors, buf))
    }

    /// Write nbr_sectors after start_sector location from the buf
    pub fn write(&self, start_sector: Sector, nbr_sectors: NbrSectors, buf: *const u8) -> AtaResult<()> {
        self.get_selected_drive().ok_or(AtaError::DeviceNotFound).and_then(|d| d.write(start_sector, nbr_sectors, buf))
    }

    /// Get the drive pointed by Rank, or else return None
    fn get_selected_drive(&self) -> Option<&Drive> {
        match self.selected_drive? {
            Rank::Primary(Hierarchy::Master) => self.primary_master.as_ref(),
            Rank::Primary(Hierarchy::Slave) => self.primary_slave.as_ref(),
            Rank::Secondary(Hierarchy::Master) => self.secondary_master.as_ref(),
            Rank::Secondary(Hierarchy::Slave) => self.secondary_slave.as_ref(),
        }
    }
}

/// Emit Out Of Bound when a bound problem occured
fn check_bounds(start_sector: Sector, nbr_sectors: NbrSectors, drive_capacity: NbrSectors) -> AtaResult<()> {
    // 0 sector meens nothing for an human interface
    if nbr_sectors == NbrSectors(0) {
        Err(AtaError::NothingToDo)
    // Be careful with logical overflow
    } else if start_sector.0 as u64 > u64::max_value() as u64 - nbr_sectors.0 as u64 {
        Err(AtaError::OutOfBound)
    // raide disk capacity
    } else if start_sector.0 + nbr_sectors.0 as u64 > drive_capacity.0 {
        Err(AtaError::OutOfBound)
    } else {
        Ok(())
    }
}

#[no_mangle]
fn primary_hard_disk_interrupt_handler() -> u32 {
    0
}

#[no_mangle]
fn secondary_hard_disk_interrupt_handler() -> u32 {
    0
}
