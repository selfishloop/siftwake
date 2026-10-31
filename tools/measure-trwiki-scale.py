"""First50k/100k PROVIDED TR corpus, fixed slots, balanced FDB/SQLite evidence."""
import importlib
import importlib.util
import json
import math
import os
import subprocess
import sys
import time
from collections import defaultdict
from pathlib import Path

R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='trwiki-scale-20261028-v1'
spec=importlib.util.spec_from_file_location('bench',R/'tools/measure-churn.py')
b=importlib.util.module_from_spec(spec);spec.loader.exec_module(b)
PROGRAMS={e:R/f'output/trwiki-scale-20261028-v2-{e}.exe' for e in ('fdb','sqlite')}
CORPUS=Path('D:/frunner-corpus/trwiki-100k/corpus.payload-bench')
CATALOG=L/'trwiki-provided-20261028-v1-catalog.json'
CURRENT=L/'active-benchmark-slots.json'

def run(e,mode,state,label,env):
    log=L/f'{P}-{label}-{e}-{mode}.jsonl';assert not log.exists(),log
    at=time.monotonic()
    with log.open('w',encoding='utf-8') as out,log.with_suffix('.stderr').open('w',encoding='utf-8') as err:
        child=subprocess.run([str(PROGRAMS[e]),mode,'streamed',str(state)],cwd=R,env=env,
            stdout=out,stderr=err,creationflags=subprocess.CREATE_NO_WINDOW)
    result=dict(engine=e,mode=mode,label=label,log=log.name,exit_code=child.returncode,wall_s=time.monotonic()-at)
    b.write(log.with_suffix('.process.json'),result)
    print(json.dumps(result),flush=True)
    if child.returncode:raise RuntimeError(f'Worker failed; preserve evidence: {log}')
    return result

def rows(result):return [json.loads(s) for s in (L/result['log']).read_text(encoding='utf-8').splitlines() if s.strip()]
def sources():return {p.name:b.sha(p) for p in (R/'data/benchmark-work/fdb').iterdir() if p.suffix in ('.fdb','.ready')}
def warm(e):
    # Warm the SAME arm's entire existing DB by streaming it before first-call
    # timing; no file copies, no claimed forced-cold result.
    return {p.name:b.sha(p) for p in sorted((R/f'data/benchmark-work/{e}').iterdir()) if p.is_file()}
def percentile(a):return sorted(a)[math.ceil(.95*len(a))-1]

def gates(n,results):
    opens=[];idle=[];peak=[];matrix=defaultdict(list);blocks=defaultdict(list)
    for result in results:
        if result['engine']!='fdb':continue
        for x in rows(result):
            if x['phase']=='opened' and x['mode']=='scale-idle':opens.append(x['open_ms'])
            if x['phase']=='first_result':opens.append(x['session_ms'])
            if x['phase']=='scale_idle':idle.append(x['memory']['private_bytes'])
            if x['phase']=='scale_ingested':peak.append(x['memory']['os_lifetime_peak_private_commit_bytes'])
            if x['phase']=='query_experiment':matrix[x['case_id'],x['variant']].append(x)
            if x['phase']=='cpu_block':blocks[x['text'],x['variant']].append(x)
    audit=json.loads((L/'trwiki-provided-20261028-v1-audit.json').read_text(encoding='utf-8'))
    cases=json.loads(CATALOG.read_text(encoding='utf-8'))['cases'];failures=[];per_case=[]
    for (i,v),xs in sorted(matrix.items()):
        if not v.startswith('cap'):continue
        case=cases[i];group=audit['query_groups'][case['text']]
        limit=None if group=='fixed_short' else (30 if group.startswith(('very_common','common_')) else (150 if n==50000 else 300))
        p95=percentile([x['elapsed_ms'] for x in xs])
        ok=limit is None or p95<=limit
        per_case.append(dict(case_id=i,text=case['text'],group=group,variant=v,p95_ms=p95,limit_ms=limit,passed=ok,excluded_short=limit is None))
        if not ok:failures.append(dict(kind='capped_query',case_id=i,text=case['text'],variant=v,p95_ms=p95,limit_ms=limit))
    if opens and percentile(opens)>(1000 if n==50000 else 2000):failures.append(dict(kind='strict_open',p95_ms=percentile(opens),limit_ms=1000 if n==50000 else 2000))
    if idle and max(idle)>15*1024*1024:failures.append(dict(kind='idle_private',bytes=max(idle),limit=15*1024*1024))
    if peak and max(peak)>60*1024*1024:failures.append(dict(kind='ingest_peak_private',bytes=max(peak),limit=60*1024*1024))
    # The accepted hybrid relative gates are also reported on this corpus.
    relative=[]
    for i,c in enumerate(cases):
        if audit['prefixes'][str(n)]['exact'][c['text']]!=0:continue
        if (i,'cap-four') not in matrix:continue
        control=percentile([x['elapsed_ms'] for x in matrix[i,'cap-four']]);candidate=percentile([x['elapsed_ms'] for x in matrix[i,'cap-hybrid']])
        ok=candidate<=control*1.10
        item=dict(kind='hybrid_negative',text=c['text'],control_p95_ms=control,hybrid_p95_ms=candidate,limit_ms=control*1.10,passed=ok)
        relative.append(item)
        if not ok:failures.append(item)
    for text in set(q for q,_ in blocks):
        control=sum(x['cpu_ms'] for x in blocks[text,'cap']);candidate=sum(x['cpu_ms'] for x in blocks[text,'cap-hybrid'])
        item=dict(kind='hybrid_positive_cpu',text=text,serial_total_cpu_ms=control,hybrid_total_cpu_ms=candidate,limit_ms=control*1.05,passed=candidate<=control*1.05)
        relative.append(item)
        if not item['passed']:failures.append(item)
    return dict(documents=n,strict_open_samples_ms=opens,idle_private_samples=idle,ingest_peak_private_samples=peak,
        per_case=per_case,relative_hybrid=relative,failures=failures,passed=not failures,
        gate_scope='Capped N50; exact serial/four informational per explicit user reply; fixed_short excluded from search deadline')

