"""Reuse witnessed process-kill windows with a held snapshot, no new corpus."""
import importlib
import json
import os
from pathlib import Path

cost = importlib.import_module('kill-revision-cost')
b, k = cost.b, cost.k
P = 'snapshot-kill-10k-20261026'
PROGRAM = b.ROOT / 'output/snapshot-fdb-20261026-v2.exe'


def main():
    cost.P = b.PREFIX = k.PREFIX = P
    b.PROGRAMS['fdb'] = PROGRAM
    os.environ['FDB_BENCH_SNAPSHOTS'] = '1'
    os.environ.pop('FDB_QUERY_VISIBILITY_PRUNING', None)
    os.environ.pop('FDB_COST_LEGACY', None)
    state = Path(json.loads((b.LOGS / 'snapshot-10k-20261026-slot-fdb.json').read_text(encoding='utf-8'))['state'])
    protected = b.protected()
    results = []
    for mode in ['kill-retire-before', 'kill-retire-after', 'kill-cancel']:
        results.append(cost.barrier(mode, state))
        print(mode, 'oracle passed', flush=True)
    for stage in ['iterator', 'flush', 'merge', 'cache']:
        r = k.kill_stage(stage, state)
        assert r['coverage_observed']
        results.append(r)
        print(stage, 'oracle passed', flush=True)
    current = b.PROGRAMS['fdb']
    b.PROGRAMS['fdb'] = b.ROOT / 'output/churn-fdb-20261024.exe'
    compat = b.run('fdb', 'verify', 'streamed', state, 'old-reader')
    b.PROGRAMS['fdb'] = current
    assert b.protected() == protected
    b.write(b.LOGS / f'{P}-summary.json', dict(results=results, old_reader=compat,
            protected_unchanged=True, power_loss_claim=False, program_sha256=b.sha(current),
            snapshot_scope='channel and initial snapshot held before mutations; cache-repair kill occurs inside open before channel exists'))


if __name__ == '__main__':
    main()
