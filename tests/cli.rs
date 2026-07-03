use std::process::Command;

/// Write `dump_xml` to a temp file and run the `wikrs` binary over it.
fn run(extra: &[&str], dump_xml: &str, name: &str) -> std::process::Output {
    let dir = std::env::temp_dir().join("wikrs_cli_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, dump_xml).unwrap();
    let mut args = vec!["--input".to_string(), path.to_string_lossy().into_owned()];
    args.extend(extra.iter().map(|s| s.to_string()));
    Command::new(env!("CARGO_BIN_EXE_wikrs"))
        .args(&args)
        .output()
        .unwrap()
}

#[test]
fn extracts_clean_text() {
    let xml = "<mediawiki><page><title>Earth</title><ns>0</ns>\
        <revision><text>'''Earth''' is a [[Planet|planet]].</text></revision></page></mediawiki>";
    let out = run(&["--engine", "strip", "--format", "text"], xml, "a.xml");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(s.contains("Earth is a planet."), "got: {s}");
}

#[test]
fn ast_engine_extracts_text() {
    let xml = "<mediawiki><page><title>Earth</title><ns>0</ns>\
        <revision><text>'''Earth''' is a [[Planet|planet]].</text></revision></page></mediawiki>";
    let out = run(&["--engine", "ast"], xml, "ast.xml");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(s.contains("Earth is a planet."), "got: {s}");
}

#[test]
fn corrupt_dump_is_a_hard_error_not_a_silent_skip() {
    // An unresolvable entity makes the dump reader yield Err mid-stream. The old
    // CLI `filter_map(Result::ok)` swallowed it — exit 0 with silently truncated
    // output. Honest behavior: fail loudly, say what broke, exit non-zero.
    let xml = "<mediawiki>\
        <page><title>Alpha</title><ns>0</ns><revision><text>Alpha body.</text></revision></page>\
        <page><title>Beta</title><ns>0</ns><revision><text>Beta &bogus; body.</text></revision></page>\
        <page><title>Gamma</title><ns>0</ns><revision><text>Gamma body.</text></revision></page>\
        </mediawiki>";
    let out = run(&["--format", "jsonl"], xml, "corrupt.xml");
    assert!(
        !out.status.success(),
        "a corrupt dump must not exit 0 (output would be silently truncated)"
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("dump"), "stderr should say what failed: {err}");
}

#[test]
fn streams_articles_in_dump_order() {
    // Streaming rewrite guard: every article present, in dump order; non-articles
    // (talk ns, redirects) still filtered. Behavior must match the old
    // collect-everything pipeline exactly.
    let xml = "<mediawiki>\
        <page><title>Alpha</title><ns>0</ns><revision><text>one</text></revision></page>\
        <page><title>Talk:Skip</title><ns>1</ns><revision><text>talk</text></revision></page>\
        <page><title>Beta</title><ns>0</ns><revision><text>two</text></revision></page>\
        <page><title>Redir</title><ns>0</ns><redirect title=\"Beta\" />\
            <revision><text>#REDIRECT [[Beta]]</text></revision></page>\
        <page><title>Gamma</title><ns>0</ns><revision><text>three</text></revision></page>\
        </mediawiki>";
    let out = run(&["--format", "jsonl"], xml, "order.xml");
    assert!(out.status.success());
    let s = String::from_utf8(out.stdout).unwrap();
    let titles: Vec<String> = s
        .lines()
        .map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).unwrap();
            v["title"].as_str().unwrap().to_owned()
        })
        .collect();
    assert_eq!(titles, ["Alpha", "Beta", "Gamma"], "articles in dump order");
}

#[test]
fn index_flag_parallel_decode_matches_sequential() {
    // The README's headline flag: `--index` must produce byte-identical output
    // to the sequential path, end-to-end through the real binary. Build a tiny
    // multistream dump (header stream + page streams + trailer, one bz2 stream
    // each) plus its offset index, run both ways, compare.
    use std::io::Write as _;
    fn bz(s: &str) -> Vec<u8> {
        let mut e = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::fast());
        e.write_all(s.as_bytes()).unwrap();
        e.finish().unwrap()
    }
    let dir = std::env::temp_dir().join("wikrs_cli_test");
    std::fs::create_dir_all(&dir).unwrap();
    let mut dump = bz("<mediawiki>\n");
    let mut index = String::new();
    let mut id = 0;
    for s in 0..3 {
        let offset = dump.len();
        let mut chunk = String::new();
        for p in 0..2 {
            id += 1;
            chunk.push_str(&format!(
                "<page><title>P{id}</title><ns>0</ns>\
                 <revision><text>body {id} (stream {s} page {p})</text></revision></page>\n"
            ));
            index.push_str(&format!("{offset}:{id}:P{id}\n"));
        }
        if s == 2 {
            chunk.push_str("</mediawiki>\n");
        }
        dump.extend_from_slice(&bz(&chunk));
    }
    let dump_path = dir.join("ms.xml.bz2");
    let index_path = dir.join("ms-index.txt");
    std::fs::write(&dump_path, &dump).unwrap();
    std::fs::write(&index_path, index).unwrap();

    let run_with = |extra: &[&str]| {
        let mut args = vec!["--input", dump_path.to_str().unwrap(), "--format", "jsonl"];
        args.extend(extra);
        Command::new(env!("CARGO_BIN_EXE_wikrs"))
            .args(&args)
            .output()
            .unwrap()
    };
    let seq = run_with(&[]);
    let par = run_with(&["--index", index_path.to_str().unwrap()]);
    assert!(
        seq.status.success() && par.status.success(),
        "seq: {}\npar: {}",
        String::from_utf8_lossy(&seq.stderr),
        String::from_utf8_lossy(&par.stderr)
    );
    assert_eq!(
        seq.stdout, par.stdout,
        "--index output must be byte-identical to sequential"
    );
    assert_eq!(
        String::from_utf8_lossy(&par.stdout).lines().count(),
        6,
        "all 6 articles, in dump order"
    );
}

