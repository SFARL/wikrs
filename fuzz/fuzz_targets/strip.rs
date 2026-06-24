#![no_main]

use libfuzzer_sys::fuzz_target;

// `extract::strip` must never panic, hang, or OOM on any input.
// Run: cargo +nightly fuzz run strip
fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = wikrs::extract::strip(s);
    }
});
