"""Read-only counters, Pending kill/reopen and frozen old-reader compatibility."""
import importlib
import importlib.util
import json
import os
import subprocess
import sys
from pathlib import Path

R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='posting-pages-50k-20261029-v3'
spec=importlib.util.spec_from_file_location('measure',R/'tools/measure-posting-pages.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
s=m.s

def main():
    target=L/f'{P}-verification.json';assert not target.exists()
    d=json.loads((L/f'{P}-design.json').read_text(encoding='utf-8'))
    assert d.get('authoritative_sources_unchanged') and len(d['completed'])==20
    state=Path(d['models']['fdb'])
    slot=json.loads(s.CURRENT.read_text(encoding='utf-8'))['slots']['fdb']
    assert slot['documents']==50000 and s.b.sha(state)==slot['model_sha256']
    env=os.environ.copy();env.update(PYTHONUTF8='1',FDB_SCALE_DOCUMENTS='50000',
        FDB_BENCH_CORPUS_PATH=str(s.CORPUS),FDB_BENCH_CORPUS_INDEX=str(s.CATALOG),
        FDB_BENCH_ORIGINAL_BODIES='1',FDB_STREAMED_WAL_LAYER='1',FDB_RECLAMATION_CONTROL='1')
    before=s.sources()
    profiles=[]
    binaries={'baseline':R/'output/trwiki-scale-20261028-v4-diagnostic.exe',
              'candidate':R/'output/posting-pages-20261029-v3-diagnostic.exe'}
    pe=env.copy();pe.update(FDB_QUERY_CASES='15,22,23,24,28,30,32,33,34',
                           FDB_QUERY_VARIANTS='cap,cap-hybrid',FDB_SCALE_QUERY_REPEATS='1')
    for e,binary in binaries.items():
        old=m.programs[e];m.programs[e]=binary
        try:r=m.run(e,'scale-query-experiments',state,'counter-profile',pe)
        finally:m.programs[e]=old
        r['binary']=str(binary.relative_to(R));r['binary_sha256']=s.b.sha(binary);profiles.append(r)
    stderr=L/f'{P}-pending-kill.stderr';assert not stderr.exists()
    ke=env.copy();ke.update(FDB_FIRST_QUERY='olduğu',FDB_FIRST_VARIANT='cap-hybrid')
    with stderr.open('w',encoding='utf-8') as err:
        child=subprocess.Popen([str(m.programs['candidate']),'first-background','streamed',str(state)],
            cwd=R,env=ke,stdout=subprocess.PIPE,stderr=err,text=True,encoding='utf-8',
            creationflags=subprocess.CREATE_NO_WINDOW)
        try:
            witness=json.loads(child.stdout.readline())
            assert witness['phase']=='session_return' and witness['status']=='pending',witness
            child.kill();code=child.wait(timeout=10)
        finally:
            if child.poll() is None:child.kill();child.wait(timeout=10)
    assert code!=0
    reopened=m.run('candidate','first-strict',state,'reopen-after-kill',ke)
    lines=[json.loads(line) for line in (L/reopened['log']).read_text(encoding='utf-8').splitlines()]
    assert any(x['phase']=='first_result_oracle' and x['oracle'] for x in lines)
    sys.path.insert(0,str(R/'tools'));old=importlib.import_module('kill-block2-scale');old.P=P+'-compat'
    os.environ.update({k:v for k,v in env.items() if k.startswith('FDB_')})
    compatibility=old.old_reader(state)
    assert before==s.sources() and d['protected_after']==s.b.protected()
    assert s.b.sha(s.CORPUS)==d['provided_source_after']
    prior=json.loads((L/'trwiki-scale-20261028-v1-final-audit.json').read_text(encoding='utf-8'))
    format_paths=['src/format.rs','src/manifest.rs','src/payload.rs','src/engine/streamed.rs']
    for path in format_paths:assert s.b.sha(R/path)==prior['engine_source_sha256'][path],path
    s.b.write(target,dict(profiles=profiles,pending_kill=dict(witness=witness,exit_code=code),
        reopen=reopened,old_reader=compatibility,source_hashes_unchanged=True,
        old_fixture_and_corpus_unchanged=True,provided_corpus_unchanged=True,
        encoded_entry_limit_and_format_source_sha256={p:s.b.sha(R/p) for p in format_paths},
        power_loss_claim=False,corpus_100k_started=False))
    print('Counter/kill/reopen/full old-reader oracle passed',flush=True)

if __name__=='__main__':main()
