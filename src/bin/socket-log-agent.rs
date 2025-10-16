//! A simple UDP / TCP server that receives packets and dumps to disk no mem-allocations (zerocopy)
//!

use io_uring::{IoUring, opcode, squeue, types};
use libc::{MAP_ANON, MAP_PRIVATE, PROT_READ, PROT_WRITE, mmap};
use std::net::UdpSocket;
use std::os::fd::AsRawFd;
use std::slice;

const PORT: &str = "0.0.0.0:9000";
const QUEUE_DEPTH: u32 = 256;
const BUF_SIZE: usize = 2048; // 2 KiB
const BUF_COUNT: usize = 512;
const BUF_GROUP_ID: u16 = 1;

fn allocate_provided_buffers() -> *mut u8 {
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

    return ptr;
}

fn main() -> std::io::Result<()> {
    let sock = UdpSocket::bind(PORT)?;
    sock.set_nonblocking(true)?;

    let fd = types::Fd(sock.as_raw_fd());

    let mut ring = IoUring::new(QUEUE_DEPTH)?;

    let base = allocate_provided_buffers();

    let provide = opcode::ProvideBuffers::new(
        base as *mut _,   // addr
        BUF_SIZE as _,    // len of each buffer
        BUF_COUNT as u16, // nr buffers
        BUF_GROUP_ID,     // buffer group id
        0,                // starting bid (buffer id)
    )
    .build()
    .user_data(1);

    unsafe {
        let mut sq = ring.submission();
        sq.push(&provide).unwrap();
    }
    ring.submit_and_wait(1)?;
    if let Some(cqe) = ring.completion().next() {
        if cqe.result() < 0 {
            panic!("provide_buffers failed: {}", cqe.result());
        }
        println!("provide_buffers completed");
    }

    let mut msghdr: libc::msghdr = unsafe { std::mem::MaybeUninit::zeroed().assume_init() };

    //
    let recv_e = opcode::RecvMsgMulti::new(fd, &mut msghdr, BUF_GROUP_ID)
        .build()
        .flags(squeue::Flags::BUFFER_SELECT)
        .user_data(2);

    unsafe {
        ring.submission().push(&recv_e).unwrap();
    }
    ring.submit()?;

    println!("listening on {PORT} (multishot + provided buffers; bgid={BUF_GROUP_ID})");

    loop {
        // TODO: this to avoid busy loop only, need better solution to handle many CQEs
        ring.submit_and_wait(1)?;
        while let Some(cqe) = ring.completion().next() {
            let res = cqe.result();
            if res <= 0 {
                eprintln!("recv error: {}", std::io::Error::from_raw_os_error(-res));
                continue;
            }

            let buf_id = ((cqe.flags() >> 16) & 0xFFFF) as usize;

            // Address of the used buffer = base + buf_id * BUF_SIZE
            // Unlike [`RecvMsg`], this multishot recvmsg will prepend a struct which describes the layout
            // of the rest of the buffer in combination with the initial msghdr structure submitted with
            // the request. Use [`types::RecvMsgOut`] to parse the data received and access its
            // components.
            let ptr_packet = unsafe { base.add(buf_id * BUF_SIZE) };
            let packet_data = unsafe { slice::from_raw_parts(ptr_packet, res as usize) };

            let out = types::RecvMsgOut::parse(packet_data, &msghdr).unwrap();

            parse_log(out.payload_data());

            // TODO: dump to disk (or send to syslog server)
        }
    }
}

fn parse_log(data: &[u8]) {
    println!("parsed log: {:?}", String::from_utf8_lossy(data));
}
