#!/usr/bin/env bash
# Polyglot benchmark runner
set -e
cd "$(dirname "$0")"
PERRY_ROOT="../.."
SUITE="../suite"
RUNS=${1:-3}
TMPDIR=/tmp/perry_polyglot_bench

mkdir -p "$TMPDIR"

# --- Runtime detection ---
HAS_BUN=0
HAS_SHERMES=0
command -v bun >/dev/null 2>&1 && HAS_BUN=1
command -v shermes >/dev/null 2>&1 && HAS_SHERMES=1

# Strip TypeScript annotations so Hermes (JS-only) can parse.
# Matches the helper in benchmarks/suite/run_benchmarks.sh.
strip_types() {
  sed -E \
    -e 's/: (number|string|boolean|any|void)(\[\])?//g' \
    -e 's/\): (number|string|boolean|any|void)(\[\])? \{/) {/g' \
    "$1"
}

echo "=== Building ==="
cargo build --release --manifest-path="$PERRY_ROOT/Cargo.toml" -p perry -q 2>/dev/null
PERRY="$PERRY_ROOT/target/release/perry"
for f in "$SUITE"/*.ts; do
  name=$(basename "$f" .ts)
  $PERRY compile "$f" -o "$TMPDIR/perry_${name}" -q 2>/dev/null || true
done
echo "  Perry: done"
g++ -O3 -std=c++17 bench.cpp -o "$TMPDIR/bench_cpp" 2>/dev/null && echo "  C++: done"
rustc -O bench.rs -o "$TMPDIR/bench_rs" 2>/dev/null && echo "  Rust: done"
swiftc -O bench.swift -o "$TMPDIR/bench_swift" 2>/dev/null && echo "  Swift: done"
go build -o "$TMPDIR/bench_go" bench.go 2>/dev/null && echo "  Go: done"
javac -d "$TMPDIR" bench.java 2>/dev/null && echo "  Java: done"
echo "  Python: (interpreted)"

# Compile Hermes binaries (one per benchmark) from stripped-types .js
if [ $HAS_SHERMES -eq 1 ]; then
  for bk in "05_fibonacci" "02_loop_overhead" "03_array_write" "04_array_read" "06_math_intensive" "07_object_create" "10_nested_loops" "13_factorial"; do
    js_file="$TMPDIR/shermes_${bk}.js"
    strip_types "$SUITE/${bk}.ts" > "$js_file"
    shermes -typed -O -o "$TMPDIR/shermes_${bk}" "$js_file" 2>/dev/null || \
      shermes -O -o "$TMPDIR/shermes_${bk}" "$js_file" 2>/dev/null || true
  done
  echo "  Hermes: done"
fi

echo ""
echo "=== Running (best of $RUNS) ==="

get_time() { echo "$1" | grep -oE "${2}:[0-9]+" | head -1 | grep -oE '[0-9]+$'; }

best_of() {
  local cmd="$1" key="$2" best=""
  for i in $(seq 1 $RUNS); do
    local out t
    out=$(eval "$cmd" 2>/dev/null) || true
    t=$(get_time "$out" "$key")
    if [ -n "$t" ]; then
      if [ -z "$best" ] || [ "$t" -lt "$best" ]; then best=$t; fi
    fi
  done
  echo "${best:--}"
}

# Run each language and collect into temp files
run_lang() {
  local lang="$1" cmd="$2"
  local results="$TMPDIR/results_${lang}.txt"
  > "$results"
  for bk in "fibonacci:fibonacci" "loop_overhead:loop_overhead" "array_write:array_write" "array_read:array_read" "math_intensive:math_intensive" "object_create:object_create" "nested_loops:nested_loops" "accumulate:accumulate"; do
    IFS=: read -r bench key <<< "$bk"
    local t=$(best_of "$cmd" "$key")
    echo "${bench}=${t}" >> "$results"
  done
  echo "  $lang: done"
}

# Perry (separate binaries per benchmark)
> "$TMPDIR/results_perry.txt"
for bk in "fibonacci:05_fibonacci:fibonacci" "loop_overhead:02_loop_overhead:loop_overhead" "array_write:03_array_write:array_write" "array_read:04_array_read:array_read" "math_intensive:06_math_intensive:math_intensive" "object_create:07_object_create:object_create" "nested_loops:10_nested_loops:nested_loops" "accumulate:13_factorial:accumulate"; do
  IFS=: read -r bench ts key <<< "$bk"
  t=$(best_of "$TMPDIR/perry_${ts}" "$key")
  echo "${bench}=${t}" >> "$TMPDIR/results_perry.txt"
done
echo "  Perry: done"

# Node (separate .ts files)
> "$TMPDIR/results_node.txt"
for bk in "fibonacci:05_fibonacci:fibonacci" "loop_overhead:02_loop_overhead:loop_overhead" "array_write:03_array_write:array_write" "array_read:04_array_read:array_read" "math_intensive:06_math_intensive:math_intensive" "object_create:07_object_create:object_create" "nested_loops:10_nested_loops:nested_loops" "accumulate:13_factorial:accumulate"; do
  IFS=: read -r bench ts key <<< "$bk"
  t=$(best_of "node --experimental-strip-types $SUITE/${ts}.ts" "$key")
  echo "${bench}=${t}" >> "$TMPDIR/results_node.txt"
done
echo "  Node: done"

# Bun (separate .ts files — Bun parses TS natively)
> "$TMPDIR/results_bun.txt"
if [ $HAS_BUN -eq 1 ]; then
  for bk in "fibonacci:05_fibonacci:fibonacci" "loop_overhead:02_loop_overhead:loop_overhead" "array_write:03_array_write:array_write" "array_read:04_array_read:array_read" "math_intensive:06_math_intensive:math_intensive" "object_create:07_object_create:object_create" "nested_loops:10_nested_loops:nested_loops" "accumulate:13_factorial:accumulate"; do
    IFS=: read -r bench ts key <<< "$bk"
    t=$(best_of "bun run $SUITE/${ts}.ts" "$key")
    echo "${bench}=${t}" >> "$TMPDIR/results_bun.txt"
  done
  echo "  Bun: done"
else
  for bench in fibonacci loop_overhead array_write array_read math_intensive object_create nested_loops accumulate; do
    echo "${bench}=-" >> "$TMPDIR/results_bun.txt"
  done
  echo "  Bun: skipped (not installed)"
fi

# Static Hermes (compiled binaries)
> "$TMPDIR/results_hermes.txt"
if [ $HAS_SHERMES -eq 1 ]; then
  for bk in "fibonacci:05_fibonacci:fibonacci" "loop_overhead:02_loop_overhead:loop_overhead" "array_write:03_array_write:array_write" "array_read:04_array_read:array_read" "math_intensive:06_math_intensive:math_intensive" "object_create:07_object_create:object_create" "nested_loops:10_nested_loops:nested_loops" "accumulate:13_factorial:accumulate"; do
    IFS=: read -r bench ts key <<< "$bk"
    if [ -x "$TMPDIR/shermes_${ts}" ]; then
      t=$(best_of "$TMPDIR/shermes_${ts}" "$key")
    else
      t="-"
    fi
    echo "${bench}=${t}" >> "$TMPDIR/results_hermes.txt"
  done
  echo "  Hermes: done"
else
  for bench in fibonacci loop_overhead array_write array_read math_intensive object_create nested_loops accumulate; do
    echo "${bench}=-" >> "$TMPDIR/results_hermes.txt"
  done
  echo "  Hermes: skipped (not installed)"
fi

# Polyglot languages (all benchmarks in one binary)
run_lang "rust" "$TMPDIR/bench_rs"
run_lang "cpp" "$TMPDIR/bench_cpp"
run_lang "swift" "$TMPDIR/bench_swift"
run_lang "go" "$TMPDIR/bench_go"
run_lang "java" "java -cp $TMPDIR bench"
run_lang "python" "python3 bench.py"

# Read result
r() {
  local lang="$1" bench="$2"
  grep "^${bench}=" "$TMPDIR/results_${lang}.txt" 2>/dev/null | cut -d= -f2
}

echo ""
echo "# Polyglot Benchmark Results"
echo ""
echo "Best of $RUNS runs, macOS ARM64 (Apple Silicon). All times in milliseconds."
echo "Lower is better."
echo ""
printf "| %-14s | %5s | %5s | %5s | %5s | %5s | %5s | %5s | %5s | %6s | %7s |\n" \
  "Benchmark" "Perry" "Rust" "C++" "Go" "Swift" "Java" "Node" "Bun" "Hermes" "Python"
echo "|----------------|-------|-------|-------|-------|-------|-------|-------|-------|--------|---------|"

for bench in fibonacci loop_overhead array_write array_read math_intensive object_create nested_loops accumulate; do
  printf "| %-14s | %5s | %5s | %5s | %5s | %5s | %5s | %5s | %5s | %6s | %7s |\n" \
    "$bench" \
    "$(r perry $bench)" \
    "$(r rust $bench)" \
    "$(r cpp $bench)" \
    "$(r go $bench)" \
    "$(r swift $bench)" \
    "$(r java $bench)" \
    "$(r node $bench)" \
    "$(r bun $bench)" \
    "$(r hermes $bench)" \
    "$(r python $bench)"
done
