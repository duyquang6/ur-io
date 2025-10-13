//! A simple UDP server that receives packets and dumps to disk no mem-allocations (zerocopy)
//!

use io_uring::{IoUring, opcode, types};
use libc::{AF_INET, SOCK_DGRAM, sockaddr, sockaddr_in};
use std::net::UdpSocket;
use std::os::fd::{AsRawFd, FromRawFd};

fn main() -> std::io::Result<()> {
    let sock = UdpSocket::bind("0.0.0.0:9000")?;
    sock.set_nonblocking(true)?;
    let fd = types::Fd(sock.as_raw_fd());

    let mut ring = IoUring::new(256)?;

    Ok(())
}
