use crate::{debug, trace};
mod context;
mod device_manager;
mod registers;
mod simple_alloc;

use registers::*;

use self::simple_alloc::SimpleAlloc;

const MEM_POOL_SIZE: usize = 4 * 1024 * 1024;
static ALLOC: spin::Mutex<simple_alloc::SimpleAlloc<MEM_POOL_SIZE>> =
    spin::Mutex::new(SimpleAlloc::new());

pub struct Controller<'a> {
    cap_regs: &'a mut CapabilityRegisters,
    op_regs: &'a mut OperationalRegisters,
    doorbell_first: *mut Doorbell,
}

impl<'a> Controller<'a> {
    /// # Safety
    /// mmio_base must be a valid base address for xHCI device MMIO
    pub unsafe fn new(mmio_base: usize) -> Self {
        let cap_regs = &mut *(mmio_base as *mut CapabilityRegisters);
        trace!("cap regs: {}", cap_regs);
        let op_regs =
            &mut *((mmio_base + cap_regs.cap_length.read() as usize) as *mut OperationalRegisters);
        let doorbell_first =
            (mmio_base + (cap_regs.db_off.read() & 0xffff_fffc) as usize) as *mut Doorbell;

        op_regs.usbcmd.modify(|usbcmd| {
            usbcmd.set_intterupt_enable(false);
            usbcmd.set_host_system_error_enable(false);
            usbcmd.set_enable_wrap_event(false);
        });
        if !op_regs.usbsts.read().hc_halted() {
            debug!("hc not halted");
            op_regs.usbcmd.modify(|usbcmd| usbcmd.set_run_stop(false));
        }

        while !op_regs.usbsts.read().hc_halted() {}
        trace!("hc halted");

        // reset controller
        trace!(
            "hc reset value; {}",
            op_regs.usbcmd.read().host_controller_reset()
        );
        op_regs.usbcmd.modify(|usbcmd| {
            usbcmd.set_host_controller_reset(true);
        });
        while op_regs.usbcmd.read().host_controller_reset() {}
        trace!("controller reset done.");
        while op_regs.usbsts.read().controller_not_ready() {}
        trace!("controller is ready.");
        let max_slots = cap_regs.hcs_params1.read().max_device_slots();
        debug!("max device slots: {}", max_slots);
        op_regs
            .config
            .modify(|config| config.set_max_device_slots_enabled(max_slots));
        let alloc = ALLOC.lock();

        Controller {
            cap_regs,
            op_regs,
            doorbell_first,
        }
    }
}
