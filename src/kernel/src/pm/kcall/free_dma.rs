// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

use crate::{
    hal::mem::{
        Address,
        PageAligned,
        VirtualAddress,
    },
    kcall::KcallResult,
    mm::VirtMemoryManager,
    pm::ProcessManager,
};
use ::sys::{
    error::{
        Error,
        ErrorCode,
    },
    pm::{
        Capability,
        ProcessIdentifier,
    },
};

fn do_free_dma(
    pm: &mut ProcessManager,
    mm: &mut VirtMemoryManager,
    pid: ProcessIdentifier,
    vaddr: PageAligned<VirtualAddress>,
    nframes: usize,
) -> Result<(), Error> {
    pm.free_dma(mm, pid, vaddr, nframes)
}

pub fn free_dma(caller_pid: ProcessIdentifier, arg0: u32, arg1: u32) -> KcallResult {
    let pm: &mut ProcessManager = unsafe { ProcessManager::get_mut() };
    let mm: &mut VirtMemoryManager = unsafe { VirtMemoryManager::get_mut() };

    let vaddr: PageAligned<VirtualAddress> = match PageAligned::from_raw_value(arg0 as usize) {
        Ok(vaddr) => vaddr,
        Err(e) => return KcallResult::Error(e.code.into()),
    };
    let nframes: usize = arg1 as usize;

    match pm.has_capability(caller_pid, Capability::IoManagement) {
        Ok(true) => (),
        Ok(false) => {
            let reason: &str = "process does not have IO management capabilities";
            error!("{reason}");
            return KcallResult::Error(ErrorCode::PermissionDenied.into());
        },
        Err(e) => return KcallResult::Error(e.code.into()),
    }

    match do_free_dma(pm, mm, caller_pid, vaddr, nframes) {
        Ok(_) => KcallResult::ok(),
        Err(e) => KcallResult::Error(e.code.into()),
    }
}
