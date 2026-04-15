// Optimized Rust variant — same algorithms, type choices and loop forms
// aligned with what Perry does by default.
//
// Changes vs bench.rs:
//  - fib:        i32 → i64 (ARM64 native word size; matches Perry's i64
//                inference from TS `number`)
//  - accumulate: f64 sum, `(i % 1000) as f64` → i64 sum, `i % 1000` as i64.
//                Perry's integer-mod fast path emits srem; the default
//                variant in bench.rs calls libm fmod once per iter.
//  - array_write: index loop → `arr.iter_mut().enumerate()`. Rustc elides
//                bounds checks on iterator chains; indexed access does not.
//  - array_read:  index loop → `arr.iter().sum()`. Same reason.
//  - nested_loops: inner loop → `arr[row..row+n].iter().sum()`. Rustc
//                promotes the row slice to a bounds-checked range load
//                once per outer iteration; the inner loop is clean.
//  - loop_overhead, math_intensive: compiled with
//                `RUSTFLAGS=-C llvm-args=-fp-contract=fast` to turn on FMA
//                contraction at LLVM level. This is stable Rust. `reassoc`
//                is not exposed as a stable flag — for a full Perry-
//                equivalent, nightly `std::intrinsics::fadd_fast` would be
//                needed. We use manual unrolling (4 parallel accumulators)
//                as a stable-Rust stand-in for what LLVM would do with
//                reassoc. See the "note" comment in each of those two
//                functions.
//
// Compile:
//   rustc -O -C llvm-args=-fp-contract=fast bench_opt.rs

use std::time::Instant;

fn fib(n: i64) -> i64 {
    if n < 2 {
        return n;
    }
    fib(n - 1) + fib(n - 2)
}

fn bench_fibonacci() {
    let start = Instant::now();
    let result = fib(40);
    let elapsed = start.elapsed().as_millis();
    println!("fibonacci:{}", elapsed);
    println!("  checksum: {}", result);
}

fn bench_loop_overhead() {
    // Manual 4-way unrolling to match what LLVM emits under `reassoc`:
    // four parallel fadd chains, summed at the end. Stable Rust does not
    // expose `reassoc` as a compile flag, so we hand-write the effect.
    let start = Instant::now();
    let mut s0: f64 = 0.0;
    let mut s1: f64 = 0.0;
    let mut s2: f64 = 0.0;
    let mut s3: f64 = 0.0;
    let iters = 100_000_000 / 4;
    for _ in 0..iters {
        s0 += 1.0;
        s1 += 1.0;
        s2 += 1.0;
        s3 += 1.0;
    }
    let sum = s0 + s1 + s2 + s3;
    let elapsed = start.elapsed().as_millis();
    println!("loop_overhead:{}", elapsed);
    println!("  checksum: {:.0}", sum);
}

fn bench_array_write() {
    let mut arr = vec![0.0_f64; 10_000_000];
    let start = Instant::now();
    for (i, slot) in arr.iter_mut().enumerate() {
        *slot = i as f64;
    }
    let elapsed = start.elapsed().as_millis();
    println!("array_write:{}", elapsed);
    println!("  checksum: {:.0}", arr[9_999_999]);
}

fn bench_array_read() {
    let mut arr = vec![0.0_f64; 10_000_000];
    for (i, slot) in arr.iter_mut().enumerate() {
        *slot = i as f64;
    }
    let start = Instant::now();
    let sum: f64 = arr.iter().sum();
    let elapsed = start.elapsed().as_millis();
    println!("array_read:{}", elapsed);
    println!("  checksum: {:.0}", sum);
}

fn bench_math_intensive() {
    // Same 4-way manual unrolling. Each lane computes its own reciprocal
    // sum; combined at the end. Without reassoc this is the only
    // stable-Rust way to break the fadd latency chain.
    let start = Instant::now();
    let mut r0: f64 = 0.0;
    let mut r1: f64 = 0.0;
    let mut r2: f64 = 0.0;
    let mut r3: f64 = 0.0;
    let mut i = 1i64;
    while i + 3 <= 50_000_000 {
        r0 += 1.0 / i as f64;
        r1 += 1.0 / (i + 1) as f64;
        r2 += 1.0 / (i + 2) as f64;
        r3 += 1.0 / (i + 3) as f64;
        i += 4;
    }
    // Handle any remainder (50M is divisible by 4, so in practice none).
    while i <= 50_000_000 {
        r0 += 1.0 / i as f64;
        i += 1;
    }
    let result = r0 + r1 + r2 + r3;
    let elapsed = start.elapsed().as_millis();
    println!("math_intensive:{}", elapsed);
    println!("  checksum: {:.6}", result);
}

struct Point {
    x: f64,
    y: f64,
}

fn bench_object_create() {
    let start = Instant::now();
    let mut sum: f64 = 0.0;
    for i in 0..1_000_000 {
        let p = Point {
            x: i as f64,
            y: i as f64 * 2.0,
        };
        sum += p.x + p.y;
    }
    let elapsed = start.elapsed().as_millis();
    println!("object_create:{}", elapsed);
    println!("  checksum: {:.0}", sum);
}

fn bench_nested_loops() {
    let n = 3000;
    let mut arr = vec![0.0_f64; n * n];
    for (i, slot) in arr.iter_mut().enumerate() {
        *slot = i as f64;
    }
    let start = Instant::now();
    let mut sum: f64 = 0.0;
    for row in arr.chunks_exact(n) {
        sum += row.iter().sum::<f64>();
    }
    let elapsed = start.elapsed().as_millis();
    println!("nested_loops:{}", elapsed);
    println!("  checksum: {:.0}", sum);
}

fn bench_accumulate() {
    let start = Instant::now();
    let mut sum: i64 = 0;
    for i in 0..100_000_000_i64 {
        sum += i % 1000;
    }
    let elapsed = start.elapsed().as_millis();
    println!("accumulate:{}", elapsed);
    println!("  checksum: {}", sum);
}

fn main() {
    bench_fibonacci();
    bench_loop_overhead();
    bench_array_write();
    bench_array_read();
    bench_math_intensive();
    bench_object_create();
    bench_nested_loops();
    bench_accumulate();
}
