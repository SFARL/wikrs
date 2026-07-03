//! Dev scratch: reproduce a markdown round-trip fuzz find. Delete freely.
fn main() {
    let path = std::env::args().nth(1).expect("usage: md_repro <file>");
    let wt = std::fs::read_to_string(&path).unwrap();
    let parsed = wikrs::parser::parse(&wt);
    println!("AST: {:?}", parsed.nodes);
    let md = wikrs::render::markdown(&parsed.nodes);
    println!("MD: {md:?}");
    println!("intent: {:?}", wikrs::mdnorm::from_ast(&parsed.nodes));
}
