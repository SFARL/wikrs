use wikrs::extract::strip;

#[test]
fn article_snapshot() {
    let wikitext = "\
'''Earth''' is the [[Planet|third planet]] from the Sun.<ref>cite</ref>

== History ==
{{Infobox planet|age=4.5e9}}
* Formed ~4.5 billion years ago
* Has [[File:Earth.jpg|thumb|a moon]] one moon

See [https://nasa.gov NASA].";
    insta::assert_snapshot!(strip(wikitext));
}
