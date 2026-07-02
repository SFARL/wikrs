//! Streaming reader for Wikimedia XML dumps (`pages-articles-multistream.xml.bz2`).
//!
//! Yields one page at a time at constant memory, filtering to article
//! namespaces and skipping redirects.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;

/// One page from a dump. `text` is raw wikitext.
#[derive(Debug, Clone)]
pub struct Page {
    pub title: String,
    pub namespace: i32,
    pub redirect: bool,
    pub text: String,
}

impl Page {
    /// A real article: main namespace and not a redirect.
    pub fn is_article(&self) -> bool {
        self.namespace == 0 && !self.redirect
    }
}

/// Streaming iterator over `<page>` elements. Constant memory per page.
pub struct Pages<R: BufRead> {
    reader: Reader<R>,
    buf: Vec<u8>,
}

impl<R: BufRead> Pages<R> {
    pub fn new(read: R) -> Self {
        Pages {
            reader: Reader::from_reader(read),
            buf: Vec::new(),
        }
    }
}

/// Open a dump file, transparently decompressing multistream `.bz2`.
pub fn open(path: &Path) -> anyhow::Result<Pages<Box<dyn BufRead>>> {
    let file = File::open(path)?;
    let reader: Box<dyn BufRead> = if path.extension().is_some_and(|e| e == "bz2") {
        Box::new(BufReader::new(bzip2::read::MultiBzDecoder::new(file)))
    } else {
        Box::new(BufReader::new(file))
    };
    Ok(Pages::new(reader))
}

/// Resolve an XML entity-reference *name* (the part between `&` and `;`) to its
/// character: the five predefined XML entities plus numeric character
/// references (`#233`, `#x41`). Anything else is ill-formed in a dump — XML has
/// no other built-ins, and MediaWiki dumps declare no custom entities.
fn resolve_entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let num = name.strip_prefix('#')?;
            let code = match num.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => num.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}

/// Which `<page>` child we are currently accumulating text into.
#[derive(Default)]
enum Field {
    #[default]
    None,
    Title,
    Ns,
    Text,
}

impl<R: BufRead> Iterator for Pages<R> {
    type Item = anyhow::Result<Page>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut page: Option<Page> = None;
        let mut field = Field::None;
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => match e.name().as_ref() {
                    b"page" => {
                        page = Some(Page {
                            title: String::new(),
                            namespace: 0,
                            redirect: false,
                            text: String::new(),
                        })
                    }
                    b"title" => field = Field::Title,
                    b"ns" => field = Field::Ns,
                    b"text" => field = Field::Text,
                    _ => {}
                },
                Ok(Event::Empty(e)) if e.name().as_ref() == b"redirect" => {
                    if let Some(p) = page.as_mut() {
                        p.redirect = true;
                    }
                }
                Ok(Event::Text(t)) => {
                    if let Some(p) = page.as_mut() {
                        // 0.40: decode bytes -> str, then resolve XML entities.
                        let decoded = match t.decode() {
                            Ok(d) => d,
                            Err(e) => return Some(Err(e.into())),
                        };
                        let s = match quick_xml::escape::unescape(&decoded) {
                            Ok(s) => s,
                            Err(e) => return Some(Err(e.into())),
                        };
                        match field {
                            Field::Title => p.title.push_str(&s),
                            Field::Text => p.text.push_str(&s),
                            Field::Ns => p.namespace = s.trim().parse().unwrap_or(0),
                            Field::None => {}
                        }
                    }
                }
                // quick-xml emits entity references (`&amp;` …) as separate
                // GeneralRef events — they never appear inside Text events. Real
                // dumps escape every `&`/`<`/`>` in wikitext, so dropping these
                // corrupts pages (`&lt;ref&gt;` never becomes `<ref>`). Resolve
                // them into the current field; an unresolvable entity inside a
                // page's title/text is ill-formed input and surfaces as an Err
                // (refs elsewhere, e.g. siteinfo we don't consume, are skipped).
                Ok(Event::GeneralRef(e)) => {
                    if let Some(p) = page.as_mut() {
                        let dest = match field {
                            Field::Title => Some(&mut p.title),
                            Field::Text => Some(&mut p.text),
                            Field::Ns | Field::None => None,
                        };
                        if let Some(dest) = dest {
                            let name = match e.decode() {
                                Ok(n) => n,
                                Err(err) => return Some(Err(err.into())),
                            };
                            match resolve_entity(&name) {
                                Some(c) => dest.push(c),
                                None => {
                                    return Some(Err(anyhow::anyhow!(
                                        "unresolvable XML entity `&{name};` in page {:?} (ill-formed dump)",
                                        p.title
                                    )))
                                }
                            }
                        }
                    }
                }
                Ok(Event::End(e)) => match e.name().as_ref() {
                    b"title" | b"ns" | b"text" => field = Field::None,
                    b"page" => return page.map(Ok),
                    _ => {}
                },
                Ok(Event::Eof) => return None,
                Err(e) => return Some(Err(e.into())),
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const SAMPLE: &str = r#"
<mediawiki>
  <page><title>Earth</title><ns>0</ns>
    <revision><text>Earth is the '''third''' planet.</text></revision>
  </page>
  <page><title>Talk:Earth</title><ns>1</ns>
    <revision><text>discuss here</text></revision>
  </page>
  <page><title>USA</title><ns>0</ns>
    <redirect title="United States" />
    <revision><text>#REDIRECT [[United States]]</text></revision>
  </page>
</mediawiki>"#;

