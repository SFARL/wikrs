//! Dev measurement: tally parser diagnostics across the cached differential
//! pages, to answer "which unsupported constructs actually drive the Reported
//! bucket, and is U-TABLE ever the *sole* reason a page is flagged?" Reliable
//! per the differential's lesson: reads raw `.wikitext` and calls `parse()`
//! directly (no dump-XML round-trip that would corrupt `<>&`).
//!
//! Run: `cargo run --example diag_tally`

use std::collections::BTreeMap;
use wikrs::diag::Severity;

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "tests/diff/cache".into());
    let mut pages = 0usize;
    let mut page_with: BTreeMap<&str, usize> = BTreeMap::new(); // unsupported code -> #pages
    let mut occ: BTreeMap<&str, usize> = BTreeMap::new(); // any code -> #occurrences
    let mut sole_table = Vec::new();

    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .expect("cache dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "wikitext"))
        .collect();
    paths.sort();

    for path in &paths {
        let wt = std::fs::read_to_string(path).unwrap();
        let parsed = wikrs::parser::parse(&wt);
        let slug = path.file_stem().unwrap().to_string_lossy().into_owned();
        pages += 1;

        let mut uns: BTreeMap<&str, usize> = BTreeMap::new();
        for d in &parsed.diagnostics {
            *occ.entry(d.code).or_default() += 1;
            if d.severity == Severity::Unsupported {
                *uns.entry(d.code).or_default() += 1;
            }
        }
        for code in uns.keys() {
            *page_with.entry(code).or_default() += 1;
        }
        let codes: Vec<&str> = uns.keys().copied().collect();
        if codes == ["U-TABLE"] {
            sole_table.push(slug.clone());
        }
        println!("{slug:28} {uns:?}");
    }

    println!("\n=== UNSUPPORTED code -> #pages flagged (of {pages}) ===");
    for (code, n) in &page_with {
        println!("  {code:12} {n} pages");
    }
    println!("\n=== all diagnostics -> #occurrences (incl. Warning, e.g. dropped templates) ===");
    for (code, n) in &occ {
        println!("  {code:12} {n}");
    }
    println!("\nPages where U-TABLE is the SOLE unsupported flag (table support would un-Report them):");
    println!("  {sole_table:?}");
}
