//! Repair/verification entry point, independent of the unfinished benchmark.
#[allow(dead_code)]
#[path = "../tools/bench_support/churn.rs"]
mod churn;
#[path = "../tools/bench_support/block3a_repair.rs"]
mod repair;
use churn::Result;
use fdb::{
    Database, FirstPage, Guard, Key, MatchTotal, Mutation, Query, SearchPage, StoredDocument,
    ValidationStatus,
};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};
struct Engine(Database);
impl repair::Store for Engine {
    fn sequence(&mut self) -> Result<u64> {
        Ok(self.0.stats()?.sequence)
    }
    fn count(&mut self) -> Result<usize> {
        Ok(self.0.stats()?.documents)
    }
    fn get(&mut self, key: &Key) -> Result<Option<StoredDocument>> {
        Ok(self.0.get(key)?)
    }
    fn first(&mut self, q: &Query) -> Result<FirstPage> {
        Ok(self.0.search_first_hybrid(q, 200)?)
    }
    fn delete_expected(&mut self, rows: &[StoredDocument]) -> Result<u64> {
        assert_eq!(rows.len(), 16);
        Ok(self
            .0
            .commit_streamed(rows.iter().enumerate().map(|(slot, row)| {
                assert_eq!(row.document.key, repair::key(slot));
                Ok(Mutation::Delete {
                    key: row.document.key.clone(),
                    guard: Guard::Version {
                        version: row.version,
                    },
                })
            }))?)
    }
}
struct Legacy {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}
impl Legacy {
    fn request(&mut self, request: Value) -> Result<Value> {
        serde_json::to_writer(&mut self.input, &request)?;
        self.input.write_all(b"\n")?;
        self.input.flush()?;
        let mut line = String::new();
        assert!(self.output.read_line(&mut line)? > 0, "old reader closed");
        let response: Value = serde_json::from_str(&line)?;
        assert_eq!(response["ok"], true, "old reader {response}");
        Ok(response["result"].clone())
    }
    fn finish(self) -> Result<()> {
        let Self {
            mut child,
            input,
            output,
        } = self;
        drop(input);
        drop(output);
        assert!(child.wait()?.success());
        Ok(())
    }
}
impl repair::Store for Legacy {
    fn sequence(&mut self) -> Result<u64> {
        Ok(self.request(json!({"op":"stats"}))?["sequence"]
            .as_u64()
            .ok_or("sequence")?)
    }
    fn count(&mut self) -> Result<usize> {
        Ok(self.request(json!({"op":"stats"}))?["documents"]
            .as_u64()
            .ok_or("documents")? as usize)
    }
    fn get(&mut self, key: &Key) -> Result<Option<StoredDocument>> {
        Ok(serde_json::from_value(
            self.request(json!({"op":"get","key":key}))?,
        )?)
    }
    fn first(&mut self, q: &Query) -> Result<FirstPage> {
        let p: SearchPage =
            serde_json::from_value(self.request(json!({"op":"search","query":q}))?)?;
        Ok(FirstPage {
            sequence: p.sequence,
            hits: p.hits,
            total: MatchTotal::Exact(p.total),
            validation: ValidationStatus::Verified,
        })
    }
    fn delete_expected(&mut self, _: &[StoredDocument]) -> Result<u64> {
        Err("legacy verification is read-only".into())
    }
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 3);
    if args[0] == "legacy" {
        assert_eq!(args[1], "verify");
        let mut child = Command::new("output/maintenance-candidate-cli-20261024.exe")
            .args(["data/benchmark-work/fdb", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        let mut db = Legacy {
            child,
            input,
            output,
        };
        repair::run(&mut db, &args[1], &args[2])?;
        db.finish()
    } else {
        assert_eq!(args[0], "fdb");
        let mut db = Engine(Database::open("data/benchmark-work/fdb")?);
        db.0.set_streamed_wal_layer(true)?;
        repair::run(&mut db, &args[1], &args[2])
    }
}
