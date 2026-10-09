use fdb::{Database, Document, Guard, Key, Mutation, Query, Range, adapters};
use serde_json::json;
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "data/example".into());
    let mut db = Database::open(&dir)?;
    let fund = adapters::fundops_record(
        "shared",
        "funds",
        "ABC",
        BTreeMap::from([
            ("code".into(), json!("ABC")),
            ("name".into(), json!("Örnek Teknoloji Fonu")),
        ]),
        "",
    )?;
    let report = adapters::fundops_record(
        "shared",
        "kap_reports",
        "sample-report-1",
        BTreeMap::from([
            ("fund_code".into(), json!("ABC")),
            ("as_of".into(), json!(20261007)),
            ("source_url".into(), json!("https://example.invalid/rapor")),
        ]),
        "Örnek içerik: şirketin teknoloji yatırımları ve portföy dağılımı.",
    )?;
    let file = adapters::file_entry(
        "shared",
        "sample-volume",
        42,
        5,
        "ABC raporu.pdf",
        r"D:\Raporlar\ABC raporu.pdf",
        0,
    )?;
    let content = Document {
        key: Key::new("shared", "file_contents", "sample-volume:42"),
        title: file.title.clone(),
        text: "Örnek çıkarılmış metin: teknoloji yatırımları".into(),
        fields: BTreeMap::from([("extractor".into(), json!("sample-v1"))]),
        links: vec![file.key.clone(), report.key.clone(), fund.key.clone()],
    };
    db.commit(
        vec![fund, report, file, content]
            .into_iter()
            .map(|document| Mutation::Put {
                document,
                guard: Guard::Any,
            })
            .collect(),
    )?;
    let mut query = Query::new("shared");
    query.text = "TEKNOLOJİ yatırımları".into();
    query.linked_to = Some(Key::new("shared", "funds", "ABC"));
    println!("{}", serde_json::to_string_pretty(&db.search(&query)?)?);
    query.collection = Some("kap_reports".into());
    query.ranges = vec![Range {
        field: "as_of".into(),
        min: 20261001,
        max: 20261031,
    }];
    assert_eq!(db.search(&query)?.total, 1);
    db.compact()?;
    drop(db);
    let reopened = Database::open(dir)?;
    assert_eq!(reopened.search(&query)?.total, 1);
    Ok(())
}
