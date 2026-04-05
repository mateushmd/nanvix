extern crate alloc;

use sys::error::Error;
use syscall::safe::time::Time;
use smoltcp::{
    phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken},
    iface::{Config, Interface, SocketSet},
    wire::{EthernetAddress, IpAddress, IpCidr, Ipv4Address},
    socket::tcp,
    time::Instant,
};

// Not sure, got from Valedo's tcp.rs file...
const IP: &str = "10.0.2.15"; // QEMU user networking default IP
const GATEWAY: &str = "10.0.2.2"; // QEMU user networking gateway
const PORT: u16 = 5555;

pub fn now_instant() -> Result<Instant, Error> {
    let now = Time::now()?;
    let millis = now.seconds() * 1000 + (now.nanoseconds() as u64 / 1_000_000);
    Ok(Instant::from_millis(millis as i64))
}

/*
#[cfg(test)]
mod tests {
}
*/
