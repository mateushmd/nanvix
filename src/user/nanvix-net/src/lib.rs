#![no_std]

extern crate alloc;

#[cfg(test)]
extern crate std;

pub mod e1000;
pub mod e1000_for_nanvix;
pub mod pci_kernel;

#[cfg(feature = "syscall-time")]
use ::smoltcp::time::Instant;
#[cfg(feature = "syscall-time")]
use ::sys::error::Error;
#[cfg(feature = "syscall-time")]
use ::syscall::safe::time::Time;

pub use self::e1000::{
    E1000Device,
    E1000RxToken,
    E1000TxToken,
    KernelFunctions,
    RxDesc,
    TxDesc,
};

#[cfg(feature = "syscall-time")]
pub fn now_instant() -> Result<Instant, Error> {
    let now = Time::now()?;
    let millis = now.seconds() * 1000 + (now.nanoseconds() as u64 / 1_000_000);
    Ok(Instant::from_millis(millis as i64))
}
