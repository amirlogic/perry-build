# Polyglot Benchmark Methodology

Last updated: 2026-04-15 — Perry commit `e1cbd37`.

This document describes how the polyglot benchmark suite is constructed and
run, what each benchmark measures, and why Perry's numbers differ from the
other languages. It is the companion to [`RESULTS.md`](./RESULTS.md).

## What this suite is (and isn't)

Eight compute-bound microbenchmarks, implemented identically in 10 runtimes.
Each benchmark runs for 0.1–15 seconds depending on the language. Best of 5
runs per (benchmark, language) pair is reported.

**This suite measures:** loop iteration throughput, arithmetic latency,
sequential array access, recursive call overhead, object allocation
patterns, and integer-modulo performance on f64-typed code.

**This suite does not measure:** startup time, allocator throughput under
mixed workloads, GC pressure, I/O, async/await, JIT warmup behavior, memory
locality across realistic working sets, or anything a real application
spends most of its time on. Do not extrapolate these numbers to "language X
is N× faster than language Y on real workloads." They are a probe into
specific compiler choices, not a general benchmark.

## Hardware

Apple M1 Max (10 cores: 8P + 2E), 64 GB RAM, macOS 26.4. All benchmarks
run on performance cores via default scheduling — no explicit affinity
pinning, no `taskset`, no thermal throttle mitigation beyond best-of-N.

## Compiler / runtime versions

Captured at the time of the last results refresh. See `RESULTS.md` for the
date of the run being reported.

| Runtime       | Version                                      | Invocation                        |
|---------------|----------------------------------------------|-----------------------------------|
| Perry         | commit `e1cbd37` (v0.5.22, LLVM backend)     | `perry compile file.ts -o bin`    |
| Rust          | rustc 1.92.0 (stable)                        | `rustc -O bench.rs`               |
| C++           | Apple clang 21.0 (Xcode)                     | `g++ -O3 -std=c++17`              |
| Go            | go 1.21.3                                    | `go build`                        |
| Swift         | Swift 6.3                                    | `swiftc -O`                       |
| Java          | OpenJDK 21.0.7                               | `javac` + `java` (JIT)            |
| Node.js       | v25.8.0                                      | `node --experimental-strip-types` |
| Bun           | 1.3.5                                        | `bun run file.ts`                 |
| Static Hermes | `shermes` (LLVH 8.0.0svn)                    | `shermes -typed -O` AOT           |
| Python        | CPython 3.14.3                               | `python3 bench.py`                |

**Flag discipline:** every compiled language uses the flag its documentation
suggests for "release mode" — nothing more. No `-ffast-math`, no `-Ounchecked`,
no `#[target_feature]`, no `-march=native`, no profile-guided optimization.
The point is to compare defaults. A "what-if" suite with aggressive flags is
the companion `RESULTS_OPT.md` (see phase 2).

## Methodology

### Measurement

Each benchmark prints a single line of the form `name:elapsed_ms` using the
language's highest-resolution monotonic clock:

| Language | Clock                                    |
|----------|------------------------------------------|
| Perry    | `Date.now()` (maps to `clock_gettime(MONOTONIC)`) |
| Rust     | `std::time::Instant::now()`              |
| C++      | `std::chrono::steady_clock::now()`       |
| Go       | `time.Now()`                             |
| Swift    | `Date()` / `DispatchTime.now()`          |
| Java     | `System.nanoTime()`                      |
| Node/Bun/Hermes | `Date.now()`                       |
| Python   | `time.perf_counter()`                    |

All timings are integer milliseconds after truncation. Sub-millisecond
benchmarks (e.g. object_create on Rust/C++/Go/Swift, which is 0 ms after
dead-code elimination) are reported as `0` — this is a real result, not a
missing value. See the "where Perry loses" discussion in `RESULTS.md`.

### Best-of-N

The runner invokes each binary 5 times and reports the minimum. Best-of-N
tracks the compiler's asymptotic output rather than scheduler noise,
thermal throttling, or interference from other processes. The variance on
these benchmarks is small (<5% across runs on an idle system) — `best-of-5`
vs `best-of-10` produces the same numbers to the millisecond.

### Warmup

None. These are AOT-compiled (or, for Java and Node/Bun, contain enough
iterations that JIT compilation converges well before the hot loop finishes).
The one runtime where this matters is the JVM — Java's numbers include
~50ms of C2 tier-up for the first few iterations. That's visible on
`loop_overhead` (98ms vs Node 53ms) but washes out on longer benchmarks.

### Iteration counts

Chosen so that the slowest compiled language runs each benchmark in
0.5–1 second. Python is treated as out-of-scope for iteration-count tuning;
it runs the same loops and reports the time it takes, which is 100–1000×
everything else.

