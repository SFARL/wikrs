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
