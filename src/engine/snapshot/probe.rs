//! Untimed setup and exactly one explicit full-vs-incremental mask sample.
use super::*;
impl Database {
    pub fn diagnostic_mask_pair(&mut self, keys: &[Key]) -> Result<crate::timing::mask::Sample> {
        if self.count != 100000 || keys.len() != 100 || self.snapshot_context.is_some() {
            return Err(Error::Invalid(
                "mask-only probe requires prepared100k and100 existing keys".into(),
            ));
        }
        self.flush()?;
        self.wait_for_compaction()?;
        while self.start_compaction()? {
            self.wait_for_compaction()?;
        }
        // Copies existing immutable rows verbatim; no new corpus or logical revision.
        let stored = keys
            .iter()
            .map(|key| {
                self.get(key)?
                    .ok_or_else(|| Error::Invalid("probe key missing".into()))
            })
            .collect::<Result<Vec<_>>>()?;
        for _ in 0..2 {
            let id = self.allocate()?;
            let run = write_run_prepared(
                &self.dir,
                id,
                self.sequence,
                stored
                    .iter()
                    .cloned()
                    .map(|stored| Ok(Change::Put { stored })),
                |_| Ok(()),
            )?;
            self.runs.push(run);
        }
        let id = self.allocate()?;
        self.publish(
            id,
            &Manifest {
                version: 3,
                runs: self.descriptors(),
                active: self.active.clone(),
            },
        )?;
        let reader = self.snapshot_reader()?;
        let pinned = reader.acquire()?;
        crate::timing::mask::begin();
        if !self.start_compaction()? {
            return Err(Error::Invalid("probe has no merge window".into()));
        }
        self.wait_for_compaction()?;
        let sample = crate::timing::mask::take()
            .ok_or_else(|| Error::Invalid("probe missed incremental replacement".into()))?;
        assert_eq!(pinned.sequence(), self.sequence);
        drop(pinned);
        drop(reader);
        Ok(sample)
    }
}
