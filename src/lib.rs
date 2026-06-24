//! wikrs — fast, honest wikitext extraction and parsing.
//!
//! Early development; the public API is unstable and changes between commits.
//! See `docs/DESIGN.md` for architecture and `docs/stages/` for the roadmap.
//!
//! Stage 1 builds the two modules below into a `wikitext -> plain text`
//! extractor. Stage 2 adds `tokenizer` / `parser` / `ast` / `render` / `diag`.

pub mod dump;
pub mod extract;