    #[test]
    fn parses_pages_with_fields() {
        let pages: Vec<Page> = Pages::new(Cursor::new(SAMPLE))
            .collect::<anyhow::Result<_>>()
            .unwrap();
        assert_eq!(pages.len(), 3);
        assert_eq!(pages[0].title, "Earth");
        assert_eq!(pages[0].namespace, 0);
        assert!(!pages[0].redirect);
        assert_eq!(pages[0].text, "Earth is the '''third''' planet.");
        assert!(pages[2].redirect);
    }

    #[test]
    fn resolves_xml_entities_in_title_and_text() {
        // Real dumps escape every `&`, `<`, `>` in wikitext. quick-xml (0.37+)
        // emits entity references as separate GeneralRef events, NOT inside Text
        // events — dropping them corrupts the wikitext: `&lt;ref&gt;` never
        // becomes `<ref>`, so ref-stripping silently stops working downstream.
        let xml = r##"<mediawiki><page><title>AT&amp;T</title><ns>0</ns>
            <revision><text>A &amp; B &lt;ref&gt;c&lt;/ref&gt; &quot;q&quot; &#233;&#x41;</text></revision>
        </page></mediawiki>"##;
        let pages: Vec<Page> = Pages::new(Cursor::new(xml))
            .collect::<anyhow::Result<_>>()
            .unwrap();
        assert_eq!(pages[0].title, "AT&T");
        assert_eq!(pages[0].text, "A & B <ref>c</ref> \"q\" \u{e9}A");
    }

    #[test]
    fn unknown_entity_in_page_is_an_error_not_a_silent_drop() {
        // An unresolvable entity means the dump is ill-formed; surfacing an Err
        // (instead of silently dropping bytes) is what lets the CLI fail loudly.
        let xml = "<mediawiki><page><title>X</title><ns>0</ns>\
            <revision><text>a &bogus; b</text></revision></page></mediawiki>";
        let res: anyhow::Result<Vec<Page>> = Pages::new(Cursor::new(xml)).collect();
        assert!(res.is_err(), "unknown entity must surface as an error");
    }

    #[test]
    fn is_article_filters_ns_and_redirects() {
        let pages: Vec<Page> = Pages::new(Cursor::new(SAMPLE))
            .collect::<anyhow::Result<_>>()
            .unwrap();
        let articles: Vec<&str> = pages
            .iter()
            .filter(|p| p.is_article())
            .map(|p| p.title.as_str())
            .collect();
        assert_eq!(articles, ["Earth"]); // ns1 and the redirect are excluded
    }
}
