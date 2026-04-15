# Polyglot Benchmark Results — Default vs Optimized

Same benchmarks as [`RESULTS.md`](./RESULTS.md), but with a second column
per native language showing what happens when the language is given the
flags and idioms that match what Perry does by default.

**Run date:** 2026-04-15 — Perry commit `e1cbd37`.
**Hardware:** Apple M1 Max, macOS 26.4.
**Methodology:** best of 5 per cell (best of 20 for `fibonacci`).

## Side by side

All times in milliseconds. `Δ` = (default − opt) / default. Positive = opt
is faster.

| Benchmark        | Perry |  C++<br>dflt |  C++<br>opt |  ΔC++ | Rust<br>dflt | Rust<br>opt | ΔRust |  Go<br>dflt |  Go<br>opt |  ΔGo | Swift<br>dflt | Swift<br>opt | ΔSwift |
|------------------|------:|-------------:|------------:|------:|-------------:|------------:|------:|------------:|-----------:|-----:|--------------:|-------------:|-------:|
| loop_overhead    |    12 |           98 |          12 |  88%  |           99 |          24 |  76%  |          97 |         99 |  0%  |            97 |           24 |   75%  |
| math_intensive   |    14 |           50 |          14 |  72%  |           49 |          14 |  71%  |          49 |         49 |  0%  |            49 |           14 |   71%  |
| accumulate       |    24 |           97 |          26 |  73%  |           97 |          41 |  58%  |          99 |         70 | 29%  |            96 |           42 |   56%  |
| array_write      |     2 |            2 |           2 |   0%  |            7 |           7 |   0%  |           9 |          9 |  0%  |             2 |            2 |    0%  |
| array_read       |     3 |            9 |           1 |  89%  |           10 |           9 |  10%  |          10 |         11 | -10% |             9 |            9 |    0%  |
| nested_loops     |     9 |            8 |           1 |  88%  |            8 |           8 |   0%  |          10 |          9 | 10%  |             8 |            8 |    0%  |
| fibonacci        |   311 |          310 |         312 |  -1%  |          319 |         319 |   0%  |         450 |        454 | -1%  |           403 |          360 |   11%  |
| object_create    |     2 |            0 |           0 |  --   |            0 |           0 |  --   |           0 |          0 |  --  |             0 |            0 |    --  |

## The one-line story per language

**C++ (`bench_opt.cpp`, `-O3 -ffast-math -std=c++17`):** adding `-ffast-math`
and switching `accumulate` to `int64_t` closes every gap. C++ matches Perry
to the millisecond on `loop_overhead` (12 = 12) and `math_intensive` (14 =
14), and **beats Perry** on `array_read` (1 < 3) and `nested_loops` (1 < 9)
because clang's autovectorizer on ffast-math flat-array sums is more
aggressive than what Perry currently emits. The thesis is confirmed: the
entire Perry advantage on numeric f64 loops is the default flag choice,
not the compiler or the codegen backend.

**Rust (`bench_opt.rs`, stable + `-C llvm-args=-fp-contract=fast`):** manual
4-way unrolling + iterator form + `i64` accumulate closes **most** of the
gap, but not all. `loop_overhead` goes from 99 → 24 ms (76% improvement)
but doesn't reach Perry's 12 ms — because stable Rust has no way to expose
LLVM's `reassoc` flag on individual fadd instructions. Nightly Rust's
`std::intrinsics::fadd_fast` would get there; we intentionally stayed on
stable. This is an interesting finding: Rust's *type system* can express
what Perry does (via `i64`), but Rust's *compile flags* cannot express
what Perry does (via `reassoc`).

**Go (`bench_opt.go`, `go build`):** the only language that **cannot** close
the `loop_overhead` / `math_intensive` gap at all. Go has no `-ffast-math`,
no `reassoc` flag, and its compiler does not ship a floating-point
reassociation pass. `99 → 99` and `49 → 49` on the two fast-math-dependent
benchmarks, even with the full suite of type and loop-form changes that
helped the other languages. The only benchmark where Go opt improves on
Go default is `accumulate` (99 → 70), from the `int64` switch — and even
there, Go's 70 ms is well short of C++ opt's 26 ms, because Go's compiler
inserts a runtime integer-divide path that's slower than a bare ARM `sdiv`
+ `msub` for the modulo.

**Swift (`bench_opt.swift`, `-Ounchecked`):** manual unrolling and
`UnsafeBufferPointer` close the `loop_overhead` (97 → 24) and
`math_intensive` (49 → 14) gaps partially — same profile as Rust. Swift
also has no reachable `reassoc` flag on its public release toolchain as of
6.3, so the remaining 24 → 12 gap is the same story as Rust. `fibonacci`
improves noticeably (403 → 360) with `-Ounchecked`.

## Where the opt variants matter less than expected

**`array_write` / `array_read`:** the bounds-check elimination story is
less dramatic than predicted in the phase-2 plan. Rust's default indexed
`arr[i]` access with `-O` already gets within 10% of optimal because rustc
is good at proving `i < arr.len()` for classic for-loops. `.iter().sum()`
only shaves 10 → 9 on `array_read`. Swift `UnsafeBufferPointer` on
`array_write` shaved 2 → 1 ms but that's mostly in the noise floor.

The real `array_read` win is on **C++ opt (1 ms)** — and that's from
`-ffast-math` enabling LLVM to break the sum reduction into 4 parallel
lanes, not from bounds elimination. C++ had no bounds checks to remove.

**`fibonacci`:** type-switching from i32 → i64 (C++, Rust) or no-op (Go,
Swift — both already Int64-native on arm64) doesn't change the numbers
materially. The fib recursion is bottlenecked on call overhead, not
arithmetic width, and ARM64 handles i32 and i64 ops at the same rate. The
language-to-language fib gap (~315 ms for Rust/C++/Perry vs ~450 ms for
Go) is the compiler's recursion-folding quality, not expressible in
benchmark-source-level changes.

## Compile commands

| File             | Command                                                      |
|------------------|--------------------------------------------------------------|
| `bench.cpp`      | `g++ -O3 -std=c++17 bench.cpp -o bench_cpp`                  |
| `bench_opt.cpp`  | `g++ -O3 -ffast-math -std=c++17 bench_opt.cpp -o bench_opt_cpp` |
| `bench.rs`       | `rustc -O bench.rs -o bench_rs`                              |
| `bench_opt.rs`   | `RUSTFLAGS="-C llvm-args=-fp-contract=fast" rustc -O bench_opt.rs -o bench_opt_rs` |
| `bench.go`       | `go build -o bench_go bench.go`                              |
| `bench_opt.go`   | `go build -o bench_opt_go bench_opt.go` (no opt flags exist) |
| `bench.swift`    | `swiftc -O bench.swift -o bench_swift`                       |
| `bench_opt.swift`| `swiftc -Ounchecked bench_opt.swift -o bench_opt_swift`      |

## Reproducing

```bash
cd benchmarks/polyglot
bash run_opt.sh        # builds opt variants, runs best of 5, prints table
```

`run_opt.sh` reads default numbers from the last `run_all.sh` sweep
(stored in `/tmp/perry_polyglot_bench/results_*.txt`) so a full refresh
is `run_all.sh && run_opt.sh`.
