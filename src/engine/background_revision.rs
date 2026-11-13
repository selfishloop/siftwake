//! Prepare one revision rewrite off the writer. Only the writer publishes it.
use super::*;

#[derive(Default)]
pub(super) struct State {
    job: Option<Pending>,
    examined: Option<Vec<Source>>,
    collect: bool,
}

struct Pending {
    base: Vec<Source>,
    id: u64,
    control: Arc<maintenance::Control>,
    handle: JoinHandle<Result<Option<Prepared>>>,
}

struct Prepared {
    range: std::ops::Range<usize>,
    output: JobOutput,
}

impl State {
    pub(super) fn pending(&self) -> bool {
        self.job.is_some()
    }

    pub(super) fn running(&self) -> bool {
        self.job
            .as_ref()
            .is_some_and(|job| !job.handle.is_finished())
    }

    pub(super) fn cancel(&self) {
        if let Some(job) = &self.job {
            job.control.cancel();
        }
    }

    pub(super) fn finish_on_close(&mut self) {
        if let Some(job) = self.job.take() {
            let _ = job.handle.join();
        }
    }

    pub(super) fn protect(&self, live: &mut BTreeSet<(Kind, u64)>) {
        if let Some(job) = &self.job {
            live.insert((Kind::Run, job.id));
            live.extend(job.base.iter().map(|source| (source.kind, source.id)));
        }
    }
}

fn prefix_matches(runs: &[Run], base: &[Source]) -> bool {
    runs.len() >= base.len()
        && runs
            .iter()
            .zip(base)
            .all(|(run, source)| &run.source == source)
}

impl Database {
    pub(super) fn schedule_background_revision(&mut self) -> Result<bool> {
        let state = &self.background_revision;
        if !self.revision_reclamation
            || !self.revision_responsive
            || self.job.is_some()
            || state.pending()
            || self.runs.is_empty()
            || state.examined.as_ref().is_some_and(|base| {
                base.len() == self.runs.len() && prefix_matches(&self.runs, base)
            })
        {
            return Ok(false);
        }

        let base = self.descriptors();
        let runs = self.runs.clone();
        let dir = self.dir.clone();
        let id = self.allocate()?;
        let control = Arc::new(maintenance::Control::new(self.maintenance_activity.clone()));
        let worker_control = control.clone();
        let handle = std::thread::Builder::new()
            .name("fdb-revision".into())
            .spawn(move || {
                let _scope = maintenance::Scope::enter(Some(worker_control.clone()));
                #[cfg(test)]
                test_jobs::wait_revision(&dir);
                let Some(plan) = revisions::plan(&runs, true)? else {
                    return Ok(None);
                };
                let range = plan.start..plan.end;
                let output = revisions::rewrite(
                    &dir,
                    id,
                    &runs[range.clone()],
                    plan,
                    true,
                    Some(worker_control),
                )?;
                Ok(Some(Prepared { range, output }))
            })?;
        self.background_revision.job = Some(Pending {
            base,
            id,
            control,
            handle,
        });
        Ok(true)
    }

    pub(super) fn finish_background_revision(&mut self, wait: bool) -> Result<bool> {
        let state = &mut self.background_revision;
        if self.job.is_some()
            || !state
                .job
                .as_ref()
                .is_some_and(|job| wait || job.handle.is_finished())
        {
            return Ok(false);
        }
        let job = state.job.take().unwrap();
        let prepared = job
            .handle
            .join()
            .map_err(|_| Error::Invalid("revision worker panicked".into()))?;
        if job.control.is_cancelled() || !prefix_matches(&self.runs, &job.base) {
            state.collect = true;
            return Ok(false);
        }
        let Some(prepared) = prepared? else {
            state.examined = Some(job.base);
            return Ok(false);
        };
        state.collect = true;
        self.install_output(&job.base[prepared.range], prepared.output, false)
    }

    pub(super) fn collect_background_revision(&mut self) -> Result<bool> {
        if !self.background_revision.collect {
            return Ok(false);
        }
        self.collect_obsolete()?;
        self.background_revision.collect = false;
        Ok(true)
    }
}

#[cfg(test)]
mod tests;
