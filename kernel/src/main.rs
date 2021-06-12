#![no_std]
#![no_main]
#![feature(asm)]
#![feature(lang_items)]
#![feature(custom_test_frameworks)]
#![test_runner(tests::test_runner)]
#![reexport_test_harness_main = "test_main"]

mod ascii_font;
pub mod bitwise_macro;
pub mod console;
pub mod graphics;
pub mod log;
pub mod memory;
pub mod pci;
pub mod usb;
pub mod volatile;

use log::*;

use console::Console;
use core::panic::PanicInfo;
use graphics::{FrameBuffer, Graphics, ModeInfo, PixelColor};
use memory::MemoryDescriptor;
use pci::PciDevices;
use pci::{read_bar, read_class_code, read_vendor_id, scan_all_bus, ClassCode, Device};

const BG_COLOR: PixelColor = PixelColor(0, 80, 80);
const FG_COLOR: PixelColor = PixelColor(255, 128, 0);

const MOUSE_CURSOR_HEIGHT: usize = 24;

const MOUSE_CURSOR_SHAPE: [&str; MOUSE_CURSOR_HEIGHT] = [
    "@              ",
    "@@             ",
    "@.@            ",
    "@..@           ",
    "@...@          ",
    "@....@         ",
    "@.....@        ",
    "@......@       ",
    "@.......@      ",
    "@........@     ",
    "@.........@    ",
    "@..........@   ",
    "@...........@  ",
    "@............@ ",
    "@......@@@@@@@@",
    "@......@       ",
    "@....@@.@      ",
    "@...@ @.@      ",
    "@..@   @.@     ",
    "@.@    @.@     ",
    "@@      @.@    ",
    "@       @.@    ",
    "         @.@   ",
    "         @@@   ",
];

fn initialize(fb: *mut FrameBuffer, mi: *mut ModeInfo) {
    unsafe { Graphics::initialize_instance(fb, mi) }
    Console::initialize(&FG_COLOR, &BG_COLOR);
    Graphics::instance().clear(&BG_COLOR);
}

