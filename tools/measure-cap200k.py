"""User-confirmed capped corpus, 50k then100k gates, fixed reusable DB slots."""
import argparse
import importlib
import importlib.util
import json
import math
import os
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='cap200k-scale-20261029-v1'
spec=importlib.util.spec_from_file_location('scale',R/'tools/measure-trwiki-scale.py')
s=importlib.util.module_from_spec(spec);spec.loader.exec_module(s)
s.P=P
s.CORPUS=Path('D:/frunner-corpus/trwiki-100k-cap200k/corpus.payload-bench')
s.CATALOG=L/'trwiki-cap200k-20261029-v3-catalog.json'
s.PROGRAMS={'fdb':R/'output/posting-pages-20261029-v3-fdb.exe','sqlite':R/'output/trwiki-scale-20261028-v2-sqlite.exe'}
A=L/'trwiki-cap200k-20261029-v3-audit.json'

def p95(xs):return sorted(xs)[math.ceil(.95*len(xs))-1]

def gates(n,results,audit):
    opens=[];idle=[];peak=[];matrix=defaultdict(list);blocks=defaultdict(list);negative=defaultdict(list)
    for result in results:
        if result['engine']!='fdb':continue
        for x in s.rows(result):
            if x['phase']=='opened' and result['mode']=='scale-idle':opens.append(x['open_ms'])
            if x['phase']=='first_result':opens.append(x['session_ms'])
            if x['phase']=='scale_idle':idle.append(x['memory']['private_bytes'])
            if x['phase']=='scale_ingested':peak.append(x['memory']['os_lifetime_peak_private_commit_bytes'])
            if x['phase']=='query_experiment':
                target=negative if 'negative' in result['label'] else matrix
                target[x['case_id'],x['variant']].append(x)
            if x['phase']=='cpu_block':blocks[x['text'],x['variant']].append(x)
    failed=[];queries=[];relative=[]
    for (i,variant),xs in matrix.items():
        if variant!='cap-hybrid':continue
        case=audit['query_cases'][i];group=audit['query_groups'][case['text']]
        limit=None if group=='fixed_short' else (30 if group.startswith(('very_common','common_')) else (150 if n==50000 else 300))
        value=p95([x['elapsed_ms'] for x in xs]);ok=limit is None or value<=limit
        item=dict(case_id=i,text=case['text'],p95_ms=value,limit_ms=limit,excluded_short=limit is None,passed=ok)
        queries.append(item)
        if not ok:failed.append(dict(kind='main_query',**item))
    if opens and p95(opens)>(1000 if n==50000 else 2000):failed.append(dict(kind='strict_open',p95_ms=p95(opens)))
    if idle and max(idle)>15*2**20:failed.append(dict(kind='idle_private',bytes=max(idle)))
    if peak and max(peak)>60*2**20:failed.append(dict(kind='ingest_peak_private',bytes=max(peak)))
    for i in [39,40]:
        if (i,'cap-four') not in negative:continue
        control=p95([x['elapsed_ms'] for x in negative[i,'cap-four']]);hybrid=p95([x['elapsed_ms'] for x in negative[i,'cap-hybrid']])
        limit=control+2 if control<1 else control*1.1
        item=dict(kind='negative_hybrid',text=audit['query_cases'][i]['text'],control_p95_ms=control,hybrid_p95_ms=hybrid,limit_ms=limit,passed=hybrid<=limit)
        relative.append(item)
        if not item['passed']:failed.append(item)
    for q in sorted({q for q,v in blocks}):
        averages={v:sum(x['cpu_ms'] for x in blocks[q,v])/sum(x['queries'] for x in blocks[q,v]) for v in ['cap','cap-hybrid']}
        serial=averages['cap'];hybrid=averages['cap-hybrid'];limit=serial+.2 if serial<10 else serial*1.05
        item=dict(kind='positive_cpu',text=q,serial_ms_per_call=serial,hybrid_ms_per_call=hybrid,limit_ms_per_call=limit,passed=hybrid<=limit)
        relative.append(item)
        if not item['passed']:failed.append(item)
    return dict(documents=n,strict_open_samples_ms=opens,idle_private_samples=idle,ingest_peak_private_samples=peak,
        per_case=queries,relative=relative,failures=failed,passed=not failed,
        scope='Hybrid200 N50 primary; exact serial informational; ve/if/id/zx excluded from deadline')

