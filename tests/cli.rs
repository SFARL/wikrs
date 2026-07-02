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
