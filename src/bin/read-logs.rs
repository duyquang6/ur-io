//! Utility to read zero-copy logs written by socket-log-agent
//!
//! Log Frame Format:
//!  0                   1                   2                   3
//!  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |                                                               |
//! +                        Timestamp (64 bits)                    +
//! |                                                               |
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |                      Payload Length (32 bits)                 |
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |                                                               |
//! +                        Payload Data                           +
//! |                         (variable)                            |
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+

use memmap2::Mmap;
use std::fs::File;
use std::ptr;

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let log_file = if args.len() > 1 { &args[1] } else { "logs.bin" };

    let file = File::open(log_file)?;
    let mmap = unsafe { Mmap::map(&file)? };

    println!("Reading logs from: {}", log_file);
    println!("File size: {} bytes\n", mmap.len());

    let mut offset = 0;
    let mut count = 0;

    while offset + 12 <= mmap.len() {
        // Read timestamp (8 bytes)
        let timestamp = unsafe { ptr::read_unaligned(mmap.as_ptr().add(offset) as *const u64) };

        // Read length (4 bytes)
        let len =
            unsafe { ptr::read_unaligned(mmap.as_ptr().add(offset + 8) as *const u32) } as usize;

        if len == 0 || offset + 12 + len > mmap.len() {
            break; // End of logs or corrupted entry
        }

        // Read data
        let data = &mmap[offset + 12..offset + 12 + len];

        count += 1;
        println!("[{}] ts={} len={}", count, timestamp, len);
        println!("  {}", String::from_utf8_lossy(data));

        offset += 12 + len;
    }

    println!("\nTotal entries: {}", count);
    println!("Bytes read: {}", offset);

    Ok(())
}
