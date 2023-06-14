//! Kernel tty manager
#![cfg_attr(all(not(test), not(feature = "std-print")), no_std)]

extern crate alloc;

#[macro_use]
pub mod macros;
#[cfg(feature = "serial-eprintln")]
#[macro_use]
pub mod uart_16550;
#[cfg(feature = "serial-eprintln")]
pub use uart_16550::UART_16550;

pub mod early_terminal;
pub use early_terminal::EarlyTerminal;

pub mod cursor;
pub use cursor::Cursor;

use ansi_escape_code::{color::Colored, Pos};
use screen::{bmp_loader, bmp_loader::BmpImage, AdvancedGraphic, Drawer, ScreenMonad};

mod tty;
pub use tty::{BufferedTty, Scroll, Tty, WriteMode};

mod line_discipline;
use line_discipline::LineDiscipline;
pub use line_discipline::ReadResult;

pub mod log;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;
use keyboard::{KeyCode, KeySymb, ScanCode};

use lazy_static::lazy_static;
use sync::Spinlock;

use libc_binding::winsize;

lazy_static! {
    /// Output monad
    pub static ref SCREEN_MONAD: Spinlock<ScreenMonad> = Spinlock::new(ScreenMonad::new());
}

/// Main EarlyTerminal Globale
pub static mut EARLY_TERMINAL: EarlyTerminal = EarlyTerminal::new();

/// No initialized at the beginning
pub static mut TERMINAL: Option<Terminal> = None;

/// Main structure of the terminal center
#[derive(Debug, Clone)]
pub struct Terminal {
    ttys: BTreeMap<usize, Box<LineDiscipline>>,
}

const MAX_SCREEN_BUFFER: usize = 10;

/// Index of the system log TTY
pub const SYSTEM_LOG_TTY_IDX: usize = 0;

/// Describe the output if the `handle_tty_control` function`
enum TtyControlOutput {
    /// The tty switch was succesfull, return the new foreground tty index
    SwitchSuccess(usize),
    /// A switch was requested but they some errors happened
    SwitchError,
    /// It is not a tty control character !
    NoControlInput,
}

impl Terminal {
    /// TTY constructor: Construct the system tty
    pub fn new() -> Self {
        let mut res = Self {
            ttys: BTreeMap::new(),
        };
        res.add_tty(SYSTEM_LOG_TTY_IDX);
        res
    }

    /// Add a tty of index n
    fn add_tty(&mut self, index: usize) -> usize {
        let screen_monad = SCREEN_MONAD.lock();

        let v = if screen_monad.is_graphic() {
            let (height, width, bpp) = screen_monad.query_graphic_infos().unwrap();
            let size = width * height * bpp / 8;
            // TODO: check fallible()
            let mut v = vec![0; size];
            bmp_loader::draw_image(
                unsafe { &_univers_bmp_start },
                v.as_mut_ptr(),
                width,
                height,
                bpp,
            )
            .unwrap();
            Some(v)
        } else {
            None
        };

        // TODO: Must protect from MAX_TTY_IDX, already added tty and memory
        let size = screen_monad.query_window_size();
        let line_discipline = Box::new(LineDiscipline::new(BufferedTty::new(Tty::new(
            false,
            size.line,
            size.column,
            MAX_SCREEN_BUFFER,
            v,
        ))));
        self.ttys.insert(index, line_discipline);
        index
    }

    /// Switch to a new foreground tty, returns TRUE if success
    fn switch_foreground_tty(&mut self, new_foreground_tty: usize) -> bool {
        // Check is the new desired TTY is available
        match self.ttys.get(&new_foreground_tty) {
            None => false,
            Some(_) => {
                // Set the current tty as 'background'
                self.ttys
                    .values_mut()
                    .find(|l| l.get_tty().foreground)
                    .map(|l| l.get_tty_mut().foreground = false);

                // Set the new tty as 'foreground'
                self.ttys.get_mut(&new_foreground_tty).map(|tty| {
                    let new_tty = tty.get_tty_mut();
                    new_tty.foreground = true;
                    new_tty.refresh(true);
                });
                true
            }
        }
    }

    /// Get the foregounded TTY
    pub fn get_foreground_tty(&mut self) -> &mut LineDiscipline {
        self.ttys
            .values_mut()
            .find(|l| l.get_tty().foreground)
            .expect("no foreground tty")
    }

    /// Open a TTY in point of view of IPC !
    pub fn open(&mut self, tty_index: usize, uid_file_op: usize) {
        if self.ttys.get(&tty_index).is_none() {
            self.add_tty(tty_index);
            self.ttys
                .get_mut(&tty_index)
                .expect("Cannot open a non existant TTY")
                .open(uid_file_op);
        }
    }

    /// Read a Key from the buffer
    pub fn read(&mut self, buf: &mut [u8], tty_index: usize) -> ReadResult {
        self.ttys
            .get_mut(&tty_index)
            .expect("Cannot read from non existant TTY")
            .read(buf)
    }

    /// Get the TTY n
    pub fn get_tty(&mut self, tty_index: usize) -> &mut BufferedTty {
        &mut self
            .ttys
            .get_mut(&tty_index)
            .expect("Cannot get a non-existant TTY")
            .tty
    }

