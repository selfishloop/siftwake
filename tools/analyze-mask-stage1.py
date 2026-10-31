"""Extract small summaries without materializing the large profile JSON trace."""
import json, math, re
from pathlib import Path
L=Path(__file__).resolve().parent.parent/'docs/benchmarks'
P='mask-stage1-20261030-same'
def pct(values,p):
    return sorted(values)[max(0,math.ceil(len(values)*p)-1)]
def main():
    arms=[];pooled={k:[] for k in ['control','candidate']}
    for i,role in enumerate(['control','candidate','candidate','control']):
        path=L/f'{P}-arm{i}-{role}-snapshot-control-1.jsonl'
        with path.open('rb') as f:
            head=f.read(16384).decode('utf-8');f.seek(max(0,path.stat().st_size-12*1024**2));tail=f.read().decode('utf-8')
        publication=json.loads(re.search(r'"publication":(\{[^}]*\})',head)[1])
        j=tail.rfind('"writes":');writes=json.JSONDecoder().raw_decode(tail[j+9:])[0]
        values=[w['commit_ms'] for w in writes];assert len(values)==16384;pooled[role].extend(values)
        arms.append(dict(role=role,arm=i,log=path.name,publication=publication,
            p95=pct(values,.95),p99=pct(values,.99),p999=pct(values,.999),max=max(values),
            over50=[w for w in writes if w['commit_ms']>50],over100=sum(v>100 for v in values)))
    reads=[];by_role={k:{} for k in pooled}
    for i,role in enumerate(['control','candidate','candidate','control']):
        path=L/f'{P}-read{i}-{role}-snapshot-one.jsonl';rows=[json.loads(x) for x in path.read_text(encoding='utf-8').splitlines()]
        probe=next(x for x in rows if x['phase']=='snapshot_idle_probe');assert probe['oracle']
        for reader in probe['results']:
            for r in reader['records']:by_role[role].setdefault(r['case_id'],[]).append(r['engine_ms'])
        memory=rows[-1]['memory'];reads.append(dict(role=role,arm=i,memory=memory,log=path.name))
    cases=[dict(case_id=i,control_p95=pct(by_role['control'][i],.95),candidate_p95=pct(by_role['candidate'][i],.95),
        control_samples=len(by_role['control'][i]),candidate_samples=len(by_role['candidate'][i])) for i in sorted(by_role['control'])]
    result=dict(arms=arms,pooled={role:dict(p95=pct(v,.95),p99=pct(v,.99),p999=pct(v,.999),max=max(v),over50=sum(x>50 for x in v),over100=sum(x>100 for x in v)) for role,v in pooled.items()},
        reads=reads,cases=cases,reader_ram_decision='INCONCLUSIVE; identical executable/path no-op controls also vary; no non-regression pass claimed',
        engine_ram_gate='not measured by this model/oracle/profile harness')
    (L/'mask-stage1-20261030-summary.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps(result['pooled']))
if __name__=='__main__':main()
