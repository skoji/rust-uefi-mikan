#![no_std]
#![no_main]
#![feature(asm)]
#![feature(lang_items)]

use core::fmt::Write;
use core::panic::PanicInfo;
use laranja_kernel::graphics::{FrameBuffer, Graphics, ModeInfo, PixelColor};

static mut RAW_GRAPHICS: [u8; core::mem::size_of::<Graphics>()] =
    [0; core::mem::size_of::<Graphics>()];

fn global_graphics() -> &'static mut Graphics {
    unsafe { &mut *(RAW_GRAPHICS.as_mut_ptr() as *mut laranja_kernel::graphics::Graphics) }
}

fn create_global_graphics(fb: *mut FrameBuffer, mi: *mut ModeInfo) {
    let fb = unsafe { *fb };
    let mi = unsafe { *mi };

    unsafe {
        core::ptr::copy(
            &Graphics::new(fb, mi),
            RAW_GRAPHICS.as_mut_ptr() as *mut Graphics,
            1,
        )
    };
}

#[no_mangle]
extern "C" fn kernel_main(fb: *mut FrameBuffer, mi: *mut ModeInfo) {
    create_global_graphics(fb, mi);
    let graphics = global_graphics();

    graphics.clear(&PixelColor(32, 32, 32));
    let (width, _) = graphics.resolution();
    let mut x = width / 3;
    let mut y = 100;

    for i in 0x20u8..0x7Fu8 {
        graphics.write_ascii(x, y, i.into(), &PixelColor(0, 255, 0));
        x += 8;
        if x > width / 3 * 2 {
            x = width / 3;
            y += 32;
        }
    }
    x = width / 3;
    y += 32;
    graphics.write_string(x, y, "Hello, Laranja/Mikan OS", &PixelColor(255, 0, 255));

    let mut writer = graphics.text_writer(width / 3, y + 32, &PixelColor(255, 255, 0));
    writeln!(writer, "1 + 2 = {}", 1 + 2).unwrap();

    unsafe {
        loop {
            asm!("hlt");
        }
    }
}

#[lang = "eh_personality"]
fn eh_personality() {}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
