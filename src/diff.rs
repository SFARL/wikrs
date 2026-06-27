//! Layer 2 differential (see `docs/TESTING.md`): bucket wikrs's plain-text output
//! for a page against ground-truth article prose (the visible text of Parsoid's
//! HTML) into the three headline numbers.
//!
//! **Precision-oriented by design.** wikrs deliberately drops templates (the D4
//! speed moat), so its text is a *subset* of Parsoid's — we never penalize it for
//! the omitted template content. Instead we ask: is everything wikrs *did* emit
//! faithful to the article? Pages wikrs flagged `Unsupported` are honestly out of
//! range (the differentiator vs silently-wrong extractors), not failures. The
//! omitted-content gap is reported separately as `coverage`, never against the
//! `Faithful` bucket.

use std::collections::HashSet;

/// Which of the three headline buckets a page falls into. Together they partition
/// every page, so the percentages sum to 100.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bucket {
    /// No `Unsupported` diagnostics *and* everything wikrs emitted is faithful to
    /// the ground truth — the "X% identical" number.
    Faithful,
    /// wikrs emitted text that disagrees with the ground truth and gave *no*
    /// diagnostic to warn of it — a silent error. The "Y%" number, the bucket the
    /// whole project exists to drive toward zero.
    Divergent,
    /// wikrs flagged ≥1 `Unsupported` construct — honestly out of declared range.
    /// The "Z%" number; not a failure.
    Reported,
}

/// Faithfulness threshold: the fraction of wikrs's phrase-shingles that must
/// appear in the ground truth for its output to count as faithful. Below 1.0 to
/// absorb benign normalization noise (entity decoding, punctuation spacing);
/// genuine fabrication/garbling scores far lower. Tunable once real-page
/// distributions are in hand.
const FAITHFUL_THRESHOLD: f64 = 0.90;

/// Word-precision fallback threshold: if nearly all of wikrs's *distinct words*
/// are corroborated, the page is faithful even when phrase-shingles differ —
/// exactly the table-cell case (same words, flattened in a different order than
/// Parsoid's grid). Separates reordering from genuine fabrication.
const WORD_FAITHFUL_THRESHOLD: f64 = 0.97;

/// Shingle width in words. Phrase-level: catches "wikrs emitted a phrase not in
/// the article" while staying robust to paragraph reflow and to wikrs omitting
/// whole template chunks.
const SHINGLE_K: usize = 3;

/// Classify one page. `has_unsupported` is whether wikrs emitted ≥1 `Unsupported`
/// diagnostic for it (the caller derives this from `parse().diagnostics`).
///
/// Precedence matters: a page wikrs *flagged* is `Reported` even if the text also
/// diverges — being honest about it is the whole point. Only a page wikrs thought
/// it handled cleanly (no diagnostic) yet got wrong is `Divergent`.
pub fn classify(wikrs_text: &str, truth_text: &str, has_unsupported: bool) -> Bucket {
    if has_unsupported {
        Bucket::Reported
    } else if is_faithful(wikrs_text, truth_text) {
        Bucket::Faithful
    } else {
        Bucket::Divergent
    }
}

/// Is everything wikrs emitted corroborated by the ground truth? Faithful when the
/// phrase-shingles match (strict — catches fabrication) OR nearly all distinct
/// words are present (order-robust — so table-cell reordering isn't a false
/// divergence).
pub fn is_faithful(wikrs_text: &str, truth_text: &str) -> bool {
    precision(wikrs_text, truth_text) >= FAITHFUL_THRESHOLD
        || word_precision(wikrs_text, truth_text) >= WORD_FAITHFUL_THRESHOLD
}

/// Fraction of wikrs's phrase-shingles that also occur in the ground truth — how
/// much of what wikrs emitted is corroborated by the article. Empty wikrs output
/// emitted nothing false, so it is vacuously faithful (precision 1.0); the
/// omitted content shows up in `coverage`, not here.
pub fn precision(wikrs_text: &str, truth_text: &str) -> f64 {
    let got = shingles(wikrs_text);
    if got.is_empty() {
        return 1.0;
    }
    let truth = shingles(truth_text);
    let hits = got.iter().filter(|s| truth.contains(*s)).count();
    hits as f64 / got.len() as f64
}

/// Fraction of the ground truth's phrase-shingles that wikrs reproduced — the
/// flip side of precision. Low coverage is expected and fine (dropped templates);
/// it is reported as a separate informational number, never against `Faithful`.
pub fn coverage(wikrs_text: &str, truth_text: &str) -> f64 {
    let truth = shingles(truth_text);
    if truth.is_empty() {
        return 1.0;
    }
    let got = shingles(wikrs_text);
    let hits = truth.iter().filter(|s| got.contains(*s)).count();
    hits as f64 / truth.len() as f64
}

/// Order-independent precision: the fraction of wikrs's *distinct words* that also
/// appear in the ground truth. Robust to reordering (table cells flattened in a
/// different order than Parsoid's grid), so it separates "same words, different
/// adjacency" from genuinely fabricated content. Empty output is vacuously 1.0.
pub fn word_precision(wikrs_text: &str, truth_text: &str) -> f64 {
    let got: HashSet<String> = word_vec(wikrs_text).into_iter().collect();
    if got.is_empty() {
        return 1.0;
    }
    let truth: HashSet<String> = word_vec(truth_text).into_iter().collect();
    let hits = got.iter().filter(|w| truth.contains(*w)).count();
    hits as f64 / got.len() as f64
}

