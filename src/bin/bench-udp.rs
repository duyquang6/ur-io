//! UDP benchmark tool to test socket-log-agent performance
//!
//! Sends configurable number of UDP packets and measures throughput

use std::net::UdpSocket;
use std::time::{Duration, Instant};
use std::{env, thread};

const DEFAULT_TARGET: &str = "127.0.0.1:9000";
const DEFAULT_PACKET_COUNT: usize = 100_000;
const DEFAULT_PACKET_SIZE: usize = 512;
const DEFAULT_BATCH_SIZE: usize = 100;

struct BenchConfig {
    target: String,
    packet_count: usize,
    packet_size: usize,
    batch_size: usize,
}

impl BenchConfig {
    fn from_args() -> Self {
        let args: Vec<String> = env::args().collect();

        let target = args
            .get(1)
            .map(|s| s.to_string())
            .unwrap_or_else(|| DEFAULT_TARGET.to_string());

        let packet_count = args
            .get(2)
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_PACKET_COUNT);

        let packet_size = args
            .get(3)
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_PACKET_SIZE);

        let batch_size = args
            .get(4)
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_BATCH_SIZE);

        Self {
            target,
            packet_count,
            packet_size,
            batch_size,
        }
    }
}

struct BenchStats {
    total_packets: usize,
    total_bytes: usize,
    duration: Duration,
    packets_per_sec: f64,
    mbytes_per_sec: f64,
    avg_latency_ns: u64,
}

impl BenchStats {
    fn calculate(packet_count: usize, packet_size: usize, duration: Duration) -> Self {
        let total_bytes = packet_count * packet_size;
        let secs = duration.as_secs_f64();
        let packets_per_sec = packet_count as f64 / secs;
        let mbytes_per_sec = (total_bytes as f64 / secs) / (1024.0 * 1024.0);
        let avg_latency_ns = duration.as_nanos() as u64 / packet_count as u64;

        Self {
            total_packets: packet_count,
            total_bytes,
            duration,
            packets_per_sec,
            mbytes_per_sec,
            avg_latency_ns,
        }
    }

    fn print(&self) {
        println!("\n╔════════════════════════════════════════════════╗");
        println!("║           Benchmark Results                    ║");
        println!("╠════════════════════════════════════════════════╣");
        println!(
            "║ Total packets:    {:>12}                 ║",
            self.total_packets
        );
        println!(
            "║ Total bytes:      {:>12}                 ║",
            self.total_bytes
        );
        println!(
            "║ Duration:         {:>9.2?}                  ║",
            self.duration
        );
        println!(
            "║ Throughput:       {:>12.2} pkt/s          ║",
            self.packets_per_sec
        );
        println!(
            "║ Bandwidth:        {:>12.2} MB/s           ║",
            self.mbytes_per_sec
        );
        println!(
            "║ Avg latency:      {:>12} ns            ║",
            self.avg_latency_ns
        );
        println!("╚════════════════════════════════════════════════╝\n");
    }
}

fn run_benchmark(config: &BenchConfig) -> std::io::Result<BenchStats> {
    println!("╔════════════════════════════════════════════════╗");
    println!("║         UDP Benchmark Configuration            ║");
    println!("╠════════════════════════════════════════════════╣");
    println!("║ Target:           {:<28} ║", config.target);
    println!("║ Packet count:     {:<28} ║", config.packet_count);
    println!(
        "║ Packet size:      {:<28} ║",
        format!("{} bytes", config.packet_size)
    );
    println!("║ Batch size:       {:<28} ║", config.batch_size);
    println!("╚════════════════════════════════════════════════╝\n");

    // Create socket
    let sock = UdpSocket::bind("0.0.0.0:0")?;
    sock.connect(&config.target)?;
    println!("✓ Connected to {}", config.target);

    // Create payload
    let payload = vec![b'X'; config.packet_size];
    println!("✓ Generated {} byte payload\n", config.packet_size);

    // Warm-up
    println!("Warming up...");
    for _ in 0..100 {
        sock.send(&payload)?;
    }
    thread::sleep(Duration::from_millis(100));
    println!("✓ Warm-up complete\n");

    // Run benchmark
    println!("Starting benchmark...");
    let start = Instant::now();
    let mut sent = 0;

    for i in 0..config.packet_count {
        sock.send(&payload)?;
        sent += 1;

        // Print progress every batch
        if (i + 1) % config.batch_size == 0 {
            let elapsed = start.elapsed().as_secs_f64();
            let rate = sent as f64 / elapsed;
            print!("\r[{}/{}] {:.2} pkt/s", sent, config.packet_count, rate);
        }
    }

    let duration = start.elapsed();
    println!("\n✓ Benchmark complete\n");

    Ok(BenchStats::calculate(
        config.packet_count,
        config.packet_size,
        duration,
    ))
}

fn main() -> std::io::Result<()> {
    println!("\n🚀 UDP Benchmark Tool\n");

    let config = BenchConfig::from_args();

    match run_benchmark(&config) {
        Ok(stats) => {
            stats.print();

            // Performance assessment
            if stats.packets_per_sec > 100_000.0 {
                println!("🎉 Excellent! >100K pkt/s");
            } else if stats.packets_per_sec > 50_000.0 {
                println!("✅ Great! >50K pkt/s");
            } else if stats.packets_per_sec > 10_000.0 {
                println!("👍 Good! >10K pkt/s");
            } else {
                println!("⚠️  Consider tuning - <10K pkt/s");
            }

            Ok(())
        }
        Err(e) => {
            eprintln!("❌ Benchmark failed: {}", e);
            eprintln!("\nMake sure socket-log-agent is running:");
            eprintln!("  cargo run --bin socket-log-agent");
            Err(e)
        }
    }
}
