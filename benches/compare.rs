//! Speed comparison on a representative article (see `docs/TESTING.md`).
//!
//! Baseline: `parse_wiki_text` — the most serious community Rust wikitext
//! parser (0.1.5, unmaintained since 2018), used here purely as a dev
//! benchmark reference. wikrs's own `extract::strip` is added to this group
//! once Stage 1 lands it (plan Task 6).

use criterion::{criterion_group, criterion_main, Criterion, Throughput};

const SAMPLE: &str = include_str!("../tests/fixtures/sample_article.wikitext");

fn bench_baselines(c: &mut Criterion) {
    let mut group = c.benchmark_group("wikitext/sample_article");
    group.throughput(Throughput::Bytes(SAMPLE.len() as u64));

    let config = parse_wiki_text::Configuration::default();
    group.bench_function("parse_wiki_text", |b| {
        b.iter(|| {
            let output = config.parse(std::hint::black_box(SAMPLE));
            std::hint::black_box(output.nodes.len())
        })
    });

    // wikrs's own Stage 1 extractor, on the same input, in the same group.
    group.bench_function("wikrs_strip", |b| {
        b.iter(|| std::hint::black_box(wikrs::extract::strip(std::hint::black_box(SAMPLE))))
    });

    group.finish();
}

criterion_group!(benches, bench_baselines);
criterion_main!(benches);
