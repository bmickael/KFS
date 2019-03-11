use super::{AdvancedGraphic, Color, Drawer, IoResult};
use crate::ffi::c_char;
use crate::memory::allocator::virtual_page_allocator::KERNEL_VIRTUAL_PAGE_ALLOCATOR;
use crate::memory::tools::{PhysicalAddr, VirtualAddr};
use crate::registers::{real_mode_op, BaseRegisters};
use alloc::vec;
use alloc::vec::Vec;
use core::result::Result;
use core::slice;

const TEMPORARY_PTR_LOCATION: *mut u8 = 0x2000 as *mut u8;

const LINEAR_FRAMEBUFFER_VIRTUAL_ADDR: *mut u8 = 0xf0000000 as *mut u8;

extern "C" {
    /* Fast and Furious ASM SSE2 method to copy entire buffers */
    fn _sse2_memcpy(dst: *mut u8, src: *const u8, len: usize) -> ();
    fn _sse2_memzero(dst: *mut u8, len: usize) -> ();
}

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
pub struct VbeInfo {
    /*0  */ pub vbe_signature: [c_char; 4], //db 'VESA' ; VBE Signature
    /*4  */ pub vbe_version: u16, //dw 0300h ; vbe version
    /*6  */ pub oem_string_offset: u32, //dd ? ; vbe_far_offset to oem string
    /*10 */ pub capabilities: u32, //db 4 dup (?) ; capabilities of graphics controller
    /*14 */ pub video_mode_offset: u32, //dd ? ; vbe_far_offset to video_mode_list
    /*18 */ pub total_memory: u16, //dw ? ; number of 64kb memory blocks added for vbe 2.0+
    /*20 */ pub oem_software_rev: u16, //dw ? ; vbe implementation software revision
    /*22 */ pub oem_vendor_name_offset: u32, //dd ? ; vbe_far_offset to vendor name string
    /*26 */ pub oem_product_name_offset: u32, //dd ? ; vbe_far_offset to product name string
    /*30 */ pub oem_product_rev_offset: u32, //dd ? ; vbe_far_offset to product revision string
    /*34 */ pub reserved: VbeInfoReserved, //db 222 dup (?) ; reserved for vbe implementation scratch area
    /*256*/ pub oem_data: VbeInfoOemData, //db 256 dup ; data area for oem strings
}

define_raw_data!(VbeInfoReserved, 222);
define_raw_data!(VbeInfoOemData, 256);

