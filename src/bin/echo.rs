use io_uring::{IoUring, opcode, types};
use std::net::TcpListener;
use std::os::fd::AsRawFd;

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", 9000))?;
    listener.set_nonblocking(true)?;
    let lfd = types::Fd(listener.as_raw_fd());

    let mut ring = IoUring::new(256)?;
    let mut buf = vec![0u8; 4096];

    loop {
        let accept_e = opcode::Accept::new(lfd, std::ptr::null_mut(), std::ptr::null_mut())
            .build()
            .user_data(10);

        unsafe {
            ring.submission().push(&accept_e).unwrap();
        }
        ring.submit_and_wait(1)?;

        let cqe = ring.completion().next().unwrap();
        if cqe.result() < 0 {
            eprintln!(
                "accept error: {}",
                std::io::Error::from_raw_os_error(-cqe.result())
            );
            return Ok(());
        }
        let conn_fd = types::Fd(cqe.result());

        // recv
        let recv_e = opcode::Recv::new(conn_fd, buf.as_mut_ptr(), buf.len() as _)
            .build()
            .user_data(11);

        unsafe {
            ring.submission().push(&recv_e).unwrap();
        }
        ring.submit_and_wait(1)?;

        let cqe = ring.completion().next().unwrap();
        let n = cqe.result();
        if n <= 0 {
            return Ok(());
        }

        // send back
        let send_e = opcode::Send::new(conn_fd, buf.as_ptr(), n as _)
            .build()
            .user_data(12);

        unsafe {
            ring.submission().push(&send_e).unwrap();
        }
        ring.submit_and_wait(1)?;

        println!("echoed {} bytes", n);
    }
}
