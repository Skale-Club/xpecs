//! Benchmarks in the spirit of AIDA64's cache & memory and CPU tests, plus a
//! stress test. Every test runs on all logical processors unless it says
//! "single", and reports the best of a few passes.

use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn threads() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4)
}

/// Total memory buffer size, partitioned among workers. Cache sizes vary by CPU.
const BUFFER: usize = 512 << 20;
const PASSES: usize = 4;

fn best_of(runs: usize, mut run: impl FnMut() -> f64) -> f64 {
    (0..runs).map(|_| run()).fold(0.0, f64::max)
}

/// Memory read bandwidth in GB/s.
pub fn memory_read() -> f64 {
    let words = BUFFER / 8;
    let buf: Vec<u64> = (0..words as u64).collect();
    let chunk = words.div_ceil(threads());
    best_of(3, || {
        let start = Instant::now();
        std::thread::scope(|s| {
            for part in buf.chunks(chunk) {
                s.spawn(move || {
                    for _ in 0..PASSES {
                        black_box(part.iter().fold(0u64, |a, &b| a.wrapping_add(b)));
                    }
                });
            }
        });
        (BUFFER * PASSES) as f64 / start.elapsed().as_secs_f64() / 1e9
    })
}

/// Memory write bandwidth in GB/s.
pub fn memory_write() -> f64 {
    let words = BUFFER / 8;
    let mut buf = vec![1u64; words];
    let chunk = words.div_ceil(threads());
    best_of(3, || {
        let start = Instant::now();
        std::thread::scope(|s| {
            for (i, part) in buf.chunks_mut(chunk).enumerate() {
                s.spawn(move || {
                    for pass in 0..PASSES {
                        part.fill((i + pass) as u64);
                        black_box(&part[0]);
                    }
                });
            }
        });
        (BUFFER * PASSES) as f64 / start.elapsed().as_secs_f64() / 1e9
    })
}

/// Memory copy bandwidth in GB/s (bytes copied per second).
pub fn memory_copy() -> f64 {
    let words = BUFFER / 2 / 8;
    let src: Vec<u64> = (0..words as u64).collect();
    let mut dst = vec![0u64; words];
    let chunk = words.div_ceil(threads());
    best_of(3, || {
        let start = Instant::now();
        std::thread::scope(|s| {
            for (to, from) in dst.chunks_mut(chunk).zip(src.chunks(chunk)) {
                s.spawn(move || {
                    for _ in 0..PASSES {
                        to.copy_from_slice(from);
                        black_box(&to[0]);
                    }
                });
            }
        });
        (BUFFER / 2 * PASSES) as f64 / start.elapsed().as_secs_f64() / 1e9
    })
}

/// Random-access memory latency in nanoseconds: one pointer chase through a
/// single random cycle over 256 MB.
pub fn memory_latency() -> f64 {
    let n = (256usize << 20) / std::mem::size_of::<usize>();
    let mut next: Vec<usize> = (0..n).collect();
    // Sattolo's shuffle gives one cycle through every slot.
    let mut rng = 0x9E37_79B9_7F4A_7C15u64;
    for i in (1..n).rev() {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        next.swap(i, (rng % i as u64) as usize);
    }
    let steps = 10_000_000;
    let mut best = f64::MAX;
    for _ in 0..3 {
        let mut p = 0usize;
        let start = Instant::now();
        for _ in 0..steps {
            p = next[p];
        }
        black_box(p);
        best = best.min(start.elapsed().as_secs_f64() * 1e9 / steps as f64);
    }
    best
}

fn integer_work(rounds: u64, seed: u64) -> u64 {
    let mut x = seed | 1;
    let mut h = 0xCBF2_9CE4_8422_2325u64;
    for i in 0..rounds {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        h = (h.rotate_left(5) ^ x ^ i).wrapping_mul(0x0100_0000_01B3);
    }
    h
}

/// Mandelbrot iterations over one strip; returns how many iterations ran.
fn float_work(strip: u64) -> u64 {
    const SIZE: u64 = 64;
    let mut iterations = 0u64;
    for py in 0..SIZE {
        for px in 0..SIZE {
            let cx = -2.0 + 2.6 * px as f64 / SIZE as f64 + (strip % 7) as f64 * 1e-9;
            let cy = -1.3 + 2.6 * py as f64 / SIZE as f64;
            let (mut x, mut y, mut i) = (0.0f64, 0.0f64, 0u64);
            while i < 256 && x * x + y * y <= 4.0 {
                let t = x * x - y * y + cx;
                y = 2.0 * x * y + cy;
                x = t;
                i += 1;
            }
            iterations += i;
        }
    }
    black_box(iterations)
}

/// Runs `work` on `n` threads for about a second; returns units per second.
fn throughput(n: usize, work: fn(u64) -> u64) -> f64 {
    let done = AtomicU64::new(0);
    let stop = AtomicBool::new(false);
    let start = Instant::now();
    std::thread::scope(|s| {
        for t in 0..n {
            let (done, stop) = (&done, &stop);
            s.spawn(move || {
                let mut batch = t as u64;
                while !stop.load(Ordering::Relaxed) {
                    done.fetch_add(work(batch), Ordering::Relaxed);
                    batch += 1;
                }
            });
        }
        std::thread::sleep(Duration::from_millis(1200));
        stop.store(true, Ordering::Relaxed);
    });
    done.load(Ordering::Relaxed) as f64 / start.elapsed().as_secs_f64()
}

const INT_BATCH: u64 = 1 << 20;

fn int_batch(seed: u64) -> u64 {
    black_box(integer_work(INT_BATCH, seed));
    INT_BATCH
}

/// Integer throughput in millions of hash rounds per second.
pub fn cpu_integer(all_threads: bool) -> f64 {
    let n = if all_threads { threads() } else { 1 };
    best_of(2, || throughput(n, int_batch) / 1e6)
}

/// Floating point throughput in millions of Mandelbrot iterations per second.
pub fn cpu_float(all_threads: bool) -> f64 {
    let n = if all_threads { threads() } else { 1 };
    best_of(2, || throughput(n, float_work) / 1e6)
}

// ---------- stress test ----------

static STRESS: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

/// Runs one test by id ("mem-read", "cpu-fp-n"...).
pub fn run(test: &str) -> Result<f64, String> {
    Ok(match test {
        "mem-read" => memory_read(),
        "mem-write" => memory_write(),
        "mem-copy" => memory_copy(),
        "mem-latency" => memory_latency(),
        "cpu-int-1" => cpu_integer(false),
        "cpu-int-n" => cpu_integer(true),
        "cpu-fp-1" => cpu_float(false),
        "cpu-fp-n" => cpu_float(true),
        other => return Err(format!("unknown test {other}")),
    })
}

/// Loads every logical processor with mixed integer and floating point work
/// until `stop_stress` is called.
pub fn start_stress() {
    let mut slot = STRESS.lock().unwrap();
    if slot.is_some() {
        return;
    }
    let running = Arc::new(AtomicBool::new(true));
    for t in 0..threads() {
        let running = running.clone();
        std::thread::spawn(move || {
            let mut batch = t as u64;
            while running.load(Ordering::Relaxed) {
                black_box(float_work(batch));
                black_box(integer_work(1 << 16, batch));
                batch += 1;
            }
        });
    }
    *slot = Some(running);
}

pub fn stop_stress() {
    if let Some(running) = STRESS.lock().unwrap().take() {
        running.store(false, Ordering::Relaxed);
    }
}
