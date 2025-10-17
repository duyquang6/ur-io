# ur-io

experiments with `io-uring` in Rust - exploring high-performance asynchronous I/O on Linux.

## overview

- **hello**: Basic file reading with io-uring
- **echo**: TCP echo server using io-uring operations
- **hello-buffer**: Zero-copy I/O with registered buffers
- **hello-timeout**: Operation chaining with timeouts
- **socket-log-agent**: Zero-copy UDP log collector with RecvMsgMulti + mmap

## requirements

- linux kernel 6.0+

## quick start

create a test file and run any example:

```bash
echo "Hello, io-uring!" > data.bin
cargo run --bin hello
```

### zero-copy log collector

```bash
cargo run --bin socket-log-agent

# send to log-agent
echo "log message 1" | nc -u 127.0.0.1 9000
echo "log message 2" | nc -u 127.0.0.1 9000

# read log
cargo run --bin read-logs logs.bin
```