    pub fn get_line_discipline(&mut self, tty_index: usize) -> &mut LineDiscipline {
        self.ttys.get_mut(&tty_index).expect("WTF")
    }

    /// Handle the ketPressed for special TTY changes. Report a foreground TTY modification
    pub fn handle_key_pressed(
        &mut self,
        scancode: ScanCode,
        _keycode: Option<KeyCode>,
        keysymb: Option<KeySymb>,
    ) -> Option<usize> {
        use TtyControlOutput::*;

        if let Some(keysymb) = keysymb {
            match self.handle_tty_control(keysymb) {
                SwitchSuccess(tty_index) => Some(tty_index),
                SwitchError => None,
                NoControlInput => {
                    let tty = self.get_foreground_tty();

                    #[cfg(feature = "serial-eprintln")]
                    {
                        if tty.is_raw_mode() {
                            match tty.handle_scancode(scancode) {
                                Ok(_size) => serial_println!("read buffer len: {}", _size),
                                Err(_e) => serial_println!("handle_key_pressed: {}", _e),
                            }
                        } else {
                            serial_println!("Not on raw mode");
                            tty.handle_key_pressed(keysymb).expect("write input failed");
                        }
                    }
                    #[cfg(not(feature = "serial-eprintln"))]
                    {
                        if tty.is_raw_mode() {
                            let _size = tty.handle_scancode(scancode).expect("write input failed");
                        } else {
                            tty.handle_key_pressed(keysymb).expect("write input failed");
                        }
                    }
                    None
                }
            }
        } else {
            #[cfg(feature = "serial-eprintln")]
            {
                match self.get_foreground_tty().handle_scancode(scancode) {
                    Ok(_size) => serial_println!("read buffer len: {}", _size),
                    Err(_e) => serial_println!("handle_key_pressed: {}", _e),
                }
            }
            #[cfg(not(feature = "serial-eprintln"))]
            self.get_foreground_tty()
                .handle_scancode(scancode)
                .expect("write input failed");
            None
        }
    }

    pub fn get_window_capabilities(&self) -> winsize {
        // we handle only one size of screen in all tty for the moment
        let screen_monad = SCREEN_MONAD.lock();
        let size = screen_monad.query_window_size();

        let mut win: winsize = unsafe { core::mem::zeroed() };
        win.ws_row = size.line as u16;
        win.ws_col = size.column as u16;
        if let Ok((height, width, bpp)) = screen_monad.query_graphic_infos() {
            win.ws_xpixel = width as u16;
            win.ws_ypixel = height as u16;
            win.bpp = bpp as u16;
        }
        win
    }

    /// Provide a tiny interface to control some features on the tty
    fn handle_tty_control(&mut self, keysymb: KeySymb) -> TtyControlOutput {
        let n = match keysymb {
            KeySymb::F1 => Some(SYSTEM_LOG_TTY_IDX),
            KeySymb::F2 => Some(1),
            KeySymb::F3 => Some(2),
            KeySymb::F4 => Some(3),
            KeySymb::F5 => Some(4),
            KeySymb::F6 => Some(5),
            KeySymb::F7 => Some(6),
            KeySymb::F8 => Some(7),
            KeySymb::F9 => Some(8),
            KeySymb::F10 => Some(9),
            KeySymb::F11 => Some(10),
            KeySymb::F12 => Some(11),
            _ => None,
        };
        use TtyControlOutput::*;
        if let Some(n) = n {
            if self.switch_foreground_tty(n) == true {
                SwitchSuccess(n)
            } else {
                SwitchError
            }
        } else {
            NoControlInput
        }
    }
}

extern "C" {
    static _univers_bmp_start: BmpImage;
}

/// Extern function for initialisation
pub fn init_terminal() {
    SCREEN_MONAD.lock().switch_graphic_mode(0x118).unwrap();

    let mut term = Terminal::new();
    term.get_tty(SYSTEM_LOG_TTY_IDX).tty.cursor.visible = false;

    term.switch_foreground_tty(SYSTEM_LOG_TTY_IDX);

    let screen_monad = SCREEN_MONAD.lock();
    if screen_monad.is_graphic() {
        let (height, width, bpp) = screen_monad.query_graphic_infos().unwrap();
        let size = width * height * bpp / 8;

        let mut v: Vec<u8> = vec![84; size];
        bmp_loader::draw_image(
            unsafe { &_univers_bmp_start },
            v.as_mut_ptr(),
            width,
            height,
            bpp,
        )
        .unwrap();
        term.get_tty(SYSTEM_LOG_TTY_IDX)
            .tty
            .set_background_buffer(v);
    }

    // unlock mutex
    drop(screen_monad);

    term.get_foreground_tty().tty.tty.refresh(true);
    unsafe {
        TERMINAL = Some(term);
    }
    self::log::init().unwrap();

    let size = SCREEN_MONAD.lock().query_window_size();
    printfixed!(
        Pos {
            line: 1,
            column: size.column - 17
        },
        "{}",
        "Turbo Fish v10.0".green()
    );
    ::log::info!("Terminal has been initialized");
}
