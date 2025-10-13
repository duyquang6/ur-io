# ur-io

experiments with `io-uring` in Rust - exploring high-performance asynchronous I/O on Linux.

## overview

- **hello**: Basic file reading with io-uring
- **echo**: TCP echo server using io-uring operations
- **hello-buffer**: Zero-copy I/O with registered buffers
- **hello-timeout**: Operation chaining with timeouts
- **udp-log-agent**: UDP log-agent collector (WIP)

## requirements

- linux kernel 6.0+

## quick Start

create a test file and run any example:

```bash
echo "Hello, io-uring!" > data.bin
cargo run --bin hello
```
