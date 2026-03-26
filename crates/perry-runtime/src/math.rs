//! Math operations runtime support

use rand::Rng;

/// Math.pow(base, exponent) -> number
#[no_mangle]
pub extern "C" fn js_math_pow(base: f64, exp: f64) -> f64 {
    base.powf(exp)
}

/// Floating-point modulo using the C library's fmod
/// This is often faster than the inline computation a - trunc(a/b) * b
#[no_mangle]
pub extern "C" fn js_math_fmod(a: f64, b: f64) -> f64 {
    a % b  // Rust's % operator maps to libm fmod
}

/// Math.log(x) -> number (natural logarithm)
#[no_mangle]
pub extern "C" fn js_math_log(x: f64) -> f64 {
    x.ln()
}

/// Math.log2(x) -> number (base-2 logarithm)
#[no_mangle]
pub extern "C" fn js_math_log2(x: f64) -> f64 {
    x.log2()
}

/// Math.log10(x) -> number (base-10 logarithm)
#[no_mangle]
pub extern "C" fn js_math_log10(x: f64) -> f64 {
    x.log10()
}

/// Math.sin(x) -> number
#[no_mangle]
pub extern "C" fn js_math_sin(x: f64) -> f64 { x.sin() }

/// Math.cos(x) -> number
#[no_mangle]
pub extern "C" fn js_math_cos(x: f64) -> f64 { x.cos() }

/// Math.tan(x) -> number
#[no_mangle]
pub extern "C" fn js_math_tan(x: f64) -> f64 { x.tan() }

/// Math.asin(x) -> number
#[no_mangle]
pub extern "C" fn js_math_asin(x: f64) -> f64 { x.asin() }

/// Math.acos(x) -> number
#[no_mangle]
pub extern "C" fn js_math_acos(x: f64) -> f64 { x.acos() }

/// Math.atan(x) -> number
#[no_mangle]
pub extern "C" fn js_math_atan(x: f64) -> f64 { x.atan() }

/// Math.atan2(y, x) -> number
#[no_mangle]
pub extern "C" fn js_math_atan2(y: f64, x: f64) -> f64 { y.atan2(x) }

/// Math.random() -> number (0 <= x < 1)
#[no_mangle]
pub extern "C" fn js_math_random() -> f64 {
    let mut rng = rand::thread_rng();
    rng.gen::<f64>()
}

/// Math.min(...array) -> number — find minimum value in an array
#[no_mangle]
pub extern "C" fn js_math_min_array(arr_ptr: i64) -> f64 {
    if arr_ptr == 0 {
        return f64::INFINITY;
    }
    let arr = arr_ptr as *const crate::ArrayHeader;
    let len = crate::array::js_array_length(arr) as usize;
    if len == 0 {
        return f64::INFINITY;
    }
    let mut result = f64::INFINITY;
    for i in 0..len {
        let num = crate::array::js_array_get_f64(arr, i as u32);
        if num.is_nan() {
            return f64::NAN;
        }
        if num < result {
            result = num;
        }
    }
    result
}

/// Math.max(...array) -> number — find maximum value in an array
#[no_mangle]
pub extern "C" fn js_math_max_array(arr_ptr: i64) -> f64 {
    if arr_ptr == 0 {
        return f64::NEG_INFINITY;
    }
    let arr = arr_ptr as *const crate::ArrayHeader;
    let len = crate::array::js_array_length(arr) as usize;
    if len == 0 {
        return f64::NEG_INFINITY;
    }
    let mut result = f64::NEG_INFINITY;
    for i in 0..len {
        let num = crate::array::js_array_get_f64(arr, i as u32);
        if num.is_nan() {
            return f64::NAN;
        }
        if num > result {
            result = num;
        }
    }
    result
}
