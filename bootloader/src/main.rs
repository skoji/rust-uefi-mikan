#![feature(abi_efiapi)]
#![feature(asm)]
#![feature(alloc_error_handler)]
#![no_std]
#![no_main]

#[macro_use]
extern crate alloc;
use alloc::string::ToString;
use console::gop;
use core::{
    fmt::Write,
    mem::{size_of, MaybeUninit},
};
use elf_rs::*;
use proto::console;
use uefi::{
    prelude::*,
    proto::{self, console::gop::GraphicsOutput, media::fs::SimpleFileSystem},
    table::boot::{EventType, MemoryDescriptor, Tpl},
};
use uefi::{
    proto::media::file::{File, FileAttribute, FileInfo, FileMode, FileType::Regular},
    table::boot::{AllocateType, MemoryType},
};

static mut LOGGER: Option<uefi::logger::Logger> = None;
static mut MMAP_BUF: [MaybeUninit<MemoryDescriptor>; 1024] = [MaybeUninit::uninit(); 1024];

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct FrameBufferInfo {
    pub fb: *mut u8,
    pub size: usize,
}

#[allow(dead_code)]
fn set_gop_mode(gop: &mut GraphicsOutput) {
    let mut mode: Option<gop::Mode> = None;
    for m in gop.modes().into_iter() {
        let m = m.unwrap();
        let res = m.info().resolution();

        // Hardcode for GPD Pocket / Lemur Pro.
        if (mode.is_none() && (1024, 768) == res) || (1200, 1920) == res || (1920, 1080) == res {
            mode = Some(m);
        }
    }

    if let Some(mode) = mode {
        gop.set_mode(&mode).unwrap().unwrap();
    }
}

fn exit_boot_services(_: uefi::Event) {
    uefi::alloc::exit_boot_services();
}