def recovery(n,state,env):
    before=s.sources();ke=env.copy();ke.update(FDB_FIRST_QUERY='olduğu',FDB_FIRST_VARIANT='cap-hybrid')
    stderr=L/f'{P}-n{n}-pending-kill.stderr';assert not stderr.exists()
    with stderr.open('w',encoding='utf-8') as err:
        child=subprocess.Popen([str(s.PROGRAMS['fdb']),'first-background','streamed',str(state)],
            cwd=R,env=ke,stdout=subprocess.PIPE,stderr=err,text=True,encoding='utf-8',creationflags=subprocess.CREATE_NO_WINDOW)
        try:
            witness=json.loads(child.stdout.readline())
            assert witness['phase']=='session_return' and witness['status']=='pending',witness
            child.kill();code=child.wait(timeout=10)
        finally:
            if child.poll() is None:child.kill();child.wait(timeout=10)
    assert code!=0
    reopened=s.run('fdb','first-strict',state,f'n{n}-reopen-after-kill',ke)
    assert any(x['phase']=='first_result_oracle' and x['oracle'] for x in s.rows(reopened))
    sys.path.insert(0,str(R/'tools'));old=importlib.import_module('kill-block2-scale');old.P=P+f'-compat-n{n}'
    os.environ.update({k:v for k,v in env.items() if k.startswith('FDB_')})
    compatible=old.old_reader(state)
    assert before==s.sources()
    result=dict(pending_kill=witness,exit_code=code,reopen=reopened,old_reader=compatible,source_hashes_unchanged=True,power_loss_claim=False)
    s.b.write(L/f'{P}-n{n}-recovery.json',result)
    print(json.dumps(dict(documents=n,recovery=True,old_reader=True)),flush=True)
    return result

