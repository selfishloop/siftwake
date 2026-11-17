//! Read-only SDB TSV importer. Writes only the separately supplied Siftwake directory.
use serde_json::json;
use siftwake::{Database, Guard, Mutation, adapters};
use std::io::{BufRead, BufReader};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !(4..=5).contains(&args.len()) {
        return Err(
            "usage: siftwake-import-sdb <db-dir> <tsv-file> <export-namespace> <scope> [max-records]"
                .into(),
        );
    }
    let max = if args.len() == 5 {
        args[4].parse::<usize>()?
    } else {
        usize::MAX
    };
    let mut lines = BufReader::new(std::fs::File::open(&args[1])?).lines();
    let header = lines.next().ok_or("empty TSV")??;
    if header.trim_start_matches('\u{feff}') != "id\tparent_id\tflags\tname" {
        return Err("invalid SDB TSV header".into());
    }
    let mut db = Database::open(&args[0])?;
    let mut batch = vec![];
    let mut count = 0;
    let mut seen = std::collections::BTreeSet::new();
    for (number, line) in lines.enumerate() {
        if count >= max {
            break;
        }
        let line = line?;
        if line.is_empty() {
            continue;
        }
        let columns: Vec<&str> = line.splitn(4, '\t').collect();
        if columns.len() != 4 {
            return Err(format!(
                "invalid row {}; earlier batches remain committed",
                number + 2
            )
            .into());
        }
        let id: u64 = columns[0].parse()?;
        let parent: u64 = columns[1].parse()?;
        let flags: u16 = columns[2].parse()?;
        if id == 0 || id == 5 {
            continue;
        }
        let mut document =
            adapters::file_entry(&args[3], &args[2], id, parent, columns[3], "", flags)?;
        // TSV does not carry the full NTFS reference or resolved path. Do not
        // misrepresent packed export IDs as stable live identities.
        document
            .fields
            .insert("identity_kind".into(), json!("sdb_tsv_export_id"));
        if !seen.insert(document.key.clone()) {
            continue;
        }
        batch.push(Mutation::Put {
            document,
            guard: Guard::Any,
        });
        count += 1;
        if batch.len() >= 500 {
            db.commit(std::mem::take(&mut batch))?;
        }
    }
    if !batch.is_empty() {
        db.commit(batch)?;
    }
    let stats = db.stats()?;
    println!(
        "{}",
        json!({"imported":count,"database_documents":stats.documents,"sequence":stats.sequence})
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
