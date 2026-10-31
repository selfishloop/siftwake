"""Finish the interrupted second100k pair, without reset, ingest, or log overwrite."""
import importlib.util
import json
import os
from pathlib import Path

R=Path(__file__).resolve().parent.parent
spec=importlib.util.spec_from_file_location('cap',R/'tools/measure-cap200k.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
s=m.s;L=m.L;P=m.P


def main():
    path=L/f'{P}-design.json';d=json.loads(path.read_text(encoding='utf-8'))
    assert not d.get('protected_unchanged') and d['gate_failed_at'] is None
    assert len(d['levels'])==1 and d['levels'][0]['gate']['passed']
    assert d['provided_source_sha256']==s.b.sha(s.CORPUS)
    assert d['protected_before']==s.b.protected()
    assert d['implementation_source_sha256']=={str(p.relative_to(R)):s.b.sha(p) for p in sorted((R/'src').rglob('*.rs'))}
    for e,p in s.PROGRAMS.items():assert d['programs'][e]['sha256']==s.b.sha(p)
    pointer=json.loads(s.CURRENT.read_text(encoding='utf-8'));states={}
    for e in ['fdb','sqlite']:
        slot=pointer['slots'][e];assert slot['status']=='ready' and slot['documents']==100000
        states[e]=Path(slot['state']);assert 'n100000-pair1-' in states[e].name
        assert s.b.sha(states[e])==slot['model_sha256']==d['models']['100000']['model_sha256']
    def save():s.b.write(path,d)
    def add(result):
        if not any(r['log']==result['log'] for r in d['completed']):d['completed'].append(result)
        save()
    for f in sorted(L.glob(f'{P}-n100000-pair1-idle*.process.json')):
        r=json.loads(f.read_text(encoding='utf-8'));assert r['exit_code']==0;add(r)
    assert len([r for r in d['completed'] if 'n100000-pair1-idle' in r['label']])==4
    partial=L/f'{P}-n100000-pair1-matrix-sqlite-scale-query-experiments.jsonl'
    assert partial.exists() and not partial.with_suffix('.process.json').exists()
    xs=[]
    for line in partial.read_text(encoding='utf-8').splitlines():
        try:x=json.loads(line)
        except json.JSONDecodeError:continue
        if x['phase']=='query_experiment':xs.append(x)
    d['interruption']=dict(reason='Driver and both worker processes no longer running; exact termination cause not measured',
        partial_log=partial.name,partial_sha256=s.b.sha(partial),partial_measured_queries=len(xs),
        partial_max_query_ms=max(x['elapsed_ms'] for x in xs) if xs else None,
        partial_in_primary_pooled_statistics=False,source_and_binary_hashes_rechecked=True,
        reset_or_ingest_repeated=False,resumed_order=['sqlite','fdb'])
    d['models']['100000']['states']={e:str(p) for e,p in states.items()};save()
    env=os.environ.copy();env.update(PYTHONUTF8='1',FDB_BENCH_CORPUS_PATH=str(s.CORPUS),
        FDB_BENCH_CORPUS_INDEX=str(s.CATALOG),FDB_BENCH_ORIGINAL_BODIES='1',FDB_STREAMED_WAL_LAYER='1',
        FDB_RECLAMATION_CONTROL='1',FDB_QUERY_CASES=','.join(map(str,range(41))),
        FDB_SCALE_QUERY_REPEATS='10',FDB_SCALE_DOCUMENTS='100000')
    before=s.sources()
    def run(e,mode,label,options):
        label='n100000-pair1-resume-'+label
        process=L/f'{P}-{label}-{e}-{mode}.process.json'
        if process.exists():
            r=json.loads(process.read_text(encoding='utf-8'));assert r['exit_code']==0
        else:r=s.run(e,mode,states[e],label,options)
        add(r);return r
    for e in ['sqlite','fdb']:
        qe=env.copy();qe['FDB_QUERY_VARIANTS']='legacy,exact-four,cap,cap-four,cap-hybrid' if e=='fdb' else 'legacy,cap'
        run(e,'scale-query-experiments','matrix',qe)
    ce=env.copy();ce.update(FDB_QUERY_CASES='0,1,2',FDB_QUERY_VARIANTS='cap-hybrid,cap-four,cap',FDB_CPU_BLOCK_REPEATS='200')
    run('fdb','scale-query-cpu-blocks','cpu',ce)
    ne=env.copy();ne.update(FDB_QUERY_CASES='39,40',FDB_QUERY_VARIANTS='cap-four,cap-hybrid')
    run('fdb','scale-query-experiments','negative',ne)
    for i,q in enumerate(d['first_queries']):
        fe=env.copy();fe.update(FDB_FIRST_QUERY=q,FDB_FIRST_VARIANT='cap-hybrid')
        for e in ['sqlite','fdb']:
            s.warm(e);run(e,'first-strict' if e=='fdb' else 'first-sqlite',f'first{i}',fe)
    assert before==s.sources()
    level=[r for r in d['completed'] if 'n100000' in r['label']]
    audit=json.loads(m.A.read_text(encoding='utf-8'));gate=m.gates(100000,level,audit)
    s.b.write(L/f'{P}-n100000-gates.json',gate)
    d['levels'].append(dict(documents=100000,full_fdb_measurement=True,gate=gate));save()
    d['recovery']['100000']=m.recovery(100000,states['fdb'],env);save()
    if not gate['passed']:
        d['gate_failed_at']=100000;print(json.dumps(dict(gate_failed_at=100000,failures=gate['failures'])),flush=True);save()
    else:
        env['FDB_CANCEL_LONG_IDS']='66498,45192'
        add(s.run('fdb','scale-long-cancel',states['fdb'],'n100000-long-cancel',env))
    d.update(protected_after=s.b.protected(),provided_source_after=s.b.sha(s.CORPUS))
    assert d['protected_before']==d['protected_after'] and d['provided_source_sha256']==d['provided_source_after']
    d.update(protected_unchanged=True,provided_source_unchanged=True);save()


if __name__=='__main__':main()
