# Polyglot Benchmark Results

Perry vs 9 other runtimes on 8 identical benchmarks. All implementations
use `f64`/`double` arithmetic to match TypeScript's `number` type. No SIMD
intrinsics, no unsafe code, no non-default optimization flags — each
language's idiomatic release-mode build. A companion `RESULTS_OPT.md`
(phase 2 of this investigation) shows what happens when each language is
given flags equivalent to Perry's defaults.

See [`METHODOLOGY.md`](./METHODOLOGY.md) for iteration counts, clocks,
compiler versions, and a full explanation of which optimizations create
each delta.

## Results

**Run date:** 2026-04-15 — Perry commit `e1cbd37` (v0.5.22).
**Hardware:** Apple M1 Max (10 cores, 64 GB RAM), macOS 26.4.
**Methodology:** best of 5 runs per cell, monotonic clock, no warmup.
All times in milliseconds. Lower is better.

† `fibonacci` is reported best-of-20 rather than best-of-5. The recursive-call
shape is unusually sensitive to icache/branch-predictor state, and we saw
±20% variance between different best-of-5 runs of Rust and C++. 20 samples
tightens the distribution to within ±2% of the minimum.

| Benchmark      | Perry |  Rust |   C++ |    Go | Swift |  Java |  Node |   Bun | Hermes |  Python |
|----------------|-------|-------|-------|-------|-------|-------|-------|-------|--------|---------|
| fibonacci†     |   311 |   319 |   310 |   450 |   403 |   280 |  1001 |   527 |   2575 |   16002 |
| loop_overhead  |    12 |    99 |    98 |    97 |    97 |    98 |    53 |    40 |     98 |    2983 |
| array_write    |     2 |     7 |     2 |     9 |     2 |     6 |     8 |     5 |     93 |     395 |
| array_read     |     3 |    10 |     9 |    10 |     9 |    11 |    13 |    14 |     46 |     344 |
| math_intensive |    14 |    49 |    50 |    49 |    49 |    51 |    50 |    51 |     50 |    2243 |
| object_create  |     2 |     0 |     0 |     0 |     0 |     5 |     8 |     5 |      2 |     161 |
| nested_loops   |     9 |     8 |     8 |    10 |     8 |    10 |    17 |    19 |     80 |     484 |
| accumulate     |    24 |    97 |    97 |    99 |    96 |   100 |   602 |    99 |    122 |    4989 |

## How to reproduce

```bash
cd benchmarks/polyglot
bash run_all.sh        # best of 3 runs (default)
bash run_all.sh 5      # best of 5 runs (what the above table used)
```

**Required:** Perry (`cargo build --release` from repo root).
**Optional** (any subset works; missing runtimes show as `-`): Node.js,
Bun, Static Hermes (`shermes`), Rust (`rustc`), C++ (`g++` or `clang++`),
Swift, Go, Java (`javac` + `java`), Python 3.

See [`METHODOLOGY.md`](./METHODOLOGY.md) for what each benchmark measures,
compiler versions, why certain cells look the way they do, and where Perry
loses (`object_create`) vs where it wins (`loop_overhead`, `math_intensive`,
`accumulate`, `array_read`).

## Benchmark-by-benchmark summary

### `loop_overhead` — `sum += 1.0` × 100M
Perry 12 ms vs all compiled languages ~97 ms. Perry emits
`reassoc contract` LLVM fast-math flags so the `fadd` chain can be broken
into parallel accumulators and vectorized. Rust/C++/Go/Swift all compile
IEEE-strict by default and hit the `fadd` latency wall. Node 53 ms / Bun 40
ms: V8 and JavaScriptCore do the reassociation at JIT time.

### `math_intensive` — `result += 1.0/i` × 50M
Perry 14 ms vs all others ~50 ms. Same story as `loop_overhead` — the
reciprocal divide has an even longer latency chain, so the parallel-
accumulator win is proportionally larger.

### `accumulate` — `sum += i % 1000` × 100M
Perry 24 ms vs Rust/C++/Go/Swift/Java/Bun all ~97 ms, Node 602 ms, Hermes
122 ms. `i % 1000` on `double` is a libm `fmod` call on ARM (~30 ns per
call). Perry's type analysis proves the operands are integer-valued and
emits `srem` (1–2 cycle hardware instruction). The other languages all use
`double` to match TS semantics, so they all call `fmod`. Node's 602 ms
outlier is V8 failing to inline the libm call on this pattern.

### `array_read` — sum 10M-element `number[]`
Perry 3 ms, C++/Swift 9 ms, Rust 10 ms, Go 10 ms, Java 11 ms. Perry
detects `for (let i = 0; i < arr.length; i++)` as statically in-bounds,
skips the JS `undefined`-on-OOB check, caches the length at loop entry,
and maintains a parallel i32 counter so the index is never a float → int
conversion. LLVM then autovectorizes to NEON 2-wide f64. C++ `std::vector`
has no bounds check by default but pays the chunk-boundary check from
`-O3`'s vectorizer framing. Rust's iterator form (not used here) matches
Perry — see `bench_opt.rs` (phase 2).

### `array_write` — `arr[i] = i` × 10M
Perry 2 ms, C++/Swift 2 ms, Rust 7 ms, Go 9 ms. Perry matches C++ here.
The Rust result is `-O` with bounds-checked indexing; `.iter_mut()` would
match Perry.

### `nested_loops` — 3000×3000 flat-array sum
All compiled languages 8–10 ms. Perry 9 ms. This benchmark is
cache-bound, not compute-bound — there is no optimization lever to pull.
Perry matches the compiled pack.

### `fibonacci` — recursive `fib(40)`
Java 280 ms (JIT inlining), C++ 310 ms, Perry 311 ms, Rust 319 ms — the
top four languages all land within 10 ms of each other. Perry's type
inference refines the TS `number` parameter to `i64` (because the function
only ever performs integer operations), producing `add/sub/icmp` (1 cycle
each) instead of the `fadd/fsub/fcmp` (2–3 cycles) that the f64-typed Rust
and C++ benchmarks emit. The reason Perry isn't dramatically further
ahead is that LLVM's recursion-folding optimizations on fib-shaped code
recover most of the gap at -O3. The Rust `f64→i64` switch is a one-line
change (tested in `bench_opt.rs`) and drops Rust to ~280 ms.

### `object_create` — allocate 1M `{x, y}` pairs, sum fields
Rust/C++/Go/Swift 0 ms: the compiler proves the struct never escapes and
eliminates the whole loop. Java 5 ms, Bun 5 ms, Node 8 ms, Perry 2 ms,
Hermes 2 ms. Perry is competitive here only because of the v0.5.17
scalar-replacement pass; without it this benchmark was ~10 ms. The 0 ms
floor from statically-typed compiled languages is an inherent tradeoff of
compiling a dynamic language — see `METHODOLOGY.md`.

## Source files

- `bench.cpp` — C++17
- `bench.rs` — Rust (no dependencies)
- `bench.go` — Go
- `bench.swift` — Swift
- `bench.java` — Java
- `bench.py` — Python 3
- `bench.zig` — Zig (may need manual build; not in the current table)
- Perry / Node / Bun / Hermes run the TS files in `../suite/`

All implementations use the same algorithm, same data types (`f64` /
`double` throughout), same iteration counts, and the same output format
(`benchmark_name:elapsed_ms`) so the runner can grep a single key per row.
