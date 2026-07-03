//! AST → GFM markdown (Stage 3 M-line).
//!
//! Correctness contract: the round-trip harness
//! (`tests/markdown_roundtrip.rs`) — pulldown-cmark must parse this module's
//! output back to the same normal form `mdnorm::from_ast` declares.
//! M1 stub: renders nothing, so the harness starts RED by construction.

use crate::ast::Node;

/// Render nodes to GFM markdown.
pub fn markdown(nodes: &[Node]) -> String {
    let _ = nodes;
    String::new()
}
