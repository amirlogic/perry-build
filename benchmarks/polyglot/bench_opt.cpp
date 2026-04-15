// Optimized C++ variant — same algorithms, type choices and compile flags
// aligned with what Perry does by default.
//
// Changes vs bench.cpp:
//  - fib:        int → int64_t (ARM64 native word size; matches Perry's i64
//                inference from TS `number` on a recursive integer function)
//  - accumulate: double → int64_t for sum and i (Perry's integer-mod fast
//                path emits srem on int64; the double variant in bench.cpp
//                calls libm fmod once per iter)
//  - loop_overhead, math_intensive: no source change; compiled with
//                `-O3 -ffast-math` so LLVM can emit `reassoc contract` on
//                fadd/fdiv. bench.cpp is `-O3` only.
//  - array_read/array_write/nested_loops: no change needed — std::vector::
//                operator[] doesn't bounds-check by default, and `-O3
//                -ffast-math` on the read loop is already enough for LLVM
//                to vectorize.
//  - object_create: no change — already fully eliminated by DCE.

#include <chrono>
#include <cstdint>
#include <cstdio>
#include <vector>

using Clock = std::chrono::steady_clock;

inline long long elapsed_ms(Clock::time_point start) {
    return std::chrono::duration_cast<std::chrono::milliseconds>(
        Clock::now() - start).count();
}

int64_t fib(int64_t n) {
    if (n < 2) return n;
    return fib(n - 1) + fib(n - 2);
}

void bench_fibonacci() {
    auto start = Clock::now();
    int64_t result = fib(40);
    printf("fibonacci:%lld\n", elapsed_ms(start));
    printf("  checksum: %lld\n", result);
}

void bench_loop_overhead() {
    auto start = Clock::now();
    double sum = 0.0;
    for (int i = 0; i < 100000000; i++) {
        sum += 1.0;
    }
    printf("loop_overhead:%lld\n", elapsed_ms(start));
    printf("  checksum: %.0f\n", sum);
}

void bench_array_write() {
    std::vector<double> arr(10000000, 0.0);
    auto start = Clock::now();
    for (int i = 0; i < 10000000; i++) {
        arr[i] = static_cast<double>(i);
    }
    printf("array_write:%lld\n", elapsed_ms(start));
    printf("  checksum: %.0f\n", arr[9999999]);
}

void bench_array_read() {
    std::vector<double> arr(10000000);
    for (int i = 0; i < 10000000; i++) {
        arr[i] = static_cast<double>(i);
    }
    auto start = Clock::now();
    double sum = 0.0;
    for (int i = 0; i < 10000000; i++) {
        sum += arr[i];
    }
    printf("array_read:%lld\n", elapsed_ms(start));
    printf("  checksum: %.0f\n", sum);
}

void bench_math_intensive() {
    auto start = Clock::now();
    double result = 0.0;
    for (int i = 1; i <= 50000000; i++) {
        result += 1.0 / static_cast<double>(i);
    }
    printf("math_intensive:%lld\n", elapsed_ms(start));
    printf("  checksum: %.6f\n", result);
}

struct Point {
    double x;
    double y;
};

void bench_object_create() {
    auto start = Clock::now();
    double sum = 0.0;
    for (int i = 0; i < 1000000; i++) {
        Point p{static_cast<double>(i), static_cast<double>(i) * 2.0};
        sum += p.x + p.y;
    }
    printf("object_create:%lld\n", elapsed_ms(start));
    printf("  checksum: %.0f\n", sum);
}

void bench_nested_loops() {
    const int n = 3000;
    std::vector<double> arr(n * n);
    for (int i = 0; i < n * n; i++) {
        arr[i] = static_cast<double>(i);
    }
    auto start = Clock::now();
    double sum = 0.0;
    for (int i = 0; i < n; i++) {
        for (int j = 0; j < n; j++) {
            sum += arr[i * n + j];
        }
    }
    printf("nested_loops:%lld\n", elapsed_ms(start));
    printf("  checksum: %.0f\n", sum);
}

void bench_accumulate() {
    auto start = Clock::now();
    int64_t sum = 0;
    for (int64_t i = 0; i < 100000000; i++) {
        sum += i % 1000;
    }
    printf("accumulate:%lld\n", elapsed_ms(start));
    printf("  checksum: %lld\n", sum);
}

int main() {
    bench_fibonacci();
    bench_loop_overhead();
    bench_array_write();
    bench_array_read();
    bench_math_intensive();
    bench_object_create();
    bench_nested_loops();
    bench_accumulate();
    return 0;
}
