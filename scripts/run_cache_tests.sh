#!/usr/bin/env bash
# Integration smoke test for the V2.2 object cache
# (see `crates/perry/src/commands/compile.rs :: ObjectCache`).
#
# Exercises:
#   1. `--no-cache` baseline: record expected runtime output.
#   2. Cold cache build (no .perry-cache/): every module is a miss + store,
#      runtime output matches baseline.
#   3. Warm cache build: every module is a hit, no compile_module invocations,
#      runtime output still matches baseline.
#   4. Source-change partial invalidation: touch one module's source, confirm
#      N-1 hits / 1 miss and that runtime output reflects the edit.
#   5. Topological order regression (v0.5.127-128 class of bug): same
#      `registry.ts` / `register-defaults.ts` / `oids.ts` project used as
#      a smoke gate — if the cache key ever drops `non_entry_module_prefixes`
#      ordering, a reordered init chain would hit a stale entry module
#      and `count=N` would silently drift.
#
# The cache key itself is unit-tested in `object_cache_tests::...`.
# This script is the end-to-end gate: the whole pipeline (collect_modules →
# rayon codegen → cache lookup/store → linker) stays byte-accurate across
# cache states.

set -euo pipefail

PERRY="${PERRY:-$(pwd)/target/release/perry}"
if [ ! -x "$PERRY" ]; then
    echo "error: $PERRY not found or not executable; run 'cargo build --release -p perry' first" >&2
    exit 1
fi

TEST_DIR="$(pwd)/test-files/module-init-order"
if [ ! -d "$TEST_DIR" ]; then
    echo "error: $TEST_DIR not found" >&2
    exit 1
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

cp -R "$TEST_DIR"/* "$WORK/"
cd "$WORK"

MAIN_ENTRY="main.ts"
BIN="./prog"

run_and_capture() {
    local label="$1"
    shift
    local logfile="$WORK/${label}.log"
    echo "=== $label ===" >&2
    PERRY_DEV_VERBOSE=1 "$PERRY" compile "$MAIN_ENTRY" -o "$BIN" "$@" >"$logfile" 2>&1 \
        || { echo "compile failed ($label):"; cat "$logfile"; exit 1; }
    "$BIN" > "$WORK/${label}.out"
    cat "$logfile" | grep -E "• codegen cache" || echo "  (no cache line)"
}

# 1. Baseline with --no-cache.
rm -rf .perry-cache
run_and_capture baseline --no-cache
BASELINE_OUT="$(cat "$WORK/baseline.out")"
echo "  baseline output: $BASELINE_OUT"

# 2. Cold cache: every module should be a miss+store.
rm -rf .perry-cache
run_and_capture cold
COLD_OUT="$(cat "$WORK/cold.out")"
[ "$COLD_OUT" = "$BASELINE_OUT" ] || { echo "FAIL: cold output differs from baseline" >&2; exit 1; }
grep -E "• codegen cache: 0/[0-9]+ hit" "$WORK/cold.log" >/dev/null \
    || { echo "FAIL: cold build should have 0 hits" >&2; cat "$WORK/cold.log" | grep cache >&2; exit 1; }

# 3. Warm cache: every module should be a hit.
run_and_capture warm
WARM_OUT="$(cat "$WORK/warm.out")"
[ "$WARM_OUT" = "$BASELINE_OUT" ] || { echo "FAIL: warm output differs from baseline" >&2; exit 1; }
# Expect zero misses: "N/N hit (0 miss)". "All hits" == miss count is 0 —
# use plain `grep -E` without a backreference so the test stays portable to
# BSD grep on macOS (GNU grep supports backrefs in -E as an extension, BSD
# grep does not).
if ! grep -E "• codegen cache: [0-9]+/[0-9]+ hit \(0 miss\)" "$WORK/warm.log" >/dev/null; then
    echo "FAIL: warm build should be all hits" >&2
    cat "$WORK/warm.log" | grep cache >&2
    exit 1
fi

# 4. Edit one module; rebuild; that module should be a miss, the others hits.
#    Using `cp` (not shell var capture) to preserve the original exactly —
#    command substitution strips trailing newlines, which would flip the
#    source hash on restore.
cp registry.ts registry.ts.orig
sed -i.bak 's/MISSING/NOTFOUND/' registry.ts
rm -f registry.ts.bak
run_and_capture partial
if ! grep -E "• codegen cache: [0-9]+/[0-9]+ hit \(1 miss\)" "$WORK/partial.log" >/dev/null; then
    echo "FAIL: partial rebuild should be 1 miss" >&2
    cat "$WORK/partial.log" | grep cache >&2
    exit 1
fi
# Output must reflect the edit — this is the key anti-staleness check:
# a cache bug that returned the OLD .o bytes would still print "MISSING".
grep -q "999=NOTFOUND" "$WORK/partial.out" \
    || { echo "FAIL: partial output did not reflect source edit" >&2; cat "$WORK/partial.out" >&2; exit 1; }

# 5. Restore source and confirm the cache correctly roundtrips back to a
#    full-hit state for the original sources (no lingering stale state).
cp registry.ts.orig registry.ts
rm -f registry.ts.orig
run_and_capture rewarm
REWARM_OUT="$(cat "$WORK/rewarm.out")"
[ "$REWARM_OUT" = "$BASELINE_OUT" ] || { echo "FAIL: post-restore output differs from baseline" >&2; exit 1; }
if ! grep -E "• codegen cache: [0-9]+/[0-9]+ hit \(0 miss\)" "$WORK/rewarm.log" >/dev/null; then
    echo "FAIL: after restoring source, rebuild should be all hits" >&2
    cat "$WORK/rewarm.log" | grep cache >&2
    exit 1
fi

# 6. `perry cache info` and `perry cache clean` smoke-test.
"$PERRY" cache info >"$WORK/info.log" 2>&1
grep -q ".perry-cache" "$WORK/info.log" || { echo "FAIL: cache info should mention .perry-cache" >&2; exit 1; }
"$PERRY" cache clean >"$WORK/clean.log" 2>&1
grep -qE "Removed.*\\.perry-cache" "$WORK/clean.log" || { echo "FAIL: cache clean should report removal" >&2; exit 1; }
[ ! -d ".perry-cache" ] || { echo "FAIL: .perry-cache still present after clean" >&2; exit 1; }

echo "PASS: V2.2 object cache end-to-end smoke test"