def main():
    ap=argparse.ArgumentParser();ap.add_argument('--confirmed-sha',required=True);args=ap.parse_args()
    design=L/f'{P}-design.json';assert not design.exists(),'No replay/reset of completed evidence'
    audit=json.loads(A.read_text(encoding='utf-8'))
    assert args.confirmed_sha.upper()==audit['payload_sha256']==s.b.sha(s.CORPUS)
    assert audit['same_order_trim_only_verified'] and audit['query_overlap']==41 and audit['unique_text_bodies']==100000
    assert s.b.sha(s.CATALOG)==audit['catalog_sha256']
    checks=json.loads((L/'snapshot-10k-20261026-validation-cap200k-20261029-v1.json').read_text(encoding='utf-8-sig'))
    assert len(checks)==8 and all(x['exit_code']==0 for x in checks)
    env=os.environ.copy();env.update(PYTHONUTF8='1',FDB_BENCH_CORPUS_PATH=str(s.CORPUS),
        FDB_BENCH_CORPUS_INDEX=str(s.CATALOG),FDB_BENCH_ORIGINAL_BODIES='1',FDB_STREAMED_WAL_LAYER='1',
        FDB_RECLAMATION_CONTROL='1',FDB_QUERY_CASES=','.join(map(str,range(41))),FDB_SCALE_QUERY_REPEATS='10')
    d=dict(programs={e:dict(path=str(p.relative_to(R)),sha256=s.b.sha(p)) for e,p in s.PROGRAMS.items()},
        provided_source_sha256=s.b.sha(s.CORPUS),catalog_sha256=s.b.sha(s.CATALOG),audit=str(A),
        source_accepted_by_user=True,original_report_hash_851_not_used=True,
        protected_before=s.b.protected(),implementation_source_sha256={str(p.relative_to(R)):s.b.sha(p) for p in sorted((R/'src').rglob('*.rs'))},
        query_cases=audit['query_cases'],variants=['legacy','exact-four','cap','cap-four','cap-hybrid'],samples_per_query_variant=20,
        orders=[['fdb','sqlite'],['sqlite','fdb']],main='Hybrid N50 budget200; exact/serial informational',
        memory='Private and WS process counters separate; ingest descriptor/model included, idle oracle unloaded',
        cache='Warm OS/engine, full GET oracle+one warmup per variant; first-result fresh process with all arm files streamed warm; not forced cold',
        gate_policy='50k then100k; stop dependent FDB search/cancel on any gate failure, finish independent ingest/disk/RAM/SQLite',
        completed=[],levels=[],models={},gate_failed_at=None,first_queries=['olduğu','akarsu','furlong','ilçesine köyleri','zx','qzxkvwnever'])
    s.b.write(design,d)
    pointer=dict(corpus='trwiki-cap200k-user-confirmed',source=str(s.CORPUS),sha256=audit['payload_sha256'],catalog=str(s.CATALOG),original_bodies=True,slots={})
    for n in [50000,100000]:
        env['FDB_SCALE_DOCUMENTS']=str(n);full=d['gate_failed_at'] is None;level=[]
        for pair,order in enumerate(d['orders']):
            states={e:L/f'{P}-n{n}-pair{pair}-{e}-state.json' for e in s.PROGRAMS}
            for e in order:
                assert s.b.sha(s.CORPUS)==audit['payload_sha256'] and not states[e].exists()
                pointer['slots'][e]=dict(documents=n,status='resetting_owned_slot',state=str(states[e]));s.b.write(s.CURRENT,pointer)
                s.b.reset(e);se=env.copy()
                if e=='fdb':se['FDB_SEED_IDLE_CLEANUP']='1'
                r=s.run(e,'scale-seed',states[e],f'n{n}-pair{pair}-seed',se);level.append(r);d['completed'].append(r)
                pointer['slots'][e].update(status='ready',model_sha256=s.b.sha(states[e]));s.b.write(s.CURRENT,pointer);s.b.write(design,d)
            assert s.b.sha(states['fdb'])==s.b.sha(states['sqlite'])
            d['models'][str(n)]=dict(count=n,states={e:str(p) for e,p in states.items()},model_sha256=s.b.sha(states['fdb']))
            before=s.sources()
            for sample in range(2):
                for e in order:
                    r=s.run(e,'scale-idle',states[e],f'n{n}-pair{pair}-idle{sample}',env);level.append(r);d['completed'].append(r)
            for e in order:
                if e=='fdb' and not full:continue
                qe=env.copy();qe['FDB_QUERY_VARIANTS']='legacy,exact-four,cap,cap-four,cap-hybrid' if e=='fdb' else 'legacy,cap'
                r=s.run(e,'scale-query-experiments',states[e],f'n{n}-pair{pair}-matrix',qe);level.append(r);d['completed'].append(r);s.b.write(design,d)
            if full:
                ce=env.copy();ce.update(FDB_QUERY_CASES='0,1,2',FDB_QUERY_VARIANTS='cap,cap-four,cap-hybrid' if pair==0 else 'cap-hybrid,cap-four,cap',FDB_CPU_BLOCK_REPEATS='200')
                r=s.run('fdb','scale-query-cpu-blocks',states['fdb'],f'n{n}-pair{pair}-cpu',ce);level.append(r);d['completed'].append(r)
                ne=env.copy();ne.update(FDB_QUERY_CASES='39,40',FDB_QUERY_VARIANTS='cap-four,cap-hybrid')
                r=s.run('fdb','scale-query-experiments',states['fdb'],f'n{n}-pair{pair}-negative',ne);level.append(r);d['completed'].append(r)
                for qi,q in enumerate(d['first_queries']):
                    fe=env.copy();fe.update(FDB_FIRST_QUERY=q,FDB_FIRST_VARIANT='cap-hybrid')
                    for e in order:
                        s.warm(e);r=s.run(e,'first-strict' if e=='fdb' else 'first-sqlite',states[e],f'n{n}-pair{pair}-first{qi}',fe);level.append(r);d['completed'].append(r)
            assert before==s.sources();s.b.write(design,d)
        gate=gates(n,level,audit);s.b.write(L/f'{P}-n{n}-gates.json',gate)
        d['levels'].append(dict(documents=n,full_fdb_measurement=full,gate=gate))
        d.setdefault('recovery',{})[str(n)]=recovery(n,Path(d['models'][str(n)]['states']['fdb']),env)
        if full and not gate['passed']:
            d['gate_failed_at']=n;print(json.dumps(dict(gate_failed_at=n,failures=gate['failures'],next='independent nextN ingest/disk/RAM/SQLite only')),flush=True)
        s.b.write(design,d)
    if d['gate_failed_at'] is None:
        env.update(FDB_SCALE_DOCUMENTS='100000',FDB_CANCEL_LONG_IDS='66498,45192')
        d['completed'].append(s.run('fdb','scale-long-cancel',Path(d['models']['100000']['states']['fdb']),'n100000-long-cancel',env))
    d.update(protected_after=s.b.protected(),provided_source_after=s.b.sha(s.CORPUS))
    assert d['protected_before']==d['protected_after'] and d['provided_source_sha256']==d['provided_source_after']
    d.update(protected_unchanged=True,provided_source_unchanged=True)
    s.b.write(design,d)

if __name__=='__main__':main()
