"""Supplementary wait counters after the final-release diagnostic race fix.

No throughput acceptance A/B: the complete v2 balanced experiment remains.
"""
import importlib
import os

b = importlib.import_module('measure-churn')
P = b.PREFIX = 'snapshot-waits-10k-20261026'
b.PROGRAMS['fdb'] = b.ROOT/'output/snapshot-fdb-20261026-v3.exe'


def main():
    for name in ['FDB_BENCH_SNAPSHOTS', 'FDB_COST_LEGACY', 'FDB_RECLAMATION_CONTROL', 'FDB_QUERY_VISIBILITY_PRUNING']:
        os.environ.pop(name, None)
    os.environ['FDB_SNAPSHOT_ROUNDS'] = '5'
    manifest = b.LOGS/f'{P}-design.json'
    assert not manifest.exists()
    design = dict(protected_before=b.protected(), program=str(b.PROGRAMS['fdb'].relative_to(b.ROOT)),
                  sha256=b.sha(b.PROGRAMS['fdb']), completed=[],
                  scope='same first10k, streamed4 readers, counter fix only; NOT a new balanced throughput gate')
    b.write(manifest, design)
    for batch in ['one', '100']:
        b.reset('fdb')
        state = b.LOGS/f'{P}-{batch}-state.json'
        b.write(b.LOGS/'snapshot-10k-20261026-slot-fdb.json',dict(state=str(state),label=f'waits-{batch}'))
        for mode in ['seed', f'snapshot-{batch}', 'verify']:
            design['completed'].append(b.run('fdb',mode,'streamed',state,batch))
            b.write(manifest, design)
    design['protected_after'] = b.protected()
    assert design['protected_before'] == design['protected_after']
    design['complete'] = True
    b.write(manifest, design)


if __name__ == '__main__':
    main()
