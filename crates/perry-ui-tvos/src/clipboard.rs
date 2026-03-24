// Clipboard is not available on tvOS (no UIPasteboard)

/// Read from clipboard — returns TAG_UNDEFINED on tvOS
pub fn read() -> f64 {
    f64::from_bits(0x7FFC_0000_0000_0001) // TAG_UNDEFINED
}

/// Write to clipboard — no-op on tvOS
pub fn write(_text_ptr: *const u8) {}
