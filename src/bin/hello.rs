use io_uring::{IoUring, opcode, types};
use std::{fs::File, os::fd::AsRawFd};

fn main() -> std::io::Result<()> {
    let mut ring = IoUring::new(256)?;

    let f = File::open("data.bin")?;
    let fd = types::Fd(f.as_raw_fd());

    let mut buf = vec![0u8; 4096];

    // build a single read-at-offset SQE
    let read_e = opcode::Read::new(fd, buf.as_mut_ptr(), buf.len() as _)
        .offset(0) // read from file offset 0
        .build()
        .user_data(0xdeadbeef);

    // submit
    unsafe {
        ring.submission().push(&read_e).unwrap();
    }
    ring.submit_and_wait(1)?; // block until at least 1 CQE

    let cqe = ring.completion().next().expect("CQE missing");
    assert_eq!(cqe.user_data(), 0xdeadbeef);
    let nread = cqe.result(); // bytes read or -errno
    if nread < 0 {
        eprintln!("read error: {}", std::io::Error::from_raw_os_error(-nread));
        return Ok(());
    }
    println!("read {} bytes: {:02x?}", nread, &buf[..nread as usize]);
    Ok(())
}
