//! Streaming reader for Wikimedia XML dumps (`pages-articles-multistream.xml.bz2`).
//!
//! Yields one page at a time at constant memory, filtering to article
//! namespaces and skipping redirects.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{sync_channel, Receiver};
use std::sync::Arc;
use std::thread::JoinHandle;

use anyhow::Context;
use quick_xml::events::Event;
use quick_xml::Reader;

/// One page from a dump. `text` is raw wikitext.
#[derive(Debug, Clone)]
pub struct Page {
    /// The page title.
    pub title: String,
    /// MediaWiki namespace number (`0` = main/article namespace).
    pub namespace: i32,
    /// Whether the page is a `#REDIRECT` stub.
    pub redirect: bool,
    /// The page's raw wikitext.
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
    /// Wrap an XML reader (already decompressed) in a page iterator.
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

/// Open a *multistream* dump with parallel bz2 decoding, driven by its
/// companion index file (`offset:pageid:title` lines, `.txt` or `.txt.bz2`).
/// A multistream dump is a concatenation of independent bz2 streams (one per
/// ~100 pages) — the index gives each stream's byte offset, so N worker
/// threads decompress different streams concurrently while this reader
/// reassembles them in dump order. Identical output to [`open`]; the wall-time
/// win is bounded by whatever downstream consumes the XML.
pub fn open_multistream(dump: &Path, index: &Path) -> anyhow::Result<Pages<Box<dyn BufRead>>> {
    let dump_len = std::fs::metadata(dump)
        .with_context(|| format!("stat dump {}", dump.display()))?
        .len();
    let ranges = multistream_ranges(index, dump_len)?;
    let reader = ParallelBzReader::spawn(dump.to_owned(), ranges);
    Ok(Pages::new(Box::new(BufReader::new(reader))))
}

/// Parse a multistream index into the byte ranges of the dump's bz2 streams.
/// Index lines are `offset:pageid:title` (~100 pages share one offset); the
/// leading header stream (byte 0) and the trailing stream are included even
/// though the index never names them.
fn multistream_ranges(index: &Path, dump_len: u64) -> anyhow::Result<Vec<(u64, u64)>> {
    let file = File::open(index).with_context(|| format!("open index {}", index.display()))?;
    let reader: Box<dyn BufRead> = if index.extension().is_some_and(|e| e == "bz2") {
        Box::new(BufReader::new(bzip2::read::MultiBzDecoder::new(file)))
    } else {
        Box::new(BufReader::new(file))
    };
    let mut offsets: Vec<u64> = Vec::new();
    for line in reader.lines() {
        let line = line.context("read index line")?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let off: u64 = line
            .split(':')
            .next()
            .unwrap_or("")
            .parse()
            .with_context(|| format!("bad index line (want offset:pageid:title): {line:?}"))?;
        // ~100 consecutive lines share one offset — cheap pre-dedupe
        if offsets.last() != Some(&off) {
            offsets.push(off);
        }
    }
    if offsets.is_empty() {
        anyhow::bail!("index {} contains no offsets", index.display());
    }
    offsets.sort_unstable();
    offsets.dedup();
    if *offsets.last().unwrap() >= dump_len {
        anyhow::bail!(
            "index offset {} is beyond the dump ({} bytes) — index for a different dump?",
            offsets.last().unwrap(),
            dump_len
        );
    }
    let mut bounds = Vec::with_capacity(offsets.len() + 2);
    if offsets[0] != 0 {
        bounds.push(0); // the header stream the index never lists
    }
    bounds.extend_from_slice(&offsets);
    bounds.push(dump_len);
    Ok(bounds.windows(2).map(|w| (w[0], w[1])).collect())
}

/// Decompress the bz2 stream(s) in `path[start..end)`.
fn decode_range(path: &Path, start: u64, end: u64) -> std::io::Result<Vec<u8>> {
    let mut f = File::open(path)?;
    f.seek(SeekFrom::Start(start))?;
    let mut out = Vec::new();
    bzip2::read::MultiBzDecoder::new(f.take(end - start))
        .read_to_end(&mut out)
        .map_err(|e| {
            std::io::Error::new(
                e.kind(),
                format!("multistream chunk at bytes {start}..{end}: {e}"),
            )
        })?;
    Ok(out)
}

/// `Read` over a multistream dump: worker threads decompress stream ranges in
/// parallel; this end reorders their chunks back into dump order. Memory stays
/// bounded — the sync channel stalls workers once `2 × threads` chunks are in
/// flight, and each chunk is ~100 pages of text.
struct ParallelBzReader {
    rx: Receiver<(usize, std::io::Result<Vec<u8>>)>,
    /// Chunks that arrived ahead of `next` (bounded by channel cap + threads).
    pending: HashMap<usize, std::io::Result<Vec<u8>>>,
    cur: Vec<u8>,
    pos: usize,
    next: usize,
    total: usize,
    _workers: Vec<JoinHandle<()>>,
}

impl ParallelBzReader {
    fn spawn(path: PathBuf, ranges: Vec<(u64, u64)>) -> Self {
        // Leave headroom for the XML-parse thread and rayon's render pool; the
        // OS schedules the overlap. Decode dominates, so more threads win
        // until the parser can't keep up anyway.
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .saturating_sub(2)
            .clamp(2, 12)
            .min(ranges.len().max(1));
        let total = ranges.len();
        let (tx, rx) = sync_channel(threads * 2);
        let ranges = Arc::new(ranges);
        let counter = Arc::new(AtomicUsize::new(0));
        let mut workers = Vec::with_capacity(threads);
        for _ in 0..threads {
            let tx = tx.clone();
            let ranges = Arc::clone(&ranges);
            let counter = Arc::clone(&counter);
            let path = path.clone();
            workers.push(std::thread::spawn(move || loop {
                let i = counter.fetch_add(1, Ordering::Relaxed);
                let Some(&(start, end)) = ranges.get(i) else {
                    break;
                };
                let res = decode_range(&path, start, end);
                if tx.send((i, res)).is_err() {
                    break; // reader dropped — stop quietly
                }
            }));
        }
        ParallelBzReader {
            rx,
            pending: HashMap::new(),
            cur: Vec::new(),
            pos: 0,
            next: 0,
            total,
            _workers: workers,
        }
    }
}

impl Read for ParallelBzReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        loop {
            if self.pos < self.cur.len() && !buf.is_empty() {
                let n = (self.cur.len() - self.pos).min(buf.len());
                buf[..n].copy_from_slice(&self.cur[self.pos..self.pos + n]);
                self.pos += n;
                return Ok(n);
            }
            if buf.is_empty() || self.next == self.total {
                return Ok(0); // zero-len read or true EOF
            }
            // Fetch chunk `next`, stashing any that arrive out of order.
            let chunk = loop {
                if let Some(c) = self.pending.remove(&self.next) {
                    break c;
                }
                match self.rx.recv() {
                    Ok((i, c)) if i == self.next => break c,
                    Ok((i, c)) => {
                        self.pending.insert(i, c);
                    }
                    Err(_) => {
                        return Err(std::io::Error::other(
                            "multistream decode workers exited before delivering all chunks",
                        ))
                    }
                }
            };
            self.cur = chunk?;
            self.pos = 0;
            self.next += 1;
        }
    }
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

    /// Compress `data` as one standalone bz2 stream.
    fn bz(data: &str) -> Vec<u8> {
        use std::io::Write as _;
        let mut enc = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::fast());
        enc.write_all(data.as_bytes()).unwrap();
        enc.finish().unwrap()
    }

    /// Build a synthetic multistream dump in a temp dir: a header stream, N
    /// page streams, a trailing `</mediawiki>` stream — plus the companion
    /// index (`offset:pageid:title`, duplicate offsets like the real format).
    /// Returns (dump_path, index_path).
    fn make_multistream(name: &str, streams: usize, pages_per: usize) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("wikrs_ms_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut dump: Vec<u8> = bz("<mediawiki>\n");
        let mut index = String::new();
        let mut id = 0usize;
        for s in 0..streams {
            let offset = dump.len();
            let mut chunk = String::new();
            for p in 0..pages_per {
                id += 1;
                // varying body sizes so decode times differ (exercises reordering);
                // an entity so the parallel path proves entity handling too
                let filler = "x".repeat(1 + (id * 37) % 900);
                chunk.push_str(&format!(
                    "<page><title>Page {id}: s{s}p{p}</title><ns>0</ns>\
                     <revision><text>body {id} &amp; {filler}</text></revision></page>\n"
                ));
                index.push_str(&format!("{offset}:{id}:Page {id}: s{s}p{p}\n"));
            }
            if s == streams - 1 {
                chunk.push_str("</mediawiki>\n");
            }
            dump.extend_from_slice(&bz(&chunk));
        }
        let dump_path = dir.join(format!("{name}.xml.bz2"));
        let index_path = dir.join(format!("{name}-index.txt"));
        std::fs::write(&dump_path, &dump).unwrap();
        std::fs::write(&index_path, index).unwrap();
        (dump_path, index_path)
    }

    #[test]
    fn parallel_multistream_matches_sequential() {
        // 24 streams × 3 pages forces real thread interleaving; the parallel
        // reader must yield the exact page sequence the sequential path does.
        let (dump, index) = make_multistream("eq", 24, 3);
        let sequential: Vec<Page> = open(&dump).unwrap().collect::<anyhow::Result<_>>().unwrap();
        let parallel: Vec<Page> = open_multistream(&dump, &index)
            .unwrap()
            .collect::<anyhow::Result<_>>()
            .unwrap();
        assert_eq!(sequential.len(), 72);
        assert_eq!(parallel.len(), sequential.len());
        for (s, p) in sequential.iter().zip(&parallel) {
            assert_eq!(s.title, p.title);
            assert_eq!(s.text, p.text);
            assert_eq!(s.namespace, p.namespace);
        }
        // entity resolution survived the parallel path
        assert!(parallel[0].text.contains("body 1 & "));
    }

    #[test]
    fn corrupt_multistream_chunk_is_an_error() {
        // Damage one middle stream: the parallel reader must surface a hard
        // error (never silently skip 100 pages).
        let (dump, index) = make_multistream("corrupt", 8, 2);
        let mut bytes = std::fs::read(&dump).unwrap();
        let mid = bytes.len() / 2;
        for b in &mut bytes[mid..mid + 24] {
            *b ^= 0xFF;
        }
        let corrupted = dump.with_file_name("corrupt-damaged.xml.bz2");
        std::fs::write(&corrupted, &bytes).unwrap();
        let res: anyhow::Result<Vec<Page>> =
            open_multistream(&corrupted, &index).unwrap().collect();
        assert!(res.is_err(), "corrupt stream must error, not skip pages");
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
