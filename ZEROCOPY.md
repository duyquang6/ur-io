# architecture

## overview

```
UDP Socket → io-uring (RecvMsgMulti) → Provided Buffers → mmap → Disk
     ↓              ↓                          ↓            ↓
  kernel      submission queue        kernel-managed    disk write
             completion queue           buffers      (page cache)
```

## features

### zero-alloc

- no allocation per op

### zero-copy

1. **network → kernel buffer**: DMA (hardware copy)
2. **kernel buffer → mmap**: `copy_nonoverlapping` (single memcpy)
3. **mmap → disk**: OS page cache (benefit mmap syscall)

### io-uring

- **recv multishot**: 1 submit - many completions
- **provide buffers**: reused kernel buffer
- **limit syscalls** per packet

## on-disk format

Binary format for maximum efficiency:

```
  0                   1                   2                   3
  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |                                                               |
 +                        Timestamp (64 bits)                    +
 |                                                               |
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |                      Payload Length (32 bits)                 |
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |                                                               |
 +                        Payload Data                           +
 |                         (variable)                            |
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

## comparison with traditional I/O

### traditional

```
┌─────────────────────────────────────────────────────────┐
│                    User Memory                           │
│                                                          │
│  recvfrom(fd, buf, len, 0)                              │
│         ↓                                                │
│  [Application Buffer] ← copy from kernel                │
└─────────────────────────────────────────────────────────┘
                        ↓ (syscall boundary)
┌─────────────────────────────────────────────────────────┐
│                  Kernel Memory                           │
│                                                          │
│  [Socket Buffer] ← copy from NIC                        │
│         ↓                                                │
│  copy to userspace                                       │
└─────────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────────┐
│                    User Memory                           │
│                                                          │
│  write(fd, buf, len)                                     │
│         ↓                                                │
│  [Application Buffer] ← already in memory               │
└─────────────────────────────────────────────────────────┘
                        ↓ (syscall boundary)
┌─────────────────────────────────────────────────────────┐
│                  Kernel Memory                           │
│                                                          │
│  [Page Cache] ← copy from userspace                     │
│         ↓                                                │
│  mark dirty, schedule writeback                          │
└─────────────────────────────────────────────────────────┘
                        ↓ (async writeback)
┌─────────────────────────────────────────────────────────┐
│                      Disk                                │
│                                                          │
│  [Physical Storage] ← written by kernel                 │
└─────────────────────────────────────────────────────────┘

Total: 3 memory copies, 2+ syscalls per packet
```

### udp-log-agent

```
┌─────────────────────────────────────────────────────────┐
│                    User Memory                           │
│                                                          │
│  [Provided Buffer Pool] (1 MB mmap)                     │
│   - Pre-allocated once                                   │
│   - Kernel writes directly here                         │
│         ↓                                                │
│  RecvMsgMulti (one submission, infinite packets)        │
└─────────────────────────────────────────────────────────┘
                        ↑ (shared ring buffers)
┌─────────────────────────────────────────────────────────┐
│                  Kernel Memory                           │
│                                                          │
│  Network packet arrives → DMA → Provided Buffer         │
│  (writes directly to userspace buffer)                   │
│         ↓                                                │
│  Post completion (just metadata)                         │
└─────────────────────────────────────────────────────────┘
                        ↓ (no syscall!)
┌─────────────────────────────────────────────────────────┐
│                    User Memory                           │
│                                                          │
│  [Memory-Mapped Log File] (1 MB mmap)                   │
│   - Backed by disk file                                  │
│   - Kernel manages page cache automatically             │
│         ↓                                                │
│  ptr::copy_nonoverlapping(provided_buf → mmap)          │
└─────────────────────────────────────────────────────────┘
                        ↓ (automatic, no syscall)
┌─────────────────────────────────────────────────────────┐
│                  Kernel Memory                           │
│                                                          │
│  [Page Cache] ← already mapped to mmap                  │
│   - Dirty pages marked automatically                     │
│   - Background writeback (pdflush/kswapd)               │
└─────────────────────────────────────────────────────────┘
                        ↓ (async, kernel-initiated)
┌─────────────────────────────────────────────────────────┐
│                      Disk                                │
│                                                          │
│  [Physical Storage] ← written by kernel threads         │
└─────────────────────────────────────────────────────────┘

Total: 1 memory copy, 0 syscalls per packet (after setup)
```