def main():
    design=L/f'{P}-design.json';assert not design.exists(),'Inspect/resume explicitly; never replay/reset silently'
    audit=json.loads((L/'trwiki-provided-20261028-v1-audit.json').read_text(encoding='utf-8'))
    assert b.sha(CORPUS)==audit['payload_sha256'] and b.sha(CATALOG)==audit['catalog_sha256']
    assert audit['documents']==audit['unique_text_bodies']==100000 and audit['original_key_template_verified']
    cases=json.loads(CATALOG.read_text(encoding='utf-8'))['cases']
    env=os.environ.copy();env.update(PYTHONUTF8='1',FDB_BENCH_CORPUS_PATH=str(CORPUS),FDB_BENCH_CORPUS_INDEX=str(CATALOG),FDB_BENCH_ORIGINAL_BODIES='1',FDB_STREAMED_WAL_LAYER='1',FDB_RECLAMATION_CONTROL='1',FDB_QUERY_CASES=','.join(str(i) for i in range(len(cases))),FDB_SCALE_QUERY_REPEATS='10')
    d=dict(programs={e:dict(path=str(p.relative_to(R)),sha256=b.sha(p)) for e,p in PROGRAMS.items()},
        protected_before=b.protected(),provided_source_sha256=b.sha(CORPUS),catalog_sha256=b.sha(CATALOG),
        corpus='Provided firstN TR wiki articles; original title/text/fields/links; no trim/marker/new corpus/copies; private/ never read',
        fixed_slots=['data/benchmark-work/fdb','data/benchmark-work/sqlite'],ingest_order=['FDB/SQLite','SQLite/FDB'],
        query_order=['FDB/SQLite','SQLite/FDB'],samples_per_query_variant=20,query_cases=cases,
        variants=['legacy','exact-four','cap','cap-four','cap-hybrid'],
        cache='warm OS/engine matrix: full payload/version oracle+one warmup each variant; first-call arms stream warm all of their DB files before API timing; no forced cold',
        strict_open_gate={50000:1000,100000:2000},search_gate='N50 primary all capped modes; common30ms,mid/rare150ms@50k/300ms@100k; short ve/if/id/zx outside deadline',
        memory_gate='clean idle private<=15MiB; streamed ingest OS lifetime peak private<=60MiB incl descriptor/model harness',
        failure_policy='If gate fails, no following FDB search/cancel/write/snapshot/cold stage; finish independent nextN ingest/disk/RAM/SQLite and report all failures',
        first_queries=['olduğu','akarsu','furlong','ilçesine köyleri','zx','qzxkvwnever'],completed=[],levels=[],gate_failed_at=None,models={})
    b.write(design,d)
    current=dict(corpus='trwiki-provided',source=str(CORPUS),catalog=str(CATALOG),original_bodies=True,slots={})
    for n in (50000,100000):
        env['FDB_SCALE_DOCUMENTS']=str(n);full=d['gate_failed_at'] is None;level=[]
        for pair,order in enumerate([('fdb','sqlite'),('sqlite','fdb')]):
            states={e:L/f'{P}-n{n}-pair{pair}-{e}-state.json' for e in PROGRAMS}
            for e in order:
                assert not states[e].exists()
                current['slots'][e]=dict(documents=n,status='resetting_owned_slot',state=str(states[e]));b.write(CURRENT,current)
                b.reset(e)
                se=env.copy()
                if e=='fdb':se['FDB_SEED_IDLE_CLEANUP']='1'
                r=run(e,'scale-seed',states[e],f'n{n}-pair{pair}-seed',se);level.append(r);d['completed'].append(r)
                current['slots'][e].update(status='ready',model_sha256=b.sha(states[e]));b.write(CURRENT,current);b.write(design,d)
            assert b.sha(states['fdb'])==b.sha(states['sqlite']), 'identical input payload/version models required'
            d['models'][str(n)]=dict(count=n,states={e:str(s) for e,s in states.items()},model_sha256=b.sha(states['fdb']))
            before=sources()
            # Fresh-process RAM/open is independent even if a prior query gate failed.
            for i in range(2):
                for e in order:
                    r=run(e,'scale-idle',states[e],f'n{n}-pair{pair}-idle{i}',env);level.append(r);d['completed'].append(r)
            for e in order:
                if e=='fdb' and not full:continue
                qe=env.copy();qe['FDB_QUERY_VARIANTS']='legacy,exact-four,cap,cap-four,cap-hybrid' if e=='fdb' else 'legacy,cap'
                r=run(e,'scale-query-experiments',states[e],f'n{n}-pair{pair}-matrix',qe);level.append(r);d['completed'].append(r);b.write(design,d)
            if full:
                ce=env.copy();ce.update(FDB_QUERY_CASES='0,1,2',FDB_QUERY_VARIANTS='cap,cap-four,cap-hybrid' if pair==0 else 'cap-hybrid,cap-four,cap',FDB_CPU_BLOCK_REPEATS='200')
                r=run('fdb','scale-query-cpu-blocks',states['fdb'],f'n{n}-pair{pair}-cpu',ce);level.append(r);d['completed'].append(r)
                for qi,q in enumerate(d['first_queries']):
                    fe=env.copy();fe.update(FDB_FIRST_QUERY=q,FDB_FIRST_VARIANT='cap-hybrid')
                    for e in order:
                        warm(e)
                        r=run(e,'first-strict' if e=='fdb' else 'first-sqlite',states[e],f'n{n}-pair{pair}-first{qi}',fe);level.append(r);d['completed'].append(r)
            assert before==sources(),'Read-only measurements must preserve all authoritative sources'
            b.write(design,d)
        gate=gates(n,level);b.write(L/f'{P}-n{n}-gates.json',gate);d['levels'].append(dict(documents=n,full_fdb_measurement=full,gate=gate,completed=level))
        if full and not gate['passed']:
            d['gate_failed_at']=n
            print(json.dumps(dict(gate_failed_at=n,failures=gate['failures'],next='Only independent ingest/disk/RAM/SQLite; no later FDB query/cancel stages')),flush=True)
        b.write(design,d)
    if d['gate_failed_at'] is None:
        env['FDB_SCALE_DOCUMENTS']='100000';fs=Path(d['models']['100000']['states']['fdb']);env['FDB_CANCEL_LONG_IDS']='66498,45192,92149'
        d['completed'].append(run('fdb','scale-long-cancel',fs,'n100000-long',env));b.write(design,d)
    # Compatibility is a correctness check, not progression to another perf block.
    sys.path.insert(0,str(R/'tools'));old=importlib.import_module('kill-block2-scale');old.P=P+'-compat-n100000'
    os.environ.update({k:v for k,v in env.items() if k.startswith('FDB_')})
    d['old_reader']=old.old_reader(Path(d['models']['100000']['states']['fdb']))
    d.update(protected_after=b.protected(),provided_source_after=b.sha(CORPUS),slot_pointer=str(CURRENT))
    assert d['protected_before']==d['protected_after'] and d['provided_source_sha256']==d['provided_source_after']
    d.update(protected_unchanged=True,provided_source_unchanged=True)
    b.write(design,d)

if __name__=='__main__':main()
