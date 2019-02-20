use crate::debug;
use crate::interrupts;
use crate::interrupts::pit::*;
use crate::interrupts::{pic_8259, PIC_8259};
use crate::monitor::bmp_loader::*;
use crate::monitor::*;
use crate::multiboot::{save_multiboot_info, MultibootInfo, MULTIBOOT_INFO};
use crate::test_helpers::fucking_big_string::fucking_big_string;
use crate::timer::Rtc;

extern "C" {
    static _asterix_bmp_start: BmpImage;
    static _wanggle_bmp_start: BmpImage;
}

#[no_mangle]
pub extern "C" fn kmain(multiboot_info: *const MultibootInfo) -> u32 {
    save_multiboot_info(multiboot_info);
    println!("multiboot_infos {:#?}", MULTIBOOT_INFO);
    println!("base memory: {:?} {:?}", MULTIBOOT_INFO.unwrap().mem_lower, MULTIBOOT_INFO.unwrap().mem_upper);

    unsafe {
        interrupts::init();

        SCREEN_MONAD.switch_graphic_mode(Some(0x118)).unwrap();
        SCREEN_MONAD.set_text_color(Color::Blue).unwrap();
        SCREEN_MONAD.clear_screen();
        SCREEN_MONAD
            .draw_graphic_buffer(|buffer: *mut u8, width: usize, height: usize, bpp: usize| {
                draw_image(&_asterix_bmp_start, buffer, width, height, bpp)
            })
            .unwrap();

        PIT0.configure(OperatingMode::RateGenerator);
        PIT0.start_at_frequency(18.0).unwrap();
        PIC_8259.enable_irq(pic_8259::Irq::SystemTimer);
    }

    debug::bench_start();
    fucking_big_string(3);
    let t = debug::bench_end();
    println!("{:?} ms ellapsed", t);

    println!("from {}", function!());

    println!("irqs state: {}", interrupts::get_interrupts_state());

    println!("irq mask: {:b}", PIC_8259.get_masks());

    let eflags = crate::registers::Eflags::get_eflags();
    println!("{:x?}", eflags);

    unsafe {
        PIT0.start_at_frequency(18.).unwrap();
    }
    debug::bench_start();
    println!("pit: {:?}", PIT0);
    unsafe {
        SCREEN_MONAD.set_text_color(Color::Green).unwrap();
        print!("H");
        SCREEN_MONAD.set_text_color(Color::Red).unwrap();
        print!("E");
        SCREEN_MONAD.set_text_color(Color::Blue).unwrap();
        print!("L");
        SCREEN_MONAD.set_text_color(Color::Yellow).unwrap();
        print!("L");
        SCREEN_MONAD.set_text_color(Color::Cyan).unwrap();
        print!("O");
        SCREEN_MONAD.set_text_color(Color::Brown).unwrap();
        print!(" ");
        SCREEN_MONAD.set_text_color(Color::Magenta).unwrap();
        print!("W");
        SCREEN_MONAD.set_text_color(Color::White).unwrap();
        print!("O");
        SCREEN_MONAD.set_text_color(Color::Green).unwrap();
        print!("R");
        SCREEN_MONAD.set_text_color(Color::Red).unwrap();
        print!("L");
        SCREEN_MONAD.set_text_color(Color::Blue).unwrap();
        print!("D");
        SCREEN_MONAD.set_text_color(Color::Yellow).unwrap();
        print!(" ");
        SCREEN_MONAD.set_text_color(Color::Cyan).unwrap();
        println!("!");
        SCREEN_MONAD.set_text_color(Color::White).unwrap();
    }
    println!("{:?} ms ellapsed", debug::bench_end());
    unsafe {
        SCREEN_MONAD
            .draw_graphic_buffer(|buffer: *mut u8, width: usize, height: usize, bpp: usize| {
                draw_image(&_wanggle_bmp_start, buffer, width, height, bpp)
            })
            .unwrap();
        SCREEN_MONAD.set_text_color(Color::Green).unwrap();
    }
    let mut rtc = Rtc::new();
    let date = rtc.read_date();
    println!("{}", date);
    0
}
