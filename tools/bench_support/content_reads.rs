//! Read-only comparison on the existing first 10k records. Oracle work is untimed.
use crate::input;
use fdb::{Key, StoredDocument};
use serde_json::json;
use std::time::Instant;

pub const QUERIES: [&str; 4] = ["the", "quixote", "he", "the man"];
pub fn fold(text: &str) -> String {
    text.chars()
        .flat_map(|c| {
            match c {
                'I' | 'İ' | 'ı' => 'i',
                'Ç' | 'ç' => 'c',
                'Ğ' | 'ğ' => 'g',
                'Ö' | 'ö' => 'o',
                'Ş' | 'ş' => 's',
                'Ü' | 'ü' => 'u',
                _ => c,
            }
            .to_lowercase()
        })
        .collect()
}

pub struct Oracle {
    pub totals: [usize; 4],
    pub pages: [Vec<StoredDocument>; 4],
    pub gets: Vec<StoredDocument>,
}
impl Oracle {
    pub fn load() -> input::Result<Self> {
        input::phase("oracle_loading_outside_timing");
        let mut oracle = Self {
            totals: [0; 4],
            pages: std::array::from_fn(|_| Vec::new()),
            gets: Vec::new(),
        };
        let mut source = input::Input::open()?;
        source.skip(0)?;
        for id in 0..10000 {
            let stored = source.read()?;
            assert_eq!(
                stored.document.key,
                Key::new("shared", "texts", &format!("{id:010}"))
            );
            let body = fold(&format!(
                "{}\n{}",
                stored.document.title, stored.document.text
            ));
            for (n, text) in QUERIES.iter().enumerate() {
                if text.split_whitespace().all(|term| body.contains(term)) {
                    oracle.totals[n] += 1;
                    if oracle.pages[n].len() < 50 {
                        oracle.pages[n].push(stored.clone());
                    }
                }
            }
            // Spread 100 point reads over all 10k keys, including both endpoints.
            if (0..100).any(|sample| sample * 9999 / 99 == id) {
                oracle.gets.push(stored);
            }
        }
        println!(
            "{}",
            json!({"phase":"oracle","records":10000,"text_bytes":source.text_bytes,"queries":QUERIES,"totals":oracle.totals,"input_crc_verified":true,"outside_timing":true})
        );
        Ok(oracle)
    }
}

pub fn summary(samples: &[f64]) -> serde_json::Value {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let rank = |percent: usize| sorted[(sorted.len() * percent).div_ceil(100) - 1];
    json!({"samples":samples,"n":samples.len(),"p50_ms":rank(50),"p95_ms":rank(95),"max_ms":sorted[sorted.len()-1]})
}

pub fn measure<G, S>(oracle: &Oracle, mut get: G, mut search: S) -> input::Result<()>
where
    G: FnMut(&Key) -> input::Result<Option<StoredDocument>>,
    S: FnMut(&str) -> input::Result<(usize, Vec<StoredDocument>)>,
{
    // One complete untimed warmup per query and per point-read key.
    for expected in &oracle.gets {
        assert_eq!(get(&expected.document.key)?, Some(expected.clone()));
    }
    for (n, text) in QUERIES.iter().enumerate() {
        let (total, hits) = search(text)?;
        assert_eq!(total, oracle.totals[n]);
        assert_eq!(hits, oracle.pages[n]);
    }
    let mut query_samples: [Vec<f64>; 4] = std::array::from_fn(|_| Vec::new());
    let mut get_samples = Vec::new();
    for round in 0..30 {
        // Rotate query order to avoid always giving one category the first slot.
        for step in 0..4 {
            let n = (round + step) % 4;
            let start = Instant::now();
            let (total, hits) = search(QUERIES[n])?;
            query_samples[n].push(start.elapsed().as_secs_f64() * 1000.);
            assert_eq!(total, oracle.totals[n], "{}", QUERIES[n]);
            assert_eq!(hits, oracle.pages[n], "{}", QUERIES[n]);
        }
        for step in 0..100 {
            let expected = &oracle.gets[(round * 37 + step) % 100];
            let start = Instant::now();
            let actual = get(&expected.document.key)?;
            get_samples.push(start.elapsed().as_secs_f64() * 1000.);
            assert_eq!(actual.as_ref(), Some(expected));
        }
    }
    println!(
        "{}",
        json!({"phase":"reads","cache":"warm OS cache, one untimed full query/key warmup; no eviction","get":summary(&get_samples),"queries":QUERIES.iter().enumerate().map(|(n,text)| json!({"text":text,"total":oracle.totals[n],"offset":0,"limit":50,"timing":summary(&query_samples[n])})).collect::<Vec<_>>(),"full_pages_and_versions_verified":true,"assertions_outside_timing":true,"memory_not_measured":true})
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn turkish_oracle_vectors() {
        assert_eq!(super::fold("IŞIK ışık İŞİK"), "isik isik isik");
        assert_eq!(
            super::fold("ŞARTNAME şartname Sartname SARTNAME"),
            "sartname sartname sartname sartname"
        );
    }
    #[test]
    fn nearest_rank_percentiles() {
        let samples: Vec<_> = (1..=30).rev().map(f64::from).collect();
        let result = super::summary(&samples);
        assert_eq!(result["p50_ms"], 15.0);
        assert_eq!(result["p95_ms"], 29.0);
    }
}
