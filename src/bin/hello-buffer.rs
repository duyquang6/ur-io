use io_uring::{IoUring, opcode, types};
use libc::iovec;
use std::{fs::File, os::fd::AsRawFd};

fn main() -> std::io::Result<()> {
    let mut ring = IoUring::new(256)?;

    let mut backing = vec![0u8; 1 << 20]; // 1 MiB
    let mut iov = iovec {
        iov_base: backing.as_mut_ptr() as *mut _,
        iov_len: backing.len(),
    };

    // register buffer as iovecs
    unsafe {
        ring.submitter()
            .register_buffers(std::slice::from_mut(&mut iov))?;
    }

    // open file and do READ_FIXED into slot 0
    let f = File::open("data.bin")?;
    let fd = types::Fd(f.as_raw_fd());

    let e = opcode::ReadFixed::new(fd, backing.as_mut_ptr(), 4096, 0) // buf ptr only for bounds; kernel uses index
        .offset(0)
        .build()
        .user_data(1);

    unsafe {
        ring.submission().push(&e).unwrap();
    }
    ring.submit_and_wait(1)?;

    let cqe = ring.completion().next().unwrap();
    let n = cqe.result();
    println!("read_fixed got {} bytes", n);

    // optional: unregister when done
    ring.submitter().unregister_buffers()?;
    Ok(())
}
