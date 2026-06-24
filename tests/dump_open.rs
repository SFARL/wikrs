use std::io::Write;

#[test]
fn opens_plain_and_bz2_dumps() {
    let xml = b"<mediawiki><page><title>A</title><ns>0</ns>\
        <revision><text>hello</text></revision></page></mediawiki>";

    let dir = std::env::temp_dir().join("wikrs_dump_open_test");
    std::fs::create_dir_all(&dir).unwrap();

    let plain = dir.join("d.xml");
    std::fs::write(&plain, xml).unwrap();

    let bz = dir.join("d.xml.bz2");
    let mut enc = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::default());
    enc.write_all(xml).unwrap();
    std::fs::write(&bz, enc.finish().unwrap()).unwrap();

    for path in [&plain, &bz] {
        let pages: Vec<_> = wikrs::dump::open(path)
            .unwrap()
            .collect::<anyhow::Result<_>>()
            .unwrap();
        assert_eq!(pages.len(), 1, "path {:?}", path);
        assert_eq!(pages[0].text, "hello");
    }
}
