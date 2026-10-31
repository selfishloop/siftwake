"""Post-hoc user-approved gates; immutable original tables and raw observations."""
import hashlib
import json
import math
from collections import defaultdict
from pathlib import Path

R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='write50-revised-20261029-v1';OLD='hybrid4r50-20261029-v1-b1'
read=lambda p:json.loads(p.read_text(encoding='utf-8-sig'))
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest().upper()
pct=lambda xs,p:sorted(xs)[math.ceil(p*len(xs))-1]

def main():
    dest=L/f'{P}-gate.json';assert not dest.exists()
    d=read(L/f'{OLD}-design.json');old=read(L/f'{OLD}-gate.json');pooled=defaultdict(list);runs=[];oracle=[];hashes={}
    for r in d['completed']:
        if not r['mode'].startswith('snapshot'):continue
        p=L/r['log'];hashes[p.name]=sha(p)
        for x in map(json.loads,p.read_text(encoding='utf-8').splitlines()):
            if x['phase'] in ['snapshot_idle_single','snapshot_idle_four']:
                rr=[z for reader in x['results'] for z in reader['records']]
                assert all(z['oracle'] for z in rr)
                oracle.append(dict(engine=r['engine'],label=r['label'],phase=x['phase'],records=len(rr),all_oracle=True,changed_group_checks=sum(z['checked_group_records'] for z in rr)))
            if x['phase']=='snapshot_load':
                arm='control' if x['control'] else 'loaded';xs=[v['commit_ms'] for v in x['writes']];pooled[r['engine'],arm].extend(xs)
                runs.append(summarize(r['engine'],arm,r['label'],xs))
    queries=[]
    for x in old['cases']:
        limit=30 if x['idle_p95_ms']<=30 else 2*x['idle_p95_ms']
        queries.append(dict(**x,new_limit_ms=limit,new_passed=x['loaded_p95_ms']<=limit,old_passed=x['passed']))
    summary=[summarize(e,a,'pooled',v) for (e,a),v in pooled.items()]
    report=R/'docs/WRITE-FOLLOWUP-50K-20261029.md'
    result=dict(authority='Direct user decision AFTER seeing results; SONRADAN REVIZE. Original gates/tables preserved.',query_rule='idle p95<=30ms: loaded<=30ms; otherwise loaded<=2*idle',long_rule='p95<=20,p99<=50,p99.9<=100ms; max informational; no SQLite acceptance gate',percentile='nearest-rank ceil(p*n)-1',queries=queries,runs=runs,pooled=summary,oracle_idle=oracle,raw_sha256=hashes,original_report_sha256=sha(report),original_gate_sha256=sha(L/f'{OLD}-gate.json'),original_gate_passed=False)
    result['query_passed']=all(x['new_passed'] for x in queries if x['engine']=='fdb')
    result['long_passed']=all(x['new_passed'] for x in runs if x['engine']=='fdb')
    result['throughput_passed']=.75<=old['throughput'][0]['loaded_control_ratio'] and old['throughput'][0]['loaded_control_ratio']>old['throughput'][1]['loaded_control_ratio']
    result['passed']=result['query_passed'] and result['long_passed'] and result['throughput_passed']
    dest.write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps(dict(passed=result['passed'],fdb_query_count=sum(x['new_passed'] for x in queries if x['engine']=='fdb'),pooled=summary)))

def summarize(e,arm,label,xs):
    a,b,c=pct(xs,.95),pct(xs,.99),pct(xs,.999);n=sum(x>100 for x in xs)
    return dict(engine=e,arm=arm,label=label,n=len(xs),p95_ms=a,p99_ms=b,p999_ms=c,max_ms=max(xs),over100_count=n,over100_ratio=n/len(xs),new_passed=a<=20 and b<=50 and c<=100,old_max_gate_passed=a<=20 and b<=50 and max(xs)<=100)

if __name__=='__main__':main()
