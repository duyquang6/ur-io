use io_uring::{IoUring, opcode, squeue::Flags, types};
use std::{fs::File, os::fd::AsRawFd};

fn main() -> std::io::Result<()> {
    let mut ring = IoUring::new(256)?;

    let f = File::open("data.bin")?;
    let fd = types::Fd(f.as_raw_fd());

    let mut buf = vec![0u8; 4096];
    let ts = types::Timespec::new().sec(1);
    let timeout_e = opcode::LinkTimeout::new(&ts).build().user_data(2);

    // build a single read-at-offset SQE
    let read_e = opcode::Read::new(fd, buf.as_mut_ptr(), buf.len() as _)
        .offset(0) // read from file offset 0
        .build()
        .user_data(1)
        .flags(Flags::IO_LINK); // link the timeout to the read

    unsafe {
        let mut sq = ring.submission();
        sq.push(&read_e).unwrap();
        sq.push(&timeout_e).unwrap();
    }
    ring.submit_and_wait(1)?; // block until at least 1 CQE

    for ceq in ring.completion() {
        match ceq.user_data() {
            1 => {
                let nread = ceq.result(); // bytes read or -errno
                if nread < 0 {
                    eprintln!("read error: {}", std::io::Error::from_raw_os_error(-nread));
                    return Ok(());
                }
                println!("read {} bytes: {:02x?}", nread, &buf[..nread as usize]);
                return Ok(());
            }
            2 => {
                println!("timeout");
            }
            _ => panic!("unexpected user data: {}", ceq.user_data()),
        }
    }
    Ok(())
}
