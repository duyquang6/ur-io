//! A simple UDP / TCP server that receives packets and dumps to disk without any mem-allocations
//!
//! TODOs:
//! - [ ] Implement log rotation
//! - [ ] Add compression support
//! - [ ] Use `io-uring` for disk writes too (WRITE operations)
//! - [ ] Support multiple buffer groups
//! - [ ] Add metrics and monitoring

use std::fs::OpenOptions;
use std::mem::MaybeUninit;
use std::net::UdpSocket;
use std::os::fd::AsRawFd;
use std::ptr;
use std::slice;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use io_uring::{IoUring, opcode, squeue, types};
use memmap2::MmapMut;

use libc::{MAP_ANON, MAP_PRIVATE, PROT_READ, PROT_WRITE, mmap};

const PORT: &str = "0.0.0.0:9000";
const QUEUE_DEPTH: u32 = 256;
const BUF_SIZE: usize = 2048; // 2 KiB per buffer
const BUF_COUNT: usize = 512; // Total: 1 MiB
const BUF_GROUP_ID: u16 = 1;
const LOG_FILE: &str = "logs.bin";
const LOG_MMAP_SIZE: usize = 1 * 1024 * 1024; // 1 MB

/// Log record representation
struct LogEntry<'a> {
    timestamp: u64,
    payload: &'a [u8],
}

impl<'a> LogEntry<'a> {
    /// Extract log entry from raw packet data
    fn extract(packet: &'a [u8]) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        LogEntry {
            timestamp,
            payload: packet,
        }
    }
}

struct MmappedLogStorage {
    mmap: MmapMut,
    offset: AtomicUsize,
}

impl MmappedLogStorage {
    fn new(path: &str, size: usize) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;

        file.set_len(size as u64)?;
        let mmap = unsafe { MmapMut::map_mut(&file)? };

        Ok(Self {
            mmap,
            offset: AtomicUsize::new(0),
        })
    }

    /// Write log entry directly to mmap (zero-copy from network buffer)
    /// TODO: make this async, use io-uring, compression
    fn write_entry(&self, entry: LogEntry) -> std::io::Result<()> {
        let entry_size = 8 + 4 + entry.payload.len();
        let offset = self.offset.fetch_add(entry_size, Ordering::Relaxed);

        if offset + entry_size > self.mmap.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::OutOfMemory,
                "Log file full",
            ));
        }

        // Log Frame Format:
        //  0                   1                   2                   3
        //  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
        // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        // |                                                               |
        // +                        Timestamp (64 bits)                    +
        // |                                                               |
        // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        // |                      Payload Length (32 bits)                 |
        // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        // |                                                               |
        // +                        Payload Data                           +
        // |                         (variable)                            |
        // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+

        unsafe {
            let base = self.mmap.as_ptr().add(offset) as *mut u8;

            // Write timestamp (8 bytes)
            ptr::copy_nonoverlapping(&entry.timestamp as *const u64 as *const u8, base, 8);

            // Write length (4 bytes)
            let len = entry.payload.len() as u32;
            ptr::copy_nonoverlapping(&len as *const u32 as *const u8, base.add(8), 4);

            // Write data (zero-copy from network buffer to mmap)
            ptr::copy_nonoverlapping(entry.payload.as_ptr(), base.add(12), entry.payload.len());
        }

        Ok(())
    }

    /// Get total bytes written to the log file
    fn bytes_written(&self) -> usize {
        self.offset.load(Ordering::Relaxed)
    }
}

// Helper functions

/// Allocate buffer pool using mmap for io-uring provided buffers
fn allocate_provided_buffers() -> *mut u8 {
    let total = BUF_SIZE * BUF_COUNT;

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
    ptr
}

fn main() -> std::io::Result<()> {
    let sock = UdpSocket::bind(PORT)?;
    sock.set_nonblocking(true)?;
    let fd = types::Fd(sock.as_raw_fd());

    let mut ring = IoUring::new(QUEUE_DEPTH)?;

    let base = allocate_provided_buffers();

    let provide = opcode::ProvideBuffers::new(
        base as *mut _,
        BUF_SIZE as _,
        BUF_COUNT as u16,
        BUF_GROUP_ID,
        0,
    )
    .build()
    .user_data(1);

    unsafe {
        ring.submission().push(&provide).unwrap();
    }
    ring.submit_and_wait(1)?;

    if let Some(cqe) = ring.completion().next() {
        if cqe.result() < 0 {
            panic!("provide_buffers failed: {}", cqe.result());
        }
        println!("Provided {} buffers to kernel", BUF_COUNT);
    }

    let storage = MmappedLogStorage::new(LOG_FILE, LOG_MMAP_SIZE)?;
    println!("Zero-copy log storage initialized: {}", LOG_FILE);

    // Submit multishot recvmsg
    let mut msghdr: libc::msghdr = unsafe { MaybeUninit::zeroed().assume_init() };

    let recv_e = opcode::RecvMsgMulti::new(fd, &mut msghdr, BUF_GROUP_ID)
        .build()
        .flags(squeue::Flags::BUFFER_SELECT)
        .user_data(2);

    unsafe {
        ring.submission().push(&recv_e).unwrap();
    }
    ring.submit()?;

    println!("Listening on {PORT} (multishot + provided buffers; bgid={BUF_GROUP_ID})");

    // Main event loop
    let mut packet_count = 0u64;
    let start = SystemTime::now();

    loop {
        ring.submit_and_wait(1)?;

        while let Some(cqe) = ring.completion().next() {
            let res = cqe.result();

            if res <= 0 {
                eprintln!("recv error: {}", std::io::Error::from_raw_os_error(-res));
                continue;
            }

            let buf_id = ((cqe.flags() >> 16) & 0xFFFF) as usize;

            // get packet data from provided buffer
            let ptr_packet = unsafe { base.add(buf_id * BUF_SIZE) };
            let packet_data = unsafe { slice::from_raw_parts(ptr_packet, res as usize) };
            let out = types::RecvMsgOut::parse(packet_data, &msghdr).unwrap();
            let payload = out.payload_data();

            // extract
            let log_entry = LogEntry::extract(payload);

            // transform

            // load to disk
            if let Err(e) = storage.write_entry(log_entry) {
                eprintln!("Storage error: {}", e);
                break;
            }

            // stats
            packet_count += 1;
            let elapsed = start.elapsed().unwrap().as_secs_f64();
            let rate = packet_count as f64 / elapsed;
            println!(
                "Received {} packets ({:.2} pkt/s), written {} bytes",
                packet_count,
                rate,
                storage.bytes_written()
            );
        }
    }
}