#[test]
fn reports_conversion_rate() {
    let xml = "<mediawiki>\
        <page><title>A</title><ns>0</ns><revision><text>clean text here</text></revision></page>\
        <page><title>B</title><ns>0</ns><revision><text>stray }} brace</text></revision></page>\
        </mediawiki>";
    let out = run(&["--stats"], xml, "b.xml");
    assert!(out.status.success());
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.contains("pages=2"), "got: {err}");
    assert!(err.contains("clean=1"), "got: {err}"); // "stray }} brace" leaves residual }}
}

#[test]
fn sections_format_emits_parseable_jsonl() {
    // Stage 3 (LLM output): one JSON object per page with flat, level-tagged
    // sections — the RAG-chunking contract from stage-3-llm-output.md.
    let xml = "<mediawiki><page><title>Earth</title><ns>0</ns>\
        <revision><text>Lead prose.\n\n== History ==\n\nOld times.\n\n=== Deep ===\n\nFine.</text>\
        </revision></page></mediawiki>";
    let out = run(&["--format", "sections"], xml, "sections.xml");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let s = String::from_utf8(out.stdout).unwrap();
    let line = s.lines().next().expect("one line per page");
    let v: serde_json::Value = serde_json::from_str(line).expect("valid JSON per line");
    assert_eq!(v["title"].as_str(), Some("Earth"));
    let secs = v["sections"].as_array().unwrap();
    assert_eq!(secs.len(), 3, "lead + h2 + h3: {line}");
    assert_eq!(secs[0]["level"].as_u64(), Some(0));
    assert_eq!(secs[1]["heading"].as_str(), Some("History"));
    assert_eq!(secs[1]["text"].as_str(), Some("Old times."));
    assert_eq!(secs[2]["level"].as_u64(), Some(3));
}

#[test]
fn sections_format_rejects_strip_engine_and_stats() {
    let xml = "<mediawiki><page><title>T</title><ns>0</ns>\
        <revision><text>body</text></revision></page></mediawiki>";
    // strip has no AST to sectionize — fail loudly, no silent fallback.
    let out = run(
        &["--format", "sections", "--engine", "strip"],
        xml,
        "s1.xml",
    );
    assert!(!out.status.success(), "strip+sections must be an error");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("ast"), "stderr should point at the fix: {err}");
    // --stats measures plain-text cleanliness; sections would skew it.
    let out = run(&["--format", "sections", "--stats"], xml, "s2.xml");
    assert!(!out.status.success(), "stats+sections must be an error");
}

#[test]
fn markdown_format_renders_structured_pages() {
    let xml = "<mediawiki><page><title>A*B</title><ns>0</ns>\
        <revision><text>'''Earth''' is a [[Planet|planet]].\n\n== History ==\n\nOld.</text>\
        </revision></page></mediawiki>";
    let out = run(&["--format", "markdown"], xml, "md.xml");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(s.contains("# A\\*B"), "escaped title as h1: {s}");
    assert!(
        s.contains("**Earth** is a [planet](./Planet)."),
        "body: {s}"
    );
    assert!(s.contains("## History"), "section heading: {s}");
}

#[test]
fn markdown_format_rejects_strip_engine_and_stats() {
    let xml = "<mediawiki><page><title>T</title><ns>0</ns>\
        <revision><text>body</text></revision></page></mediawiki>";
    let out = run(
        &["--format", "markdown", "--engine", "strip"],
        xml,
        "m1.xml",
    );
    assert!(!out.status.success(), "strip+markdown must be an error");
    assert!(String::from_utf8_lossy(&out.stderr).contains("ast"));
    let out = run(&["--format", "markdown", "--stats"], xml, "m2.xml");
    assert!(!out.status.success(), "stats+markdown must be an error");
}
