"""Preserve failed FDB ingest; finish explicitly allowed independent SQLite evidence."""
import importlib
import importlib.util
import json
import os
import subprocess
import sys
from pathlib import Path

R = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('scale', R/'tools/measure-trwiki-scale.py')
s = importlib.util.module_from_spec(spec)
spec.loader.exec_module(s)
L, P = s.L, s.P


def main():
    path = L/f'{P}-design.json'
    d = json.loads(path.read_text(encoding='utf-8'))
    assert d['gate_failed_at'] == 50000 and len(d['levels']) == 1
    failure = L/f'{P}-n100000-pair0-seed-fdb-scale-seed.process.json'
    assert json.loads(failure.read_text(encoding='utf-8'))['exit_code'] == 1
    env = os.environ.copy()
    env.update(PYTHONUTF8='1', FDB_SCALE_DOCUMENTS='100000',
        FDB_BENCH_CORPUS_PATH=str(s.CORPUS), FDB_BENCH_CORPUS_INDEX=str(s.CATALOG),
        FDB_BENCH_ORIGINAL_BODIES='1', FDB_STREAMED_WAL_LAYER='1', FDB_RECLAMATION_CONTROL='1',
        FDB_QUERY_CASES=','.join(str(i) for i in range(41)), FDB_SCALE_QUERY_REPEATS='10',
        FDB_QUERY_VARIANTS='legacy,cap')
    before = s.sources()
    cli = R/'output/maintenance-candidate-cli-20261024.exe'
    got = subprocess.run([str(cli),'data/benchmark-work/fdb','--stdio'], input='{"op":"stats"}\n',
        cwd=R, text=True, encoding='utf-8', capture_output=True,
        creationflags=subprocess.CREATE_NO_WINDOW)
    assert got.returncode == 0
    stats = json.loads(got.stdout)['result']
    assert stats['documents'] == 66000 and stats['sequence'] == 132
    assert before == s.sources()
    partial = L/f'{P}-partial66000-fdb-state.json'
    assert not partial.exists(), 'Resume must be explicit; do not overwrite evidence'
    # Exact committed ledger; the 500 rejected keys are expected absent, not
    # deletions. The reused oracle's generic `dead` field expresses absence.
    s.b.write(partial, dict(rows={str(i):dict(body=i,revision=0,version=i//500+1) for i in range(66000)},
        dead={str(i):dict(body=i,revision=0,version=0) for i in range(66000,66500)},
        next=66500,sequence=132))
    catalog = json.loads(s.CATALOG.read_text(encoding='utf-8'))
    f = dict(kind='streamed_entry_limit', error='streamed transaction exceeds entry/frame allowance',
        attempted_documents=100000, committed_documents=66000, sequence=132,
        rejected_batch=[66000,66499], offending_document=66498, text_bytes=444345,
        per_entry_limit_bytes=262144, transaction_limit_bytes=67108864,
        rejected_batch_json_bytes=sum(x['size'] for x in catalog['entries'][66000:66500]),
        raw_process=failure.name, stats=stats,
        partial_state=str(partial), partial_state_sha256=s.b.sha(partial),
        fdb_100k_ingest_succeeded=False, fdb_retry=False, truncate_or_owned_fallback=False)
    s.b.write(L/f'{P}-n100000-ingest-failure.json', f)
    d['ingest_failure'] = f
    pointer = json.loads(s.CURRENT.read_text(encoding='utf-8'))
    pointer['slots']['fdb'] = dict(documents=66000, attempted_documents=100000,
        status='failed_ingest_committed_prefix', state=str(partial),model_sha256=s.b.sha(partial))
    s.b.write(s.CURRENT,pointer)
    s.b.write(path,d)
    level=[]
    for pair in range(2):
        state=L/f'{P}-n100000-pair{pair}-sqlite-state.json'
        assert not state.exists()
        pointer['slots']['sqlite']=dict(documents=100000,status='resetting_owned_slot',state=str(state))
        s.b.write(s.CURRENT,pointer)
        s.b.reset('sqlite')
        for mode,label in [('scale-seed',f'n100000-pair{pair}-seed'),
                           ('scale-idle',f'n100000-pair{pair}-idle0'),
                           ('scale-idle',f'n100000-pair{pair}-idle1'),
                           ('scale-query-experiments',f'n100000-pair{pair}-matrix')]:
            r=s.run('sqlite',mode,state,label,env);level.append(r);d['completed'].append(r)
            if mode=='scale-seed':
                pointer['slots']['sqlite'].update(status='ready',model_sha256=s.b.sha(state))
                s.b.write(s.CURRENT,pointer)
            s.b.write(path,d)
    gate=dict(documents=100000,strict_open_samples_ms=[],idle_private_samples=[],
        ingest_peak_private_samples=[],per_case=[],relative_hybrid=[],failures=[f],passed=False,
        gate_scope='FDB ingest FAILED, no complete 100k DB; SQLite independent repeats only, no completed balanced 100k A/B')
    s.b.write(L/f'{P}-n100000-gates.json',gate)
    d['levels'].append(dict(documents=100000,full_fdb_measurement=False,gate=gate,completed=level))
    d['models']['100000']=dict(count=100000,states={'sqlite':str(state)},
        model_sha256=s.b.sha(state),fdb_partial_state=str(partial))
    sys.path.insert(0,str(R/'tools'))
    old=importlib.import_module('kill-block2-scale');old.P=P+'-compat-partial66000'
    os.environ.update({k:v for k,v in env.items() if k.startswith('FDB_')})
    d['old_reader']=old.old_reader(partial)
    assert before == s.sources()
    d.update(protected_after=s.b.protected(),provided_source_after=s.b.sha(s.CORPUS),
        slot_pointer=str(s.CURRENT),independent_completion_after_ingest_failure=True)
    assert d['protected_before']==d['protected_after'] and d['provided_source_sha256']==d['provided_source_after']
    d.update(protected_unchanged=True,provided_source_unchanged=True)
    s.b.write(path,d)
    print('Independent SQLite evidence and 66k committed-prefix old-reader oracle complete',flush=True)


if __name__=='__main__':main()
