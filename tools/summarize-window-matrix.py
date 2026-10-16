"""Aggregate read-only paired matrix logs; no corpus/database access."""
import json
import math
import statistics
import sys
from pathlib import Path

root = Path(__file__).resolve().parent.parent
prefix = root / (sys.argv[1] if len(sys.argv)>1 else 'docs/benchmarks/window-matrix-10k-20261013')
def read(suffix):
    return {r['phase']: r for r in map(json.loads, Path(f'{prefix}-{suffix}.jsonl').read_text(encoding='utf-8-sig').splitlines())}
def stats(rows):
    for row in rows:
        assert row['n'] == len(row['samples']) and row['n'] > 0
        ordered = sorted(row['samples'])
        assert row['p95_ms'] == ordered[math.ceil(len(ordered)*95/100)-1]
    values = sorted(x for row in rows for x in row['samples'])
    return {'n': len(values), 'per_process_n': [row['n'] for row in rows], **{f'p{p}_ms': values[math.ceil(len(values)*p/100)-1] for p in (50,95,99)}, 'run_p95': [row['p95_ms'] for row in rows]}
design_path=Path(f'{prefix}-design.json')
design=json.loads(design_path.read_text(encoding='utf-8-sig')) if design_path.exists() else {'pairs':3,'balanced_first_position':False}
pair_count=design['pairs']
runs = {e: [read(f'{e}-{n}')['matrix_reads'] for n in range(pair_count)] for e in ('control','fdb')}
assert all(row['full_document_checks'] for rows in runs.values() for row in rows)
queries=[]
for n, q in enumerate(runs['control'][0]['queries']):
    identity={k:q[k] for k in ('text','kind','offset','limit','total','late_matches')}
    entry=dict(identity)
    for e in runs:
        series=[row['queries'][n] for row in runs[e]]
        assert all(all(row[k]==v for k,v in identity.items()) for row in series)
        entry[e]=stats([row['timing'] for row in series])
    entry['change_p95_percent']=100*(entry['fdb']['p95_ms']/entry['control']['p95_ms']-1)
    entry['faster_pairs']=sum(a<b for a,b in zip(entry['fdb']['run_p95'],entry['control']['run_p95']))
    entry['paired_p95_change_percent']=[100*(a/b-1) for a,b in zip(entry['fdb']['run_p95'],entry['control']['run_p95'])]
    entry['median_paired_p95_change_percent']=statistics.median(entry['paired_p95_change_percent'])
    entry['tail_samples_per_process_below_20']=any(n<20 for n in entry['control']['per_process_n']+entry['fdb']['per_process_n'])
    queries.append(entry)
result={'cache':'warm OS cache, no eviction','processes_per_fdb_variant':pair_count,'design':design,'queries':queries,'lower_p95_cases':sum(q['change_p95_percent']<0 for q in queries),'higher_p95_cases':sum(q['change_p95_percent']>0 for q in queries)}
artifact_path=Path(f'{prefix}-executables.json')
if artifact_path.exists():
    hashes={row['engine']:row['sha256'] for row in json.loads(artifact_path.read_text(encoding='utf-8-sig'))}
    result['same_executable_control']=hashes['control']==hashes['fdb']
result['interpretation']='Pooled p95 changes and paired process p95 changes describe different distributions; neither case count alone establishes engine regressions or statistical significance.'
if Path(f'{prefix}-sqlite-reference.jsonl').exists():
    result['sqlite_single_reference']=read('sqlite-reference')
Path(f'{prefix}-summary.json').write_text(json.dumps(result,indent=2,ensure_ascii=False),encoding='utf-8')
print(json.dumps({'lower_p95_cases':result['lower_p95_cases'],'higher_p95_cases':result['higher_p95_cases'],'increases':[q for q in queries if q['change_p95_percent']>0]},indent=2,ensure_ascii=False))
