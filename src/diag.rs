//! Diagnostics (DESIGN.md §7). The engine's honesty mechanism: when it meets a
//! construct outside the declared support range, it emits an `Unsupported`
//! diagnostic with the original source span — instead of silently producing
//! something plausible but wrong.

use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    /// A genuine error inside the supported range.
    Error,
    /// Recoverable oddity; processing continued (possibly degraded).
    Warning,
    /// A construct we deliberately do not handle yet — reported, not guessed.
    Unsupported,
}

/// One diagnostic, locatable back to the source by `span` (byte range).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Stable machine code, e.g. `"U-TEMPLATE"`.
    pub code: &'static str,
    pub span: Range<usize>,
    pub message: String,
}

impl Diagnostic {
    pub fn unsupported(code: &'static str, span: Range<usize>, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Unsupported,
            code,
            span,
            message: message.into(),
        }
    }
}
