use serde::Deserialize;
use serde_json::{Value, json};
use siftwake::{Database, Key, Mutation, Query};
use std::io::{self, BufRead, Read, Write};

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Commit { mutations: Vec<Mutation> },
    Get { key: Key },
    Search { query: Query },
    Compact,
    Flush,
    PollCompaction,
    WaitCompaction,
    Stats,
}

fn execute(db: &mut Database, request: Request) -> siftwake::Result<Value> {
    Ok(match request {
        Request::Commit { mutations } => json!({"sequence": db.commit(mutations)?}),
        Request::Get { key } => serde_json::to_value(db.get(&key)?)?,
        Request::Search { query } => serde_json::to_value(db.search(&query)?)?,
        Request::Compact => serde_json::to_value(db.compact()?)?,
        Request::Flush => serde_json::to_value(db.flush()?)?,
        Request::PollCompaction => json!({"published": db.poll_compaction()?}),
        Request::WaitCompaction => json!({"published": db.wait_for_compaction()?}),
        Request::Stats => serde_json::to_value(db.stats()?)?,
    })
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let dir = args
        .next()
        .ok_or("usage: siftwake <database-directory> [request.json | --stdio]")?;
    let mode = args.next().unwrap_or_else(|| "--stdio".into());
    if args.next().is_some() {
        return Err("too many arguments".into());
    }
    let mut db = Database::open(dir)?;
    if mode != "--stdio" {
        let request = serde_json::from_reader(std::fs::File::open(mode)?)?;
        println!(
            "{}",
            json!({"ok": true, "result": execute(&mut db, request)?})
        );
        return Ok(());
    }
    // JSON Lines is usable from Rust, Python and Node without a network service.
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut output = io::stdout().lock();
    loop {
        let mut line = String::new();
        let count = (&mut input)
            .take(96 * 1024 * 1024 + 1)
            .read_line(&mut line)?;
        if count == 0 {
            break;
        }
        if count > 96 * 1024 * 1024 {
            return Err("request line exceeds 96 MiB".into());
        }
        let response = match serde_json::from_str::<Request>(&line)
            .map_err(siftwake::Error::from)
            .and_then(|r| execute(&mut db, r))
        {
            Ok(result) => json!({"ok": true, "result": result}),
            Err(error) => json!({"ok": false, "error": error.to_string()}),
        };
        writeln!(output, "{response}")?;
        output.flush()?;
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
