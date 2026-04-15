// Optimized Go variant — type choices aligned with Perry where possible.
//
// Changes vs bench.go:
//  - fib:        no change. Go's `int` on arm64 is already int64.
//  - accumulate: float64 sum, `float64(i % 1000)` → int64 sum, `i % 1000`.
//                Perry's integer-mod fast path emits srem; the default
//                variant in bench.go calls runtime.fmod once per iter.
//
// Things the standard Go toolchain cannot express:
//
//  - loop_overhead / math_intensive: Go's compiler does not expose
//    fast-math / reassoc flags. There is no `-ffast-math` equivalent in
//    `go build`. The `gc` compiler preserves strict IEEE 754 semantics
//    and does not ship a floating-point reassociation pass. Manual
//    unrolling (as in bench_opt.rs) would help superficially but Go's
//    register allocator still serializes the fadd chain because the
//    compiler doesn't know those fadds commute. Left as the default
//    loop — this is the honest baseline for Go on this class of code.
//
//  - array_read / array_write: Go always bounds-checks indexed slice
//    access, and the compiler's bounds-check elision is conservative
//    for `for i := 0; i < len(arr); i++ { arr[i] = ... }`. The `range`
//    form sometimes lets the compiler elide checks; we use it below
//    for array_read to give Go its best shot. array_write still uses
//    indexed form because `range` only iterates values, not slots.

package main

import (
	"fmt"
	"time"
)

func benchFibonacci() {
	var fib func(n int64) int64
	fib = func(n int64) int64 {
		if n < 2 {
			return n
		}
		return fib(n-1) + fib(n-2)
	}

	start := time.Now()
	result := fib(40)
	elapsed := time.Since(start).Milliseconds()
	fmt.Printf("fibonacci:%d\n", elapsed)
	fmt.Printf("  checksum: %d\n", result)
}

func benchLoopOverhead() {
	start := time.Now()
	sum := 0.0
	for i := 0; i < 100_000_000; i++ {
		sum += 1.0
	}
	elapsed := time.Since(start).Milliseconds()
	fmt.Printf("loop_overhead:%d\n", elapsed)
	fmt.Printf("  checksum: %.0f\n", sum)
}

func benchArrayWrite() {
	arr := make([]float64, 10_000_000)
	start := time.Now()
	for i := 0; i < 10_000_000; i++ {
		arr[i] = float64(i)
	}
	elapsed := time.Since(start).Milliseconds()
	fmt.Printf("array_write:%d\n", elapsed)
	fmt.Printf("  checksum: %.0f\n", arr[9_999_999])
}

func benchArrayRead() {
	arr := make([]float64, 10_000_000)
	for i := 0; i < 10_000_000; i++ {
		arr[i] = float64(i)
	}
	start := time.Now()
	sum := 0.0
	for _, v := range arr {
		sum += v
	}
	elapsed := time.Since(start).Milliseconds()
	fmt.Printf("array_read:%d\n", elapsed)
	fmt.Printf("  checksum: %.0f\n", sum)
}

func benchMathIntensive() {
	start := time.Now()
	result := 0.0
	for i := 1; i <= 50_000_000; i++ {
		result += 1.0 / float64(i)
	}
	elapsed := time.Since(start).Milliseconds()
	fmt.Printf("math_intensive:%d\n", elapsed)
	fmt.Printf("  checksum: %.6f\n", result)
}

type Point struct {
	x float64
	y float64
}

func benchObjectCreate() {
	start := time.Now()
	sum := 0.0
	for i := 0; i < 1_000_000; i++ {
		p := Point{x: float64(i), y: float64(i) * 2.0}
		sum += p.x + p.y
	}
	elapsed := time.Since(start).Milliseconds()
	fmt.Printf("object_create:%d\n", elapsed)
	fmt.Printf("  checksum: %.0f\n", sum)
}

func benchNestedLoops() {
	n := 3000
	arr := make([]float64, n*n)
	for i := 0; i < n*n; i++ {
		arr[i] = float64(i)
	}
	start := time.Now()
	sum := 0.0
	for _, v := range arr {
		sum += v
	}
	elapsed := time.Since(start).Milliseconds()
	fmt.Printf("nested_loops:%d\n", elapsed)
	fmt.Printf("  checksum: %.0f\n", sum)
}

func benchAccumulate() {
	start := time.Now()
	var sum int64 = 0
	for i := int64(0); i < 100_000_000; i++ {
		sum += i % 1000
	}
	elapsed := time.Since(start).Milliseconds()
	fmt.Printf("accumulate:%d\n", elapsed)
	fmt.Printf("  checksum: %d\n", sum)
}

func main() {
	benchFibonacci()
	benchLoopOverhead()
	benchArrayWrite()
	benchArrayRead()
	benchMathIntensive()
	benchObjectCreate()
	benchNestedLoops()
	benchAccumulate()
}
