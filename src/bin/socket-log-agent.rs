//! A simple UDP / TCP server that receives packets and dumps to disk no mem-allocations (zerocopy)
//!

use io_uring::{IoUring, opcode, types};
use libc::{AF_INET, SOCK_DGRAM, sockaddr, sockaddr_in};
use libc::{MAP_ANON, MAP_PRIVATE, PROT_READ, PROT_WRITE, iovec, mmap};
use std::net::UdpSocket;
use std::os::fd::{AsRawFd, FromRawFd};

const BUF_SIZE: usize = 4096; // 4 KiB
const BUF_COUNT: usize = 256;

fn allocate_provided_buffers() -> (Vec<iovec>, *mut u8) {
    let total = BUF_SIZE * BUF_COUNT; // 1 MiB

    // create anonymous mmap
    let ptr = unsafe {
        mmap(
            std::ptr::null_mut(),
            total,
            PROT_READ | PROT_WRITE,
            MAP_PRIVATE | MAP_ANON,
            -1,
            0,
        )
    } as *mut u8;

    assert!(!ptr.is_null(), "mmap failed");

    let iovecs: Vec<iovec> = (0..BUF_COUNT)
        .map(|i| iovec {
            iov_base: unsafe { ptr.add(i * BUF_SIZE) } as *mut _,
            iov_len: BUF_SIZE,
        })
        .collect();

    (iovecs, ptr)
}

fn main() -> std::io::Result<()> {
    let sock = UdpSocket::bind("0.0.0.0:9000")?;
    sock.set_nonblocking(true)?;
    let fd = types::Fd(sock.as_raw_fd());

    let mut ring = IoUring::new(256)?;

    let (iovecs, _) = allocate_provided_buffers();
    unsafe {
        ring.submitter().register_buffers(&iovecs)?;
    }

    let recv_e = opcode::RecvMsgMulti::new(fd, std::ptr::null_mut(), 0)
        .build()
        .user_data(1);

    unsafe {
        ring.submission().push(&recv_e).unwrap();
    }
    ring.submit()?;

    // WIP: parse received data, dump to disk

    Ok(())
}