impl VbeInfo {
    /// only way to initialize VbeInfo safely transform all the pointers within the struct by their offsets
    unsafe fn new(ptr: *const Self) -> Self {
        Self { video_mode_offset: (*ptr).video_mode_offset - ptr as u32, ..*ptr }
    }
    /// calculate the mode ptr using the address of self and the offset
    fn get_video_mode_ptr(&self) -> *const u16 {
        unsafe { (self as *const Self as *const u8).add(self.video_mode_offset as usize) as *const u16 }
    }
    /// return the number of modes available
    /// The VideoModePtr is a VbeFarPtr that points to a list of mode numbers for all display modes
    /// supported by the VBE implementation. Each mode number occupies one word (16 bits). The list
    /// of mode numbers is terminated by a -1 (0FFFFh). The mode numbers in this list represent all of
    /// the potentially supported modes by the display controller.
    fn nb_mode(&self) -> usize {
        let mut i = 0;
        let video_mode_ptr = self.get_video_mode_ptr();
        unsafe {
            while *((video_mode_ptr).offset(i as isize)) != 0xFFFF {
                i += 1;
                // 111 is the maximum number of modes because reserved is 222 bytes
                if i >= 111 {
                    return i;
                }
            }
        }
        i
    }
    /// return an iterator on available modes
    pub fn iter_modes(&self) -> core::slice::Iter<u16> {
        unsafe { core::slice::from_raw_parts(self.get_video_mode_ptr(), self.nb_mode()).iter() }
    }
    /// return the best resolution mode available which is in 3 bytes color if any.
    pub fn find_best_resolution_mode(&self) -> (u16, ModeInfo) {
        self.iter_modes()
            .map(|m| (*m, query_mode_info(*m).unwrap()))
            .max_by(|(_, a), (_, b)| {
                if a.bits_per_pixel != b.bits_per_pixel {
                    a.bits_per_pixel.cmp(&b.bits_per_pixel) // more bits for pixel is better
                } else {
                    (a.x_resolution + a.y_resolution).cmp(&(b.x_resolution + b.y_resolution))
                }
            })
            .unwrap()
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
pub struct ModeInfo {
    /// Mandatory information for all VBE revisions
    mode_attributes: u16, // dw ? ; mode attributes
    win_a_attributes: u8,     // db ? ; window A attributes
    win_b_attributes: u8,     // db ? ; window B attributes
    win_granularity: u16,     // dw ? ; window granularity
    win_size: u16,            // dw ? ; window size
    win_a_segment: u16,       // dw ? ; window A start segment
    win_b_segment: u16,       // dw ? ; window B start segment
    win_func_ptr: u32,        // dd ? ; real mode pointer to window function
    bytes_per_scan_line: u16, // dw ? ; bytes per scan line
    /// Mandatory information for VBE 1.2 and above
    x_resolution: u16, // dw ? ; horizontal resolution in pixels or characters 3
    y_resolution: u16,        // dw ? ; vertical resolution in pixels or characters
    x_char_size: u8,          // db ? ; character cell width in pixels
    y_char_size: u8,          // db ? ; character cell height in pixels
    number_of_planes: u8,     // db ? ; number of memory planes
    bits_per_pixel: u8,       // db ? ; bits per pixel
    number_of_banks: u8,      // db ? ; number of banks
    memory_model: u8,         // db ; memory model type
    bank_size: u8,            // db ? ; bank size in KB
    number_of_image_pages: u8, // db ; number of images
    reserved1: u8,            // db 1 ; reserved for page function
    /// Direct Color fields (required for direct/6 and YUV/7 memory models)
    red_mask_size: u8, // db ? ; size of direct color red mask in bits
    red_field_position: u8,   // db ? ; bit position of lsb of red mask
    green_mask_size: u8,      // db ? ; size of direct color green mask in bits
    green_field_position: u8, // db ? ; bit position of lsb of green mask
    blue_mask_size: u8,       // db ? ; size of direct color blue mask in bits
    blue_field_position: u8,  // db ? ; bit position of lsb of blue mask
    rsvd_mask_size: u8,       // db ? ; size of direct color reserved mask in bits
    rsvd_field_position: u8,  // db ? ; bit position of lsb of reserved mask
    direct_color_mode_info: u8, // db ? ; direct color mode attributes
    /// Mandatory information for VBE 2.0 and above
    phys_base_ptr: u32, // dd ? ; physical address for flat memory frame buffer
    reserved2: u32,           // dd 0 ; Reserved - always set to 0
    reserved3: u16,           // dw 0 ; Reserved - always set to 0
    /// Mandatory information for VBE 3.0 and above
    lin_bytes_per_scan_line: u16, // dw ? ; bytes per scan line for linear modes
    bnk_number_of_image_pages: u8, // db ? ; number of images for banked modes
    lin_number_of_image_pages: u8, // db ? ; number of images for linear modes
    lin_red_mask_size: u8,    // db ? ; size of direct color red mask (linear modes)
    lin_red_field_position: u8, // db ? ; bit position of lsb of red mask (linear modes)
    lin_green_mask_size: u8,  // db ? ; size of direct color green mask (linear modes)
    lin_green_field_position: u8, //db // ? ? ; bit position of lsb of green mask (linear modes)
    lin_blue_mask_size: u8,   // db ? ; size of direct color blue mask (linear modes)
    lin_blue_field_position: u8, // db ? ; bit position of lsb of blue mask (linear modes)
    lin_rsvd_mask_size: u8,   // db ? ; size of direct color reserved mask (linear modes)
    lin_rsvd_field_position: u8, // db ? ; bit position of lsb of reserved mask (linear modes)
    max_pixel_clock: u32,     // dd ? ; maximum pixel clock (in Hz) for graphics mode
    reserved4: ModeInfoReserved4, //db 189 dup (?) ; remainder of ModeInfo
}

define_raw_data!(ModeInfoReserved4, 189);

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
pub struct CrtcInfo {
    horizontal_total: u16,      //dw ?  ; Horizontal total in pixels
    horizontal_sync_start: u16, //dw ?  ; Horizontal sync start in pixels
    horizontal_sync_end: u16,   //dw ?  ; Horizontal sync end in pixels
    vertical_total: u16,        //dw ?  ; Vertical total in lines
    vertical_sync_start: u16,   //dw ?  ; Vertical sync start in lines
    vertical_sync_end: u16,     //dw ?  ; Vertical sync end in lines
    flags: u8,                  //db ?  ; Flags (Interlaced, Double Scan etc)
    pixel_clock: u32,           //dd ?  ; Pixel clock in units of Hz
    refresh_rate: u16,          //dw ?  ; Refresh rate in units of 0.01 Hz
    reserved: CrtcInfoReserved, //db 40 dup (?) ; remainder of mode_info_block
}

define_raw_data!(CrtcInfoReserved, 40);

extern "C" {
    static _font: Font;
    static _font_width: usize;
    static _font_height: usize;
}

/// structure contains font for the 255 ascii char
#[repr(C)]
// TODO Must be declared dynamiquely and remove 16 magic
struct Font(pub [u8; 16 * 256]);

impl Font {
    /// return the 16 * u8 slice font corresponding to the char
    fn get_char(&self, c: u8) -> &[u8] {
        &self.0[c as usize * 16..(c as usize + 1) * 16]
    }
}

#[derive(Debug, Copy, Clone)]
pub struct RGB(pub u32);

impl From<Color> for RGB {
    fn from(c: Color) -> Self {
        match c {
            Color::Red => RGB(0xFF0000),
            Color::Green => RGB(0x00FF00),
            Color::Blue => RGB(0x0000FF),
            Color::Yellow => RGB(0xFFFF00),
            Color::Cyan => RGB(0x00FFFF),
            Color::Brown => RGB(0xA52A2A),
            Color::Magenta => RGB(0xFF00FF),
            Color::White => RGB(0xFFFFFF),
            Color::Black => RGB(0x000000),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VbeMode {
    /// linear frame buffer address
    linear_frame_buffer: *mut u8,
    /// double framebuffer location
    db_frame_buffer: Vec<u8>,
    /// graphic buffer location
    graphic_buffer: Vec<u8>,
    /// character buffer
    characters_buffer: Vec<Option<(u8, RGB)>>,
    /// in pixel
    width: usize,
    /// in pixel
    height: usize,
    /// in bytes
    bytes_per_pixel: usize,
    /// bytes_per_line
    pitch: usize,
    /// in pixel
    char_height: usize,
    /// in pixel
    char_width: usize,
    /// characters per columns
    columns: usize,
    /// number of characters lines
    lines: usize,
    /// current text color
    text_color: RGB,
    // Some informations about graphic mode
    mode_info: ModeInfo,
    // Some informations about how the screen manage display
    crtc_info: CrtcInfo,
}

impl VbeMode {
    pub fn new(
        linear_frame_buffer: *mut u8,
        width: usize,
        height: usize,
        bpp: usize,
        mode_info: ModeInfo,
        crtc_info: CrtcInfo,
    ) -> Self {
        let bytes_per_pixel: usize = bpp / 8;
        let screen_size: usize = bytes_per_pixel * width * height;
        let columns: usize = unsafe { width / _font_width };
        let lines: usize = unsafe { height / _font_height };
        Self {
            linear_frame_buffer,
            db_frame_buffer: vec![0; screen_size],
            graphic_buffer: vec![0; screen_size],
            characters_buffer: vec![None; columns * lines],
            width,
            height,
            bytes_per_pixel,
            pitch: width * bytes_per_pixel,
            char_width: unsafe { _font_width },
            char_height: unsafe { _font_height },
            columns: columns,
            lines: lines,
            text_color: Color::White.into(),
            crtc_info,
            mode_info,
        }
    }
    /// return window size in nb char
    pub fn query_window_size(&self) -> (usize, usize) {
        (self.height / self.char_height, self.width / self.char_width)
    }
    /// put pixel at position y, x in pixel unit
    #[inline(always)]
    fn put_pixel(db_fb: &mut Vec<u8>, loc: usize, color: RGB) {
        unsafe {
            *((&mut db_fb[loc]) as *mut u8 as *mut u32) = color.0;
        }
    }
    /*
    TODO Cannot manage properly without Dynamic allocator
    /// display pixel at linear position pos in pixel unit
    #[allow(dead_code)]
    fn put_pixel_lin(&self, pos: usize, color: RGB) {
        unsafe {
            *((self.db_frame_buffer.add(pos * self.bytes_per_pixel)) as *mut RGB) = color;
        }
    }
    TODO Cannot manage properly without Dynamic allocator
    /// fill screen with color
    #[allow(dead_code)]
    pub fn fill_screen(&self, color: RGB) {
        for p in 0..self.width * self.height {
            self.put_pixel_lin(p, color);
        }
    }
     */
    /// Copy characters from characters_buffer to double buffer
    fn render_text_buffer(&mut self, x1: usize, x2: usize) {
        unsafe {
            for (i, elem) in self.characters_buffer[x1..x2].iter().enumerate().filter_map(|(i, x)| match x {
                Some(x) => Some((i, x)),
                None => None,
            }) {
                let char_font = _font.get_char((*elem).0 as u8);
                let cursor_x = (i + x1) % self.columns;
                let cursor_y = (i + x1) / self.columns;

                let mut y = cursor_y * self.char_height;
                let mut x;
                for l in char_font {
                    x = cursor_x * self.char_width;
                    for shift in (0..8).rev() {
                        if *l & 1 << shift != 0 {
                            Self::put_pixel(
                                &mut self.db_frame_buffer,
                                y * self.pitch + x * self.bytes_per_pixel,
                                (*elem).1,
                            );
                        }
                        x += 1;
                    }
                    y += 1;
                }
            }
        }
    }
    /// refresh framebuffer
    pub fn refresh_screen(&mut self) {
        // Copy graphic buffer to double buffer
        unsafe {
            _sse2_memcpy(self.db_frame_buffer.as_mut_ptr(), self.graphic_buffer.as_ptr(), self.pitch * self.height);
        }
        // Rend all character from character_buffer to db_buffer
        self.render_text_buffer(0, self.columns * self.lines);
        // copy double buffer to linear frame buffer
        unsafe {
            _sse2_memcpy(self.linear_frame_buffer, self.db_frame_buffer.as_ptr(), self.pitch * self.height);
        }
    }
}

impl Drawer for VbeMode {
    fn draw_character(&mut self, c: char, cursor_y: usize, cursor_x: usize) {
        self.characters_buffer[cursor_y * self.columns + cursor_x] = Some((c as u8, self.text_color));
    }
    fn scroll_screen(&mut self) {
        // scroll left the character_buffer
        let m = self.columns * (self.lines - 1);
        self.characters_buffer.copy_within(self.columns.., 0);
        for elem in self.characters_buffer[m..].iter_mut() {
            *elem = None;
        }
        self.refresh_screen();
    }
    fn clear_screen(&mut self) {
        // clean the character buffer
        for elem in self.characters_buffer.iter_mut() {
            *elem = None;
        }
        self.refresh_screen();
    }
    fn set_text_color(&mut self, color: Color) -> IoResult {
        self.text_color = color.into();
        Ok(())
    }
}

impl AdvancedGraphic for VbeMode {
    fn refresh_text_line(&mut self, x1: usize, x2: usize, y: usize) {
        let lfb = unsafe { slice::from_raw_parts_mut(self.linear_frame_buffer, self.pitch * self.height) };

        // Copy selected area from graphic buffer to double frame buffer
        for i in 0..self.char_height {
            let o1 = (y * self.char_height + i) * self.pitch + x1 * self.char_width * self.bytes_per_pixel;
            let o2 = o1 + (x2 - x1) * self.char_width * self.bytes_per_pixel;
            self.db_frame_buffer[o1..o2].copy_from_slice(&self.graphic_buffer[o1..o2]);
        }
        // get characters from character buffer and pixelize it in db_buffer
        self.render_text_buffer(y * self.columns + x1, y * self.columns + x2);
        // Copy selected area from double buffer to linear frame buffer
        for i in 0..self.char_height {
            let o1 = (y * self.char_height + i) * self.pitch + x1 * self.char_width * self.bytes_per_pixel;
            let o2 = o1 + (x2 - x1) * self.char_width * self.bytes_per_pixel;
            lfb[o1..o2].copy_from_slice(&self.db_frame_buffer[o1..o2]);
        }
    }
    fn draw_graphic_buffer<T: Fn(*mut u8, usize, usize, usize) -> IoResult>(&mut self, closure: T) -> IoResult {
        closure(self.graphic_buffer.as_mut_ptr(), self.width, self.height, self.bytes_per_pixel * 8)?;
        self.refresh_screen();
        Ok(())
    }
}

fn vbe_real_mode_op(reg: BaseRegisters, bios_int: u16) -> core::result::Result<(), VbeError> {
    /*
     ** AL == 4Fh: ** Function is supported
     ** AH == 00h: Function call successful
     */
    unsafe {
        let res = real_mode_op(reg, bios_int);
        if res & 0xFF != 0x4F || res & 0xFF00 != 0x00 {
            Err(res.into())
        } else {
            Ok(())
        }
    }
}

unsafe fn save_vbe_info() -> Result<VbeInfo, VbeError> {
    // VBE 3.0 specification says to put 'VBE2' in vbe_signature field to have pointers
    // points to reserved field instead of far pointer. So in practice it doesn't work
    TEMPORARY_PTR_LOCATION.copy_from("VBE2".as_ptr(), 4);
    let reg: BaseRegisters = BaseRegisters { edi: TEMPORARY_PTR_LOCATION as u32, eax: 0x4f00, ..Default::default() };
    vbe_real_mode_op(reg, 0x10)?;
    Ok(VbeInfo::new(TEMPORARY_PTR_LOCATION as *const VbeInfo))
}

fn query_mode_info(mode_number: u16) -> Result<ModeInfo, VbeError> {
    let reg: BaseRegisters = BaseRegisters {
        edi: TEMPORARY_PTR_LOCATION as u32,
        eax: 0x4f01,
        ecx: mode_number as u32,
        ..Default::default()
    };
    unsafe { vbe_real_mode_op(reg, 0x10).map(|_| *(TEMPORARY_PTR_LOCATION as *const ModeInfo)) }
}

unsafe fn set_vbe_mode(mode_number: u16) -> Result<CrtcInfo, VbeError> {
    let reg: BaseRegisters = BaseRegisters {
        edi: TEMPORARY_PTR_LOCATION as u32,
        eax: 0x4f02,
        ebx: (mode_number | 1 << 14) as u32, // set the bit 14 (from 0) to use linear frame buffer
        ..Default::default()
    };
    vbe_real_mode_op(reg, 0x10)?;
    Ok(*(TEMPORARY_PTR_LOCATION as *const CrtcInfo))
}

/// do all nessesary initialisation and switch to vbe mode 'mode' if given, if not swith to the best resolution mode
pub fn init_graphic_mode(mode: Option<u16>) -> Result<VbeMode, VbeError> {
    unsafe {
        let vbe_info = save_vbe_info()?;
        let mode_info: ModeInfo;
        let crtc_info: CrtcInfo;
        match mode {
            Some(m) => {
                mode_info = query_mode_info(m)?;
                crtc_info = set_vbe_mode(m)?;
            }
            None => {
                let result = vbe_info.find_best_resolution_mode();
                mode_info = result.1;
                crtc_info = set_vbe_mode(result.0)?;
            }
        }
        KERNEL_VIRTUAL_PAGE_ALLOCATOR
            .as_mut()
            .unwrap()
            .reserve(
                VirtualAddr(LINEAR_FRAMEBUFFER_VIRTUAL_ADDR as usize),
                PhysicalAddr(mode_info.phys_base_ptr as usize),
                (mode_info.x_resolution as usize * mode_info.y_resolution as usize * mode_info.bits_per_pixel as usize
                    / 8)
                .into(),
            )
            .unwrap();
        Ok(VbeMode::new(
            LINEAR_FRAMEBUFFER_VIRTUAL_ADDR,
            mode_info.x_resolution as usize,
            mode_info.y_resolution as usize,
            mode_info.bits_per_pixel as usize,
            mode_info,
            crtc_info,
        ))
    }
}

#[derive(Debug, Copy, Clone)]
pub enum VbeError {
    ///AH == 01h:
    Failed,
    ///AH == 02h:
    NotSupportedCurrentConfig,
    ///AH == 03h:
    InvalidCurentMode,
    ///Unknown Error
    Unknown,
}

impl From<u16> for VbeError {
    fn from(err_code: u16) -> Self {
        match err_code & 0xFF00 {
            0x0100 => VbeError::Failed,
            0x0200 => VbeError::NotSupportedCurrentConfig,
            0x0300 => VbeError::InvalidCurentMode,
            _ => VbeError::Unknown,
        }
    }
}