#[entry]
fn efi_main(handle: Handle, st: SystemTable<Boot>) -> Status {
    let bt = st.boot_services();

    let gop = if let Ok(gop) = bt.locate_protocol::<GraphicsOutput>() {
        let gop = gop.expect("Warnings encountered while opening GOP");
        unsafe { &mut *gop.get() }
    } else {
        panic!("no ogp");
    };

    //    uefi_services::init(&st).expect_success("Failed to initialize utilities");
    unsafe {
        uefi::alloc::init(bt);
        bt.create_event(
            EventType::SIGNAL_EXIT_BOOT_SERVICES,
            Tpl::NOTIFY,
            Some(exit_boot_services),
        )
        .map_inner(|_| ())
        .unwrap()
        .unwrap();
    }
    let stdout = st.stdout();
    let logger = unsafe {
        LOGGER = Some(uefi::logger::Logger::new(stdout));
        LOGGER.as_ref().unwrap()
    };
    log::set_logger(logger).unwrap();
    log::set_max_level(log::LevelFilter::Info);

    if st.firmware_vendor().to_string() != "EDK II" {
        // set gop mode if it is not in QEMU
        set_gop_mode(gop);
    }
    writeln!(stdout, "Hello from rust").unwrap();
    writeln!(stdout, "Firmware Vendor {}", st.firmware_vendor()).unwrap();

    // open file protocol
    let sfs = if let Ok(sfs) = bt.locate_protocol::<SimpleFileSystem>() {
        let sfs = sfs.expect("cant open filesystem protocol");
        unsafe { &mut *sfs.get() }
    } else {
        writeln!(stdout, "no simple filesystem protocol").unwrap();
        panic!("no sfs");
    };
    let mut root = sfs.open_volume().unwrap().unwrap();

    let memmap_file = root
        .open(
            "memmap.csv",
            FileMode::CreateReadWrite,
            FileAttribute::empty(),
        )
        .unwrap()
        .unwrap();
    let memmap_file = memmap_file.into_type().unwrap().unwrap();
    if let Regular(mut memmap_file) = memmap_file {
        let max_mmap_size = bt.memory_map_size() + 8 * core::mem::size_of::<MemoryDescriptor>();
        let mut mmap_storage = vec![0; max_mmap_size].into_boxed_slice();

        let (_, memmap_iter) = bt.memory_map(&mut mmap_storage[..]).unwrap().unwrap();
        memmap_file
            .write("Index, Type, PhysicalStart, NumberOfPages, Attribute\n".as_bytes())
            .unwrap()
            .unwrap();
        for (i, m) in memmap_iter.enumerate() {
            memmap_file
                .write(
                    format!(
                        "{}, {:?}, {}, {}, {:?}\n",
                        i, m.ty, m.phys_start, m.page_count, m.att
                    )
                    .as_bytes(),
                )
                .unwrap()
                .unwrap();
        }
        memmap_file.close();
    };

    let kernel_file = root
        .open("laranja-kernel", FileMode::Read, FileAttribute::READ_ONLY)
        .unwrap()
        .unwrap();
    let kernel_file = kernel_file.into_type().unwrap().unwrap();
    let mut kernel_file = match kernel_file {
        Regular(f) => f,
        _ => panic!("kernel file is not regular file"),
    };
    const BUF_SIZE: usize = 4000;
    let buf = &mut [0u8; BUF_SIZE];
    let info: &mut FileInfo = kernel_file.get_info(buf).unwrap().unwrap();
    let kernel_file_size = info.file_size();
    let kernel_file_buf = bt
        .allocate_pool(MemoryType::LOADER_DATA, kernel_file_size as usize)
        .unwrap()
        .unwrap();
    let entry_pointer_address: *const u64 = (kernel_file_buf as u64 + 24) as *const u64;
    let kernel_file_buf =
        unsafe { core::slice::from_raw_parts_mut(kernel_file_buf, kernel_file_size as usize) };
    kernel_file.read(kernel_file_buf).unwrap().unwrap();
    kernel_file.close();

    let elf = match Elf::from_bytes(&kernel_file_buf).unwrap() {
        Elf::Elf64(e) => e,
        Elf::Elf32(_) => {
            panic!("Elf32 is not supported");
        }
    };
    let mut kernel_first = u64::max_value();
    let mut kernel_last = u64::min_value();
    for h in elf.program_header_iter() {
        let header = h.ph;
        if matches!(header.ph_type(), ProgramType::LOAD) {
            let v = header.vaddr();
            let len = header.memsz();
            kernel_first = core::cmp::min(kernel_first, v);
            kernel_last = core::cmp::max(kernel_last, v + len);
        }
    }
    let kernel_first = kernel_first as usize / 0x1000 * 0x1000;
    let load_size = kernel_last as usize - kernel_first;
    let n_of_pages = (load_size + 0xfff) / 0x1000;
    writeln!(
        stdout,
        "kernel_first {:x}, last {:x}, pages {:?}",
        kernel_first, kernel_last, n_of_pages
    )
    .unwrap();
    bt.allocate_pages(
        AllocateType::Address(kernel_first),
        MemoryType::LOADER_DATA,
        n_of_pages,
    )
    .unwrap()
    .unwrap();

    // load kernel
    for h in elf.program_header_iter() {
        let header = h.ph;
        if matches!(header.ph_type(), ProgramType::LOAD) {
            let segment = h.segment();
            let dest = header.vaddr();
            let len = header.filesz();
            let dest = unsafe { core::slice::from_raw_parts_mut(dest as *mut u8, len as usize) };
            (0..len as usize).for_each(|i| {
                dest[i] = segment[i];
            });
        }
    }

    let entry_pointer = unsafe { *entry_pointer_address } as *const ();
    let kernel_entry = unsafe {
        core::mem::transmute::<
            *const (),
            extern "sysv64" fn(
                fb: *mut FrameBufferInfo,
                mi: *mut gop::ModeInfo,
                memmap_ptr: *mut MemoryDescriptor,
                memmap_length: usize,
            ) -> (),
        >(entry_pointer)
    };
    let mut mi = gop.current_mode_info();
    let mut fb = gop.frame_buffer();
    let fb_pt = fb.as_mut_ptr();
    let fb_size = fb.size();
    // exit boot service
    let max_mmap_size = bt.memory_map_size() + 8 * core::mem::size_of::<MemoryDescriptor>();
    let mut mmap_storage = vec![0; max_mmap_size].into_boxed_slice();
    let (_st, mmap_iter) = st
        .exit_boot_services(handle, &mut mmap_storage[..])
        .expect_success("Failed to exit boot services");
    let mmap_len = mmap_iter.len();

    for (i, m) in mmap_iter.enumerate() {
        unsafe {
            MMAP_BUF[i] = MaybeUninit::new(*m);
        }
    }

    let mut fb = FrameBufferInfo {
        fb: fb_pt,
        size: fb_size,
    };
    let mmap_ptr = unsafe { MMAP_BUF.as_mut_ptr() as *mut MemoryDescriptor };
    kernel_entry(&mut fb, &mut mi, mmap_ptr, mmap_len);

    uefi::Status::SUCCESS
}

#[alloc_error_handler]
fn out_of_memory(layout: ::core::alloc::Layout) -> ! {
    panic!(
        "Ran out of free memory while trying to allocate {:#?}",
        layout
    );
}

#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    log::error!("{}", info);
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}
