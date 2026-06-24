//! Streaming reader for Wikimedia XML dumps (`pages-articles-multistream.xml.bz2`).
//!
//! Yields one page at a time at constant memory, filtering to article
//! namespaces and skipping redirects.
//!
//! Implemented in Stage 1 — see `docs/stages/stage-1-extractor.md` (Task 1).
