//! Dev: dump wikrs AST render + diagnostics (with source snippets) for one
//! `.wikitext` file. Direct `parse()` — reliable (no dump-XML round-trip).
//! Run: `cargo run --example show_page <file.wikitext>`

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: show_page <file.wikitext>");
    let wt = std::fs::read_to_string(&path).unwrap();
    let parsed = wikrs::parser::parse(&wt);
    let text = wikrs::render::plain(&parsed.nodes);

    println!("=== {} DIAGNOSTICS ===", parsed.diagnostics.len());
    for d in &parsed.diagnostics {
        let snip: String = wt[d.span.clone()].chars().take(70).collect();
        println!("  [{:?}] {} {:?}: {:?}", d.severity, d.code, d.span, snip);
    }
    println!("\n=== RENDERED PLAIN ({} bytes) ===", text.len());
    let head: String = text.chars().take(1400).collect();
    println!("{head}");
}
