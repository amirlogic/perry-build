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

**Run date:** 2026-04-22 — Perry commit `d81acb5` (v0.5.162).
**Hardware:** Apple M1 Max (10 cores, 64 GB RAM), macOS 26.4.
**Methodology:** best of 5 runs per cell, monotonic clock, no warmup.
All times in milliseconds. Lower is better.

| Benchmark      | Perry |  Rust |   C++ |    Go | Swift |  Java |  Node |   Bun |  Python |
|----------------|-------|-------|-------|-------|-------|-------|-------|-------|---------|
| fibonacci      |   305 |   311 |   308 |   440 |   395 |   279 |   996 |   510 |   15792 |
| loop_overhead  |    32 |    94 |    95 |    95 |    94 |    95 |    52 |    39 |    2929 |
| array_write    |     3 |     6 |     2 |     8 |     2 |     6 |     8 |     4 |     385 |
| array_read     |     3 |     9 |     9 |     9 |     9 |    11 |    13 |    14 |     327 |
| math_intensive |    48 |    46 |    49 |    48 |    47 |    49 |    49 |    49 |    2185 |
| object_create  |     0 |     0 |     0 |     0 |     0 |     4 |     8 |     6 |     157 |
| nested_loops   |     9 |     8 |     8 |     9 |     8 |    10 |    16 |    19 |     458 |
| accumulate     |    97 |    94 |    94 |    95 |    93 |    98 |   583 |    96 |    4854 |

**Known regression** ([#140](https://github.com/PerryTS/perry/issues/140)):
`loop_overhead`, `math_intensive`, and `accumulate` all regressed 2–4x
between v0.5.22 and v0.5.162 after an `asm sideeffect` loop-body barrier
(from #74's fix) and an over-eager i32 shadow counter started blocking
the LLVM vectorizer on pure-accumulator loops. v0.5.22 numbers for those
three cells were 12/14/24 ms respectively — Perry used to beat Rust
3–8x on them. Tracking issue has the IR diff and fix options.

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
wins (`array_read`, `loop_overhead`) vs where it ties (`math_intensive`,
`accumulate`, `nested_loops`) vs where it loses (`object_create`).

## Benchmark-by-benchmark summary

### `loop_overhead` — `sum += 1.0` × 100M
Perry 32 ms vs Rust/C++/Go/Swift/Java ~94 ms. **This entire gap is the
default fast-math setting.** Perry emits `reassoc contract` on f64 ops
because JS/TS `number` semantics can't observe the difference (no
signalling NaNs, no fenv, no strict `-0` rules at the operator level).
Rust/C++/Go/Swift default to strict-IEEE `fadd`, which has a 3-cycle
latency wall and is unreassociable. `g++ -O3 -ffast-math` on the same
`bench.cpp` drops C++ from 96 ms to 11 ms — same LLVM, same pipeline,
one flag. See [RESULTS_OPT.md](./RESULTS_OPT.md) for the per-language
opt-sweep (C++ opt = 12 ms, Rust opt = 24 ms on stable, Go = 99 ms
because Go has no fast-math flag at all).

Under `reassoc`, LLVM's IndVarSimplify recognizes `sum + 1.0 × N` as
integer-valued and rewrites the f64 accumulator to i32 `add` in post-opt.
That's what Perry's 32 ms is measuring — not the f64 fadd chain at all.

This cell used to be 12 ms pre-v0.5.91 because on top of the i32 rewrite
LLVM was also vectorizing into SIMD parallel accumulators. The vectorizer
currently bails on Perry's IR; tracked as [#140](https://github.com/PerryTS/perry/issues/140).

### `math_intensive` — `result += 1.0/i` × 50M
Perry 48 ms, Rust/C++/Go/Swift/Java/Node/Bun all ~46–49 ms — essentially
tied. Same fast-math default story as `loop_overhead` but the reciprocal
divide's 10+ cycle latency makes the single-accumulator serial chain
match the integer rewrite cost-wise. Pre-v0.5.91 Perry was 14 ms here
thanks to vectorization on top of the fast-math rewrite (see [#140](https://github.com/PerryTS/perry/issues/140)).
C++ `-ffast-math` matches the v0.5.22 14 ms exactly per
[RESULTS_OPT.md](./RESULTS_OPT.md).

### `accumulate` — `sum += i % 1000` × 100M
Perry 97 ms, Rust/C++/Go/Swift/Java/Bun all 93–98 ms — tied with the
compiled pack. Node 583 ms is an outlier because V8 doesn't inline the
libm `fmod` call on this pattern. Perry's type analysis still emits
`srem` instead of `fmod` for the mod op (same optimization Node misses),
which is why Perry ties the compiled pack instead of sitting at Node's
583 ms. Pre-v0.5.91 Perry was 24 ms because a vectorized fadd reduction
ran alongside the srem path; same regression as the other two cells,
tracked in [#140](https://github.com/PerryTS/perry/issues/140).

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
