//! **wikrs** — fast, honest wikitext extraction and parsing.
//!
//! Turns MediaWiki wikitext (the markup inside Wikipedia XML dumps) into clean
//! plain text or a structured AST, and **emits a diagnostic when it hits input
//! it can't faithfully handle instead of silently corrupting the output**.
//! Validated on the full English Wikipedia (7.19M articles, 98.0% of pages
//! convert with zero residual markup).
//!
//! # Quick start
//!
//! ```
//! // Parse wikitext into an AST + diagnostics, then render plain text.
//! let parsed = wikrs::parser::parse("'''Earth''' is a [[Planet|planet]].");
//! assert!(parsed.diagnostics.is_empty());
//! assert_eq!(wikrs::render::plain(&parsed.nodes), "Earth is a planet.");
//!
//! // Or the Stage 1 one-shot stripper (fast, lossy, no diagnostics).
//! assert_eq!(
//!     wikrs::extract::strip("'''Earth''' is a [[Planet|planet]]."),
//!     "Earth is a planet."
//! );
//! ```
//!
//! Reading a whole dump ([`dump::open`], or [`dump::open_multistream`] for
//! parallel bz2 decoding) yields [`dump::Page`]s whose `text` feeds the same
//! two entry points. The `wikrs` CLI wraps exactly this pipeline.
//!
//! Pre-1.0: the API surface is the modules documented below; items marked
//! `#[doc(hidden)]` are internal plumbing with no stability promise.

#![warn(missing_docs)]

pub mod ast;
pub mod diag;
pub mod dump;
pub mod extract;
pub mod parser;
pub mod render;

// Internal plumbing, public only for the CLI / dev tooling (xtask, tests).
// No semver promise — do not build on these.
#[doc(hidden)]
pub mod diff;
#[doc(hidden)]
pub mod output;

// Crate-internal machinery (tokenizer feeds parser; entities feed render/strip).
pub(crate) mod entities;
pub(crate) mod tokenizer;
