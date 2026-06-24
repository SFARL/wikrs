//! Streaming reader for Wikimedia XML dumps (`pages-articles-multistream.xml.bz2`).
//!
//! Yields one page at a time at constant memory, filtering to article
//! namespaces and skipping redirects.

use std::io::BufRead;

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