fn draw_mouse_cursor() {
    // draw mouse cursor which will never move
    let graphics = Graphics::instance();
    for (dy, l) in MOUSE_CURSOR_SHAPE.iter().enumerate() {
        for (dx, c) in l.chars().enumerate() {
            let x = 200 + dx;
            let y = 100 + dy;
            match c {
                '@' => {
                    graphics.write_pixel(x, y, &PixelColor(0, 0, 0));
                }
                '.' => {
                    graphics.write_pixel(x, y, &PixelColor(255, 255, 255));
                }
                _ => {}
            }
        }
    }
}
fn welcome_message() {
    print!(
        r"
    __                             _       ____  _____
   / /   ____ __________ _____    (_)___ _/ __ \/ ___/
  / /   / __ `/ ___/ __ `/ __ \  / / __ `/ / / /\__ \
 / /___/ /_/ / /  / /_/ / / / / / / /_/ / /_/ /___/ /
/_____/\__,_/_/   \__,_/_/ /_/_/ /\__,_/\____//____/
                            /___/

"
    );
    info!("Resolution {:?}", Graphics::instance().resolution());
}

fn list_pci_devices() -> PciDevices {
    let pci_devices = scan_all_bus().unwrap();
    debug!("scanned pci devices.");
    for dev in pci_devices.iter() {
        let vendor_id = read_vendor_id(dev.bus, dev.device, dev.function);
        let class_code = read_class_code(dev.bus, dev.device, dev.function);
        trace!(
            "{}.{}.{}:, vend {:04x}, class {}, head {:02x}",
            dev.bus,
            dev.device,
            dev.function,
            vendor_id,
            class_code,
            dev.header_type
        );
    }
    pci_devices
}

fn find_xhc(pci_devices: &PciDevices) -> Option<Device> {
    let mut xhc_dev = None;
    let xhcclass = ClassCode {
        base: 0x0c,
        sub: 0x03,
        interface: 0x30,
    };
    for dev in pci_devices.iter() {
        if dev.class_code == xhcclass {
            xhc_dev = Some(dev);
            if dev.get_vendor_id() == 0x8086 {
                break;
            }
        }
    }
    xhc_dev
}

fn switch_echi_to_xhci(_xhc_dev: &Device, pci_devices: &PciDevices) {
    let ehciclass = ClassCode {
        base: 0x0c,
        sub: 0x03,
        interface: 0x20,
    };
    let ehci = pci_devices
        .iter()
        .find(|&dev| dev.class_code == ehciclass && dev.get_vendor_id() == 0x8086);
    if ehci.is_none() {
        info!("no ehci");
    } else {
        panic!("ehci found, but do nothing for the present");
    }
}

#[repr(C, align(16))]
struct KernelStack<const N: usize> {
    pub stack: [u8; N],
}
const MAIN_STACK_SIZE: usize = 1024 * 1024;

static mut KERNEL_MAIN_STACK: KernelStack<MAIN_STACK_SIZE> = KernelStack {
    stack: [0; MAIN_STACK_SIZE],
};

#[no_mangle]
extern "C" fn kernel_main(
    fb: *mut FrameBuffer,
    mi: *mut ModeInfo,
    memmap_ptr: *mut MemoryDescriptor,
    memmap_length: usize,
) {
    let stack_addr =
        unsafe { (&KERNEL_MAIN_STACK.stack as *const u8).add(MAIN_STACK_SIZE) as usize };
    // move stack to rsp;
    unsafe { asm!("mov rsp, {}", in(reg) stack_addr) };
    kernel_main_newstack(fb, mi, memmap_ptr, memmap_length, stack_addr);
    // should not reach here.
    unsafe {
        loop {
            asm!("hlt");
        }
    }
}

#[no_mangle]
extern "C" fn kernel_main_newstack(
    fb: *mut FrameBuffer,
    mi: *mut ModeInfo,
    memmap_ptr: *mut MemoryDescriptor,
    memmap_length: usize,
    stack_addr: usize,
) {
    initialize(fb, mi);
    welcome_message();
    unsafe { trace!("stack top 0x{:?}", (&KERNEL_MAIN_STACK.stack as *const u8)) };
    trace!("stack bottom : {:x}", stack_addr);
    trace!("memmap_length : {}", memmap_length);
    let memmaps: &[MemoryDescriptor] =
        unsafe { core::slice::from_raw_parts(memmap_ptr, memmap_length) };
    debug!("memmap 0; {}", memmaps[0]);
    debug!("memmap 1; {}", memmaps[1]);
    debug!("memmap 2; {}", memmaps[2]);
    debug!("memmap 3; {}", memmaps[3]);
    debug!("memmap 4; {}", memmaps[4]);
    #[cfg(test)]
    test_main();

    let pci_devices = list_pci_devices();
    let xhc = find_xhc(&pci_devices);
    let xhc = match xhc {
        Some(xhc) => {
            info!(
                "xHC has been found: {}.{}.{}",
                xhc.bus, xhc.device, xhc.function
            );
            xhc
        }
        None => {
            panic!("no xHC device");
        }
    };
    switch_echi_to_xhci(&xhc, &pci_devices);
    let xhc_bar = read_bar(&xhc, 0).unwrap();
    debug!("xhc_bar = {:08x}", xhc_bar);
    let xhc_mmio_base = (xhc_bar & !0xf) as usize;
    debug!("xHC mmio_base = {:08x}", xhc_mmio_base);
    unsafe {
        usb::Controller::new(xhc_mmio_base);
    };
    info!("done");
    draw_mouse_cursor();
    unsafe {
        loop {
            asm!("hlt");
        }
    }
}

#[lang = "eh_personality"]
fn eh_personality() {}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("{}", info);
    loop {}
}

#[cfg(test)]
mod tests {
    use super::*;
    pub trait TestCaseFn {
        fn run_test(&self);
    }
    impl<T: Fn()> TestCaseFn for T {
        fn run_test(&self) {
            print!("{} ... ", core::any::type_name::<T>());
            self();
            println!("Ok");
        }
    }

    pub fn test_runner(tests: &[&dyn TestCaseFn]) {
        println!("Running tests : {}", tests.len());
        for test in tests {
            test.run_test();
        }
        println!("done.");
    }
}