| Benchmark      | Iterations | Array size  | Notes                              |
|----------------|-----------:|------------:|-----------------------------------|
| fibonacci      | recursion  |           — | `fib(40)` — ~2 billion calls      |
| loop_overhead  |       100M |           — | `sum += 1.0`                      |
| array_write    |        10M |         10M | write `arr[i] = i`                |
| array_read     |        10M |         10M | sum array elements                |
| math_intensive |        50M |           — | `result += 1.0/i`                 |
| object_create  |         1M |           — | allocate `Point(x,y)`, sum fields |
| nested_loops   |   3000×3000|        3000²| flat-array index sum              |
| accumulate     |       100M |           — | `sum += i % 1000` on f64          |

## How the runner works

`run_all.sh` in this directory. Roughly:

```
1. Build Perry from source (`cargo build --release -p perry`)
2. For each .ts file in ../suite, compile via `perry compile`
3. Compile bench.{cpp,rs,swift,go,java,py,zig} with release flags
4. If Hermes is installed, strip TS types from each suite .ts file and AOT-compile
5. For each (benchmark, runtime), run 5 times, take the minimum
6. Print a markdown table
```

The Node/Bun/Hermes runs use the same `.ts` files as Perry (from
`../suite/`). Hermes requires pre-stripping TS types — handled by a
small `sed` script inside `run_all.sh`.

Python is in-scope but not apples-to-apples with the compiled languages.
Its numbers are included in `RESULTS.md` as a floor, not a comparison
target.

## What Perry does differently

Three specific optimization choices account for every benchmark where Perry
beats all native compiled languages. These are the thesis of the companion
article and the reason this suite exists.

### 1. Fast-math reassociation on f64 arithmetic

`crates/perry-codegen/src/block.rs:132-165`. Perry emits
`fadd/fsub/fmul/fdiv/frem/fneg` with the `reassoc contract` LLVM fast-math
flags on every instruction. `reassoc` lets LLVM reorder
`(a + b) + c → a + (b + c)`, which is what the loop vectorizer needs to
break a serial accumulator chain into 4–8 parallel accumulators. `contract`
lets it fuse `x*y + z` into `fma`.

Rust, C++, Go, and Swift all default to IEEE 754 strict. Under IEEE rules,
`(a + b) + c ≠ a + (b + c)` in general — because a single `inf` or `nan` in
the chain makes reordering observably change the result. The compiler
must preserve original associativity, so every `fadd` in
`for (...) sum += 1.0` has a 3-cycle latency dependency on the previous
`fadd`. That's why Rust/C++/Go/Swift cluster at ~95ms on `loop_overhead`:
they're hitting the `fadd` latency wall, all running the same IEEE-strict
serialized loop.

Perry at 12ms means LLVM broke the chain, ran 4–8 parallel `fadd`s per
NEON FPU, and probably unrolled 8×. The same C++ with `-ffast-math` reaches
the same number — phase 2 of this investigation confirms that. Perry's
advantage here is **default flags**, not compiler capability.

The full rationale is in `block.rs:101-131` — Perry deliberately does not
emit the full `fast` FMF bundle (which would include `nnan ninf nsz`)
because JavaScript programs can observe `NaN` and `-0.0` distinctions.
`reassoc contract` is the minimum set needed for the loop-vectorizer
unlock without breaking `Math.max(-0, 0)` semantics.

### 2. Integer-modulo fast path

`crates/perry-codegen/src/type_analysis.rs:488` (`is_integer_valued_expr`)
and `crates/perry-codegen/src/collectors.rs:1006` (`collect_integer_locals`).
The `BinaryOp::Mod` lowering in `expr.rs:823` checks whether both operands
are provably integer-valued. If so, it emits
`fptosi → srem → sitofp` instead of `frem double`.

On ARM, `frem` lowers to a **libm function call** (`fmod`) — there is no
hardware remainder instruction for f64. That's ~30 ns per call, plus the
overhead of a real function call in a tight loop. `srem` is a single ARM
instruction at ~1–2 cycles. The ratio is why `accumulate` shows Perry at
25 ms vs every other language at ~96 ms — the gap is entirely `srem` vs
`fmod` dispatch cost.

This is a **type-driven** optimization, not a language-capability
optimization. Every language in the suite would hit the same 25 ms if its
benchmark used `int64`/`i64`/`long` instead of `double`. The optimized
variants (phase 2, see `RESULTS_OPT.md`) confirm this. Perry's win on
`accumulate` is: it can infer, from the TS source code and the absence of
non-integer operations on the accumulator, that the `double` here is always
holding an integer value, and swap the lowering to use the integer
instruction set — while the human-written TS source still looks like
`sum += i % 1000`.

### 3. i32 loop counter + bounds elimination

