#![no_main]

use libfuzzer_sys::fuzz_target;

#[path = "../../tests/support/pulldown_nf.rs"]
mod pulldown_nf;

// Stage 3 M-line safety+correctness target: for ANY input, parse → render
// markdown → an independent GFM parser must read back exactly the declared
// normal form. Catches escaping leaks the hand corpus missed.
// Run: cargo +nightly fuzz run markdown_roundtrip
fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let parsed = wikrs::parser::parse(s);
        let intent = wikrs::mdnorm::from_ast(&parsed.nodes);
        let md = wikrs::render::markdown(&parsed.nodes);
        let actual = pulldown_nf::markdown_to_nf(&md);
        assert_eq!(intent, actual, "round-trip mismatch\n--- md ---\n{md}");
    }
});
