#![no_main]

use libfuzzer_sys::fuzz_target;

// The default engine's full path — `parser::parse` then `render::plain` — must
// never panic, hang, or OOM on any input. `strip` has its own target; this one
// covers the tokenizer/parser/renderer the CLI actually defaults to.
// Run: cargo +nightly fuzz run parse
fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let parsed = wikrs::parser::parse(s);
        let _ = wikrs::render::plain(&parsed.nodes);
    }
});