`crates/perry-codegen/src/stmt.rs:651-782`. When Perry lowers a `for` loop
whose condition is `i < arr.length` and whose body indexes `arr[i]`:

1. It allocates a parallel **i32 counter slot** alongside the f64 counter
   (`i32_counter_slots`).
2. It caches `arr.length` once at loop entry (`cached_lengths`).
3. It records the `(counter, array)` pair as statically in-bounds
   (`bounded_index_pairs`) — subsequent `arr[i]` reads skip the runtime
   length load and bounds check entirely.

The array-access codegen sites consult these maps and emit a raw
`getelementptr + load` when available. On `array_write` and `array_read`,
this produces code that LLVM can autovectorize into NEON 2-wide f64 SIMD,
matching `-O3 -ffast-math` C++ output.

**Important**: this is *not* "Perry removes safety." It's static proof that
the bounds check is dead. The JS semantics are preserved: you can still
read past the end of an array, you still get `undefined`. The compiler has
just observed, for this specific `for` loop shape, that the index is bounded
by the length. Rust's iterator path (`.iter().sum()`) does the same analysis
at the IR level — and matches Perry to the millisecond on `array_read`
when used. Phase 2 confirms this.

Go cannot express this in the standard toolchain; Go always bounds-checks
indexed array access, and the Go compiler's bounds-check elision is
conservative on patterns this simple. Go's `array_read` stays at ~10 ms
regardless of iteration form.

## Where Perry loses — and why

### `object_create` (Perry: ~2–8 ms, Rust/C++/Go/Swift: 0 ms)

The 0 ms results from Rust/C++/Go/Swift are real. Those languages:
1. Stack-allocate the struct (or elide the allocation entirely).
2. Inline the constructor.
3. Observe the struct never escapes the loop.
4. Compute the sum in closed form at compile time.

The entire loop body is dead code. The benchmark measures nothing.

Perry cannot match this without abandoning its dynamic value model.
JavaScript objects are heap-allocated by spec (with limited escape
analysis available via the v0.5.17 scalar-replacement pass, which
currently kicks in only when the object is *only ever accessed* via
field get/set — any method call defeats it). This is an inherent
cost of compiling a dynamic language: the optimizer has less static
information to work with.

This benchmark is included honestly — it's the shape of workload where
Perry's approach pays a real tax relative to ahead-of-time compiled
languages with static types.

### `fibonacci` (Perry ties C++, beats Rust — but only because of type inference)

Perry's fib is at ~309 ms, C++ 309 ms, Rust ~316 ms — Perry "beats"
Rust here. The honest framing: Perry's benchmark is written as
`fib(n: number)`, which Perry's type inference refines to `i64` because
the function only ever performs integer operations. The generated LLVM
IR uses `sub/add/icmp`. Rust's benchmark uses `f64` to match
TypeScript's `number` type — so Rust generates `fsub/fadd/fcmp`.

Both compile through LLVM. Same optimizer, different input types. If
the Rust benchmark used `fn fib(n: i64) -> i64`, it would run at
~308 ms and the "Perry wins" framing disappears. The phase 2
`bench_opt.rs` does exactly this.

Java wins this benchmark (~279 ms). The JVM's C2 JIT inlines the
recursive call more aggressively than any of the AOT compilers here
manage to do at module scope. This is a JIT-vs-AOT story, not a
Perry story.

## Changelog

This methodology will drift as the Perry codegen changes. Key moments:

- **2026-04-15 (v0.5.22 / e1cbd37):** Initial document. Bun and
  Static Hermes added to the comparison.
- **v0.5.17 (llvm-backend, earlier 2026):** Scalar-replacement pass for
  non-escaping objects dropped `object_create` from 10 ms → 2 ms and
  `binary_trees` from 9 ms → 3 ms. Relevant to the `object_create`
  discussion above; this was what made Perry competitive on that
  benchmark at all.
- **v0.5.2 (llvm-backend, earlier 2026):** The three optimizations
  described above landed. Before this, Perry was ~95 ms on
  `loop_overhead` (IEEE-strict `fadd` chain, same as the other
  languages). These benchmarks only started showing Perry ahead of
  native compiled languages after `reassoc contract` FMF and the
  integer-mod fast path landed.

## Reproducing

```bash
cd benchmarks/polyglot
bash run_all.sh 5      # best of 5 per benchmark
```

Requires: Perry built from this repo (`cargo build --release`), plus
any subset of Node, Bun, Static Hermes (`shermes`), Rust, C++, Go,
Swift, Java, Python. Missing runtimes produce `-` cells; the script
does not fail.

Runtime is ~10 minutes on an M1 Max at best-of-5, dominated by Python
(~30 s per full bench.py invocation).
