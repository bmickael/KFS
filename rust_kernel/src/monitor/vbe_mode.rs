mod init;
pub use init::*;

use alloc::vec;
use alloc::vec::Vec;
use core::slice;

use super::{AdvancedGraphic, Color, Drawer, IoResult, WriteMode};

extern "C" {
    /* Fast and Furious ASM SSE2 method to copy entire buffers */
    fn _sse2_memcpy(dst: *mut u8, src: *const u8, len: usize) -> ();
    fn _sse2_memzero(dst: *mut u8, len: usize) -> ();
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

extern "C" {
    static _font: Font;
    static _font_width: usize;
    static _font_height: usize;
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
    /// fixed characters buffer
    fixed_characters_buffer: Vec<Option<(u8, RGB)>>,
    /// set write mode
    write_mode: WriteMode,
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
            fixed_characters_buffer: vec![None; columns * lines],
            write_mode: WriteMode::Dynamic,
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
    /// Copy characters from both characters_buffer to double buffer
    fn render_text_buffer(&mut self, x1: usize, x2: usize) {
        unsafe {
            let buffers = [&self.characters_buffer, &self.fixed_characters_buffer];
            for buffer in buffers.iter() {
                for (i, elem) in buffer[x1..x2].iter().enumerate().filter_map(|(i, x)| match x {
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
        let dest = match self.write_mode {
            WriteMode::Dynamic => &mut self.characters_buffer,
            WriteMode::Fixed => &mut self.fixed_characters_buffer,
        };
        dest[cursor_y * self.columns + cursor_x] = Some((c as u8, self.text_color));
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
        // clean the fixed character buffer
        for elem in self.fixed_characters_buffer.iter_mut() {
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
    fn set_write_mode(&mut self, write_mode: WriteMode) -> IoResult {
        self.write_mode = write_mode;
        Ok(())
    }
}
