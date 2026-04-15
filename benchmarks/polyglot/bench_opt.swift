// Optimized Swift variant — type choices and compile flags aligned with
// Perry's defaults where possible.
//
// Changes vs bench.swift:
//  - fib:        no change. Swift's `Int` on arm64 is already Int64.
//  - accumulate: Double sum → Int64 sum, removed Double() cast on i%1000.
//                Perry's integer-mod fast path emits srem; the default
//                variant calls fmod once per iter.
//  - array_read / array_write / nested_loops: use
//                `arr.withUnsafeMutableBufferPointer` (write) and
//                `arr.withUnsafeBufferPointer` (read) to get raw pointer
//                iteration. This skips Swift's default Array bounds checks
//                and the ARC retain/release that the safe subscript pulls
//                in around Copy-on-Write wrappers.
//  - loop_overhead / math_intensive: compile with `-Ounchecked` (Swift's
//                only non-default knob). Swift has no exposed fast-math
//                flag as of 6.3 on the release toolchain; the LLVM FMFs
//                are not reachable from the Swift CLI. Manual 4-way
//                unrolling is added as a stand-in for what LLVM would do
//                under reassoc, matching what bench_opt.rs does for
//                stable Rust.
//
// Compile:
//   swiftc -Ounchecked bench_opt.swift

import Foundation

func benchFibonacci() {
    func fib(_ n: Int) -> Int {
        if n < 2 { return n }
        return fib(n - 1) + fib(n - 2)
    }

    let start = CFAbsoluteTimeGetCurrent()
    let result = fib(40)
    let elapsed = Int((CFAbsoluteTimeGetCurrent() - start) * 1000)
    print("fibonacci:\(elapsed)")
    print("  checksum: \(result)")
}

func benchLoopOverhead() {
    let start = CFAbsoluteTimeGetCurrent()
    // Manual 4-way unrolling — same reason as bench_opt.rs. Swift's
    // compiler does not expose reassoc on the release toolchain.
    var s0: Double = 0.0
    var s1: Double = 0.0
    var s2: Double = 0.0
    var s3: Double = 0.0
    let iters = 100_000_000 / 4
    for _ in 0..<iters {
        s0 += 1.0
        s1 += 1.0
        s2 += 1.0
        s3 += 1.0
    }
    let sum = s0 + s1 + s2 + s3
    let elapsed = Int((CFAbsoluteTimeGetCurrent() - start) * 1000)
    print("loop_overhead:\(elapsed)")
    print("  checksum: \(Int(sum))")
}

func benchArrayWrite() {
    var arr = [Double](repeating: 0.0, count: 10_000_000)
    let start = CFAbsoluteTimeGetCurrent()
    arr.withUnsafeMutableBufferPointer { buf in
        for i in 0..<buf.count {
            buf[i] = Double(i)
        }
    }
    let elapsed = Int((CFAbsoluteTimeGetCurrent() - start) * 1000)
    print("array_write:\(elapsed)")
    print("  checksum: \(Int(arr[9_999_999]))")
}

func benchArrayRead() {
    var arr = [Double](repeating: 0.0, count: 10_000_000)
    for i in 0..<10_000_000 {
        arr[i] = Double(i)
    }
    let start = CFAbsoluteTimeGetCurrent()
    var sum: Double = 0.0
    arr.withUnsafeBufferPointer { buf in
        for i in 0..<buf.count {
            sum += buf[i]
        }
    }
    let elapsed = Int((CFAbsoluteTimeGetCurrent() - start) * 1000)
    print("array_read:\(elapsed)")
    print("  checksum: \(Int(sum))")
}

func benchMathIntensive() {
    let start = CFAbsoluteTimeGetCurrent()
    var r0: Double = 0.0
    var r1: Double = 0.0
    var r2: Double = 0.0
    var r3: Double = 0.0
    var i = 1
    while i + 3 <= 50_000_000 {
        r0 += 1.0 / Double(i)
        r1 += 1.0 / Double(i + 1)
        r2 += 1.0 / Double(i + 2)
        r3 += 1.0 / Double(i + 3)
        i += 4
    }
    while i <= 50_000_000 {
        r0 += 1.0 / Double(i)
        i += 1
    }
    let result = r0 + r1 + r2 + r3
    let elapsed = Int((CFAbsoluteTimeGetCurrent() - start) * 1000)
    print("math_intensive:\(elapsed)")
    print("  checksum: \(String(format: "%.6f", result))")
}

struct Point {
    var x: Double
    var y: Double
}

func benchObjectCreate() {
    let start = CFAbsoluteTimeGetCurrent()
    var sum: Double = 0.0
    for i in 0..<1_000_000 {
        let p = Point(x: Double(i), y: Double(i) * 2.0)
        sum += p.x + p.y
    }
    let elapsed = Int((CFAbsoluteTimeGetCurrent() - start) * 1000)
    print("object_create:\(elapsed)")
    print("  checksum: \(Int(sum))")
}

func benchNestedLoops() {
    let n = 3000
    var arr = [Double](repeating: 0.0, count: n * n)
    for i in 0..<(n * n) {
        arr[i] = Double(i)
    }
    let start = CFAbsoluteTimeGetCurrent()
    var sum: Double = 0.0
    arr.withUnsafeBufferPointer { buf in
        for i in 0..<buf.count {
            sum += buf[i]
        }
    }
    let elapsed = Int((CFAbsoluteTimeGetCurrent() - start) * 1000)
    print("nested_loops:\(elapsed)")
    print("  checksum: \(Int(sum))")
}

func benchAccumulate() {
    let start = CFAbsoluteTimeGetCurrent()
    var sum: Int64 = 0
    for i in 0..<Int64(100_000_000) {
        sum += i % 1000
    }
    let elapsed = Int((CFAbsoluteTimeGetCurrent() - start) * 1000)
    print("accumulate:\(elapsed)")
    print("  checksum: \(sum)")
}

benchFibonacci()
benchLoopOverhead()
benchArrayWrite()
benchArrayRead()
benchMathIntensive()
benchObjectCreate()
benchNestedLoops()
benchAccumulate()