/// Lowercased alphanumeric words, in order.
fn word_vec(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Join the words into overlapping `SHINGLE_K`-word phrases. Text shorter than
/// `SHINGLE_K` words yields a single shingle of the whole thing, so short pages
/// still compare.
fn shingles(text: &str) -> HashSet<String> {
    let words = word_vec(text);
    let mut set = HashSet::new();
    if words.is_empty() {
        return set;
    }
    if words.len() < SHINGLE_K {
        set.insert(words.join(" "));
        return set;
    }
    for w in words.windows(SHINGLE_K) {
        set.insert(w.join(" "));
    }
    set
}

/// Running tally over many pages — produces the three headline numbers plus the
/// informational coverage average.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Report {
    pub faithful: usize,
    pub divergent: usize,
    pub reported: usize,
}

impl Report {
    /// Fold one page's bucket into the tally.
    pub fn record(&mut self, bucket: Bucket) {
        match bucket {
            Bucket::Faithful => self.faithful += 1,
            Bucket::Divergent => self.divergent += 1,
            Bucket::Reported => self.reported += 1,
        }
    }

    /// Total pages tallied.
    pub fn total(&self) -> usize {
        self.faithful + self.divergent + self.reported
    }

    /// `(faithful%, divergent%, reported%)`. Zero pages -> all zero.
    pub fn percentages(&self) -> (f64, f64, f64) {
        let total = self.total();
        if total == 0 {
            return (0.0, 0.0, 0.0);
        }
        let pct = |n: usize| 100.0 * n as f64 / total as f64;
        (pct(self.faithful), pct(self.divergent), pct(self.reported))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faithful_subset_with_no_diagnostics_is_faithful() {
        // wikrs emits a clean subset of the article's prose (templates dropped).
        let wikrs = "Earth is the third planet from the Sun";
        let truth = "Earth is the third planet from the Sun . It has one moon and abundant water .";
        assert_eq!(classify(wikrs, truth, false), Bucket::Faithful);
    }

    #[test]
    fn any_unsupported_diagnostic_is_reported_even_if_text_matches() {
        // Honesty precedence: if wikrs flagged it, it lands in Reported regardless.
        let text = "Earth is the third planet from the Sun";
        assert_eq!(classify(text, text, true), Bucket::Reported);
    }

    #[test]
    fn fabricated_phrase_without_diagnostic_is_divergent() {
        // wikrs emitted a phrase absent from the article and gave no warning —
        // the silent-error bucket.
        let wikrs = "Earth is the flat center of the universe";
        let truth = "Earth is the third planet from the Sun in the Solar System";
        assert_eq!(classify(wikrs, truth, false), Bucket::Divergent);
    }

    #[test]
    fn precision_is_one_for_subset_and_for_empty_output() {
        let wikrs = "Earth is the third planet";
        let truth = "Earth is the third planet from the Sun and more";
        assert!((precision(wikrs, truth) - 1.0).abs() < 1e-9);
        // Empty wikrs output emitted nothing false -> vacuously faithful.
        assert!((precision("", truth) - 1.0).abs() < 1e-9);
        assert!(is_faithful("", truth));
    }

    #[test]
    fn low_coverage_does_not_make_a_faithful_page_divergent() {
        // The crux of the design: wikrs faithfully emits a small subset of a
        // template-heavy page. Coverage is low (expected), precision is perfect,
        // so the page is Faithful — never penalized for the dropped templates.
        let wikrs = "Earth is the third planet";
        let truth =
            "Earth is the third planet from the Sun and it has a large natural satellite moon";
        assert!(coverage(wikrs, truth) < 0.5);
        assert!(coverage(wikrs, truth) > 0.0);
        assert!((precision(wikrs, truth) - 1.0).abs() < 1e-9);
        assert_eq!(classify(wikrs, truth, false), Bucket::Faithful);
    }

    #[test]
    fn report_tallies_and_percentages_sum_to_one_hundred() {
        let mut r = Report::default();
        r.record(Bucket::Faithful);
        r.record(Bucket::Faithful);
        r.record(Bucket::Reported);
        r.record(Bucket::Divergent);
        assert_eq!(r.total(), 4);
        let (x, y, z) = r.percentages();
        assert!((x - 50.0).abs() < 1e-9);
        assert!((y - 25.0).abs() < 1e-9);
        assert!((z - 25.0).abs() < 1e-9);
        assert!((x + y + z - 100.0).abs() < 1e-9);
    }

    #[test]
    fn empty_report_is_all_zero() {
        let r = Report::default();
        assert_eq!(r.total(), 0);
        assert_eq!(r.percentages(), (0.0, 0.0, 0.0));
    }

    #[test]
    fn word_precision_rescues_reordered_table_cells() {
        // Table-cell flattening reorders words; the content is still corroborated,
        // so word-precision stays high though the phrase-shingles don't match.
        let wikrs = "Alice 30 Bob 25";
        let truth = "Name Age Alice Bob 30 25 and more rows of data here";
        assert!(precision(wikrs, truth) < 0.90, "shingles should differ");
        assert!(word_precision(wikrs, truth) > 0.97, "every word is present");
        assert!(is_faithful(wikrs, truth), "faithful via the word fallback");
    }

    #[test]
    fn genuinely_different_words_stay_divergent() {
        // Different *words*, not just order — a real silent error, not reordering.
        let wikrs = "Berlin is the capital of Germany";
        let truth = "Paris is the capital of France and a city";
        assert!(word_precision(wikrs, truth) < 0.97);
        assert!(!is_faithful(wikrs, truth));
    }
}
