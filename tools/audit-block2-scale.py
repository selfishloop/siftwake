"""Audit the first Block2 checkpoint without starting additional workloads."""
import hashlib
import json
import math
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LOGS = ROOT / 'docs/benchmarks'
P = 'block2-scale-20261028-v1'


def read_log(n, pair, engine, mode):
    path = LOGS / f'{P}-n{n}-r{pair}-{engine}-{mode}.jsonl'
    process = json.loads(path.with_suffix('.process.json').read_text(encoding='utf-8'))
    assert process['exit_code'] == 0
    return [json.loads(line) for line in path.read_text(encoding='utf-8').splitlines()]


def phase(rows, name):
    return next(row for row in rows if row['phase'] == name)


def p95(values):
    return sorted(values)[math.ceil(len(values) * .95) - 1]


def main():
    out = LOGS / f'{P}-audit-utf8-v2.json'
    if out.exists():
        raise RuntimeError('Immutable audit already exists')
    design = json.loads((LOGS / f'{P}-design.json').read_text(encoding='utf-8'))
    assert design['protected_unchanged']
    assert len(design['completed']) == 24
    old = json.loads((LOGS / 'final-gates-10k-20261027-audit.json').read_text(encoding='utf-8'))['source_sha256']
    current = {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest().upper()
               for p in (ROOT / 'src').rglob('*.rs')}
    assert old == current, 'Engine changed since accepted Block1'
    checks = json.loads((LOGS / 'snapshot-10k-20261026-validation-block2-scale-20261028-v1.json').read_text(encoding='utf-8'))
    assert len(checks) == 8 and all(c['exit_code'] == 0 for c in checks)
    test_log = (LOGS / 'snapshot-10k-20261026-validation-block2-scale-20261028-v1-test.log').read_text(encoding='utf-8-sig')
    counts = [tuple(map(int,m)) for m in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored', test_log)]
    result = dict(engine_source_unchanged=True, source_sha256=current, tests=dict(passed=sum(x[0] for x in counts), failed=sum(x[1] for x in counts), ignored=sum(x[2] for x in counts)),
                  protected_unchanged=True, trials=[], queries=[], gates={})
    matrices = {}
    for n in (10000,50000):
        for engine in ('fdb','sqlite'):
            matrices[n,engine] = []
            for pair in (0,1):
                seed = read_log(n,pair,engine,'scale-seed')
                ingest = phase(seed,'scale_ingested')
                assert phase(seed,'scale_verified')['oracle']
                idle_rows = read_log(n,pair,engine,'scale-idle')
                idle = phase(idle_rows,'scale_idle')
                matrix = phase(read_log(n,pair,engine,'scale-matrix'),'matrix')
                assert len(matrix['records']) == 38 and matrix['oracle']
                assert all(len(r['samples_ms']) == 10 for r in matrix['records'])
                matrices[n,engine].append(matrix)
                result['trials'].append(dict(n=n,pair=pair,engine=engine,
                    ingest_ms=ingest['ingest_including_maintenance_ms'], commits_ms=ingest['commit_total_ms'], maintenance_ms=ingest['maintenance_ms'],
                    idle_private_bytes=idle['memory']['private_bytes'], idle_ws_bytes=idle['memory']['working_set_bytes'],
                    ingest_peak_private_bytes=ingest['memory']['os_lifetime_peak_private_commit_bytes'], ingest_peak_ws_bytes=ingest['memory']['os_lifetime_peak_working_set_bytes'],
                    retained_bytes=idle['disk']['retained_bytes'], open_ms=phase(idle_rows,'opened')['open_ms']))
    for i in range(38):
        record = dict(case_id=i,case=matrices[10000,'fdb'][0]['records'][i]['case'])
        for n in (10000,50000):
            for engine in ('fdb','sqlite'):
                samples = [s for m in matrices[n,engine] for s in m['records'][i]['samples_ms']]
                record[f'{engine}_{n}_p95_ms'] = p95(samples)
        record['fdb_growth'] = record['fdb_50000_p95_ms']/record['fdb_10000_p95_ms']
        record['fdb_absolute_delta_ms'] = record['fdb_50000_p95_ms'] - record['fdb_10000_p95_ms']
        record['linear_target_pass'] = record['fdb_growth'] <= 5
        result['queries'].append(record)
    fdb50 = [t for t in result['trials'] if t['engine']=='fdb' and t['n']==50000]
    catalog = json.loads((LOGS / f'{P}-corpus-index.json').read_text(encoding='utf-8'))
    prior = json.loads((LOGS / 'churn-10k-20261024-corpus-index.json').read_text(encoding='utf-8'))
    assert prior['cases'] == catalog['cases']
    assert prior['entries'] == catalog['entries'][:10000]
    result['first_10k_metadata_identical'] = True
    result['catalog_sha256'] = hashlib.sha256((LOGS / f'{P}-corpus-index.json').read_bytes()).hexdigest().upper()
    for q in result['queries']:
        c = q['case']
        for n in (10000,50000):
            matches = sum(bool(e['mask'] & (1 << c['base'])) for e in catalog['entries'][:n])
            q[f'matches_{n}'] = matches
            q[f'page_hits_{n}'] = min(c['limit'],max(0,matches-c['offset']))
    common = [q for q in result['queries'] if q['case']['base'] in (16,17,18)]
    result['gates'] = dict(idle_15_decimal_MB=all(t['idle_private_bytes']<=15_000_000 for t in fdb50),
                           ingest_peak_60_decimal_MB=all(t['ingest_peak_private_bytes']<=60_000_000 for t in fdb50),
                           common_p95_30_ms=all(q['fdb_50000_p95_ms']<=30 for q in common),
                           search_at_most_linear=all(q['linear_target_pass'] for q in result['queries']))
    result['core_gates_pass'] = all(result['gates'].values())
    first = json.loads((LOGS / f'{P}-audit.json').read_text(encoding='utf-8'))
    assert result['gates'] == first['gates'] and result['trials'] == first['trials']
    assert all({k:v for k,v in a.items() if k!='case'}=={k:v for k,v in b.items() if k!='case'}
               for a,b in zip(result['queries'],first['queries']))
    result['supersedes'] = dict(file=f'{P}-audit.json',reason='Explicit UTF8 decoding fixes two query labels; all numeric timings/trials/gates unchanged')
    out.write_text(json.dumps(result,indent=2),encoding='utf-8')
    print(json.dumps({k:v for k,v in result.items() if k not in ('queries','source_sha256')},indent=2),flush=True)


if __name__ == '__main__':
    main()
