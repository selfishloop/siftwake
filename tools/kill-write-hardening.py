"""Final recovery audit only after Block1 performance fail-fast, no new corpus."""
import importlib
import json
import os
from pathlib import Path

b = importlib.import_module('measure-churn')
P = 'write-hardening-kill-v6-10k-20261027'
PROGRAM = b.ROOT/'output/write-hardening-final-20261027-v6.exe'
POINTER = b.LOGS/'write-hardening-10k-20261027-slot-fdb.json'
THIRTY_SCOPE = 'not measured; Block1 throughput gate failed'


def main():
    state = Path(json.loads(POINTER.read_text(encoding='utf-8'))['state'])
    slot = dict(state=str(state))
    b.write(b.LOGS/'snapshot-10k-20261026-slot-fdb.json',slot)
    os.environ['FDB_STREAMED_WAL_LAYER']='1'
    # Explicitly scheduled retirement/rewrite recovery probes must not have
    # their fixture consumed by automatic reclamation while preparing it.
    os.environ['FDB_RECLAMATION_CONTROL']='1'
    k=importlib.import_module('kill-snapshots')
    k.P=P
    k.PROGRAM=PROGRAM
    k.main()
    summary=json.loads((b.LOGS/f'{P}-summary.json').read_text(encoding='utf-8'))
    scratch=[]
    for p in (b.ROOT/'data/benchmark-work/fdb').iterdir():
        if p.is_file() and (p.name.startswith('input-') or '.build-' in p.name):
            scratch.append(dict(name=p.name,bytes=p.stat().st_size))
    summary['post_reopen_scratch']=scratch
    summary['post_reopen_scratch_bytes']=sum(p['bytes'] for p in scratch)
    summary['retirement_fixture']='two immutable100-entry transactions use existing segment path for setup; WAL opt-in restored before target kill'
    summary['rewrite_iterator_limit']='historical cancel_iterator label witnesses interrupted input while revision exists; does not prove fast WAL path cancelled it'
    summary['thirty_round_gate']=THIRTY_SCOPE
    witness=importlib.import_module('measure-churn-kill')
    before_root=witness.published_sources()
    os.environ.pop('FDB_BENCH_SNAPSHOTS',None)
    b.PROGRAMS['fdb']=k.PROGRAM
    result=b.run('fdb','write-idle-check','streamed',state,'one-idle-call')
    phases=[json.loads(s) for s in (b.LOGS/result['log']).read_text(encoding='utf-8').splitlines()]
    idle=next(r for r in phases if r['phase']=='write_idle_checked')
    pinned_names={p['name'] for p in idle['while_pinned']['files']}
    assert set(before_root['source_names'])<=pinned_names,'A pinned old source path was removed'
    after_root=witness.published_sources()
    live=set(after_root['source_names'])
    orphans=[p for p in idle['after_release']['files'] if p['name'].endswith('.fdb') and p['name'] not in live]
    summary['one_idle_call']=dict(result=result,before_publication=before_root,after_publication=after_root,all_old_source_paths_present_while_pinned=True,unreferenced_fdb_files_after_release=orphans,orphan_bytes=sum(p['bytes'] for p in orphans),oracle=True)
    assert summary['one_idle_call']['orphan_bytes']<=1024*1024
    summary['one_idle_call']['post_matrix']=b.run('fdb','verify','streamed',state,'after-one-idle-call')
    b.PROGRAMS['fdb']=b.ROOT/'output/churn-fdb-20261024.exe'
    summary['one_idle_call']['old_reader_post_matrix']=b.run('fdb','verify','streamed',state,'after-one-idle-old-reader')
    b.write(b.LOGS/f'{P}-summary.json',summary)


if __name__=='__main__':main()
