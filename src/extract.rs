//! Stage 1: lossy `wikitext -> plain text` extraction.
//!
//! Deliberately *not* a parser — a fast, targeted stripper that mirrors
//! WikiExtractor's behavior (drop templates/tables/refs, keep link anchor
//! text, strip markup). The real AST engine lands in Stage 2.
//!
//! Implemented in Stage 1 — see `docs/stages/stage-1-extractor.md` (Task 3).
