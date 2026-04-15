#!/usr/bin/env bash
# Runs the _opt.{cpp,rs,go,swift} variants and pairs the numbers with the
# default-variant numbers from the last run_all.sh sweep.
set -e
cd "$(dirname "$0")"
RUNS=${1:-5}
FIB_RUNS=${2:-20}
TMPDIR=/tmp/perry_polyglot_bench
mkdir -p "$TMPDIR"

echo "=== Building opt variants ==="
g++ -O3 -ffast-math -std=c++17 bench_opt.cpp -o "$TMPDIR/bench_opt_cpp" && echo "  C++ opt: done (-O3 -ffast-math)"
RUSTFLAGS="-C llvm-args=-fp-contract=fast" rustc -O bench_opt.rs -o "$TMPDIR/bench_opt_rs" 2>/dev/null && echo "  Rust opt: done (-O, fp-contract=fast)"
go build -o "$TMPDIR/bench_opt_go" bench_opt.go && echo "  Go opt: done (no opt flags available)"
swiftc -Ounchecked bench_opt.swift -o "$TMPDIR/bench_opt_swift" && echo "  Swift opt: done (-Ounchecked)"

echo ""
echo "=== Running (best of $RUNS, fibonacci: best of $FIB_RUNS) ==="

bestof() {
  local cmd="$1" key="$2" n="$3" best=""
  for i in $(seq 1 "$n"); do
    local out t
    out=$(eval "$cmd" 2>/dev/null) || true
    t=$(echo "$out" | grep -oE "${key}:[0-9]+" | head -1 | grep -oE '[0-9]+$')
    if [ -n "$t" ]; then
      if [ -z "$best" ] || [ "$t" -lt "$best" ]; then best=$t; fi
    fi
  done
  echo "${best:--}"
}

for lang in cpp rs go swift; do
  out="$TMPDIR/results_opt_${lang}.txt"
  > "$out"
  for key in loop_overhead math_intensive array_write array_read object_create nested_loops accumulate; do
    echo "${key}=$(bestof "$TMPDIR/bench_opt_${lang}" "$key" "$RUNS")" >> "$out"
  done
  echo "fibonacci=$(bestof "$TMPDIR/bench_opt_${lang}" "fibonacci" "$FIB_RUNS")" >> "$out"
  echo "  ${lang}: done"
done

# Read helpers
rdef() { grep "^${2}=" "$TMPDIR/results_${1}.txt" 2>/dev/null | cut -d= -f2; }
ropt() { grep "^${2}=" "$TMPDIR/results_opt_${1}.txt" 2>/dev/null | cut -d= -f2; }

delta() {
  local d="$1" o="$2"
  if [ -z "$d" ] || [ -z "$o" ] || [ "$d" = "-" ] || [ "$o" = "-" ] || [ "$d" = "0" ]; then
    echo "--"
    return
  fi
  awk -v d="$d" -v o="$o" 'BEGIN { printf "%.0f%%", (d - o) / d * 100 }'
}

echo ""
echo "# Default vs Optimized"
echo ""
printf "| %-14s | %5s | %5s | %5s | %5s | %5s | %5s | %5s | %5s | %5s | %5s | %6s | %6s | %7s |\n" \
  "Benchmark" "Perry" "Cdef" "Copt" "ΔCpp" "Rdef" "Ropt" "ΔRs" "Gdef" "Gopt" "ΔGo" "Sdef" "Sopt" "ΔSw"
echo "|----------------|-------|-------|-------|-------|-------|-------|-------|-------|-------|-------|--------|--------|---------|"

for bench in loop_overhead math_intensive accumulate array_write array_read nested_loops fibonacci object_create; do
  p=$(rdef perry $bench)
  cdef=$(rdef cpp $bench);   copt=$(ropt cpp $bench)
  rdef=$(rdef rust $bench);  ropt=$(ropt rs $bench)
  gdef=$(rdef go $bench);    gopt=$(ropt go $bench)
  sdef=$(rdef swift $bench); sopt=$(ropt swift $bench)
  printf "| %-14s | %5s | %5s | %5s | %5s | %5s | %5s | %5s | %5s | %5s | %5s | %6s | %6s | %7s |\n" \
    "$bench" "$p" "$cdef" "$copt" "$(delta $cdef $copt)" "$rdef" "$ropt" "$(delta $rdef $ropt)" \
    "$gdef" "$gopt" "$(delta $gdef $gopt)" "$sdef" "$sopt" "$(delta $sdef $sopt)"
done
