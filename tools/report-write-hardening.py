"""Derive all outcomes, including failures, from immutable Block1 logs."""
import importlib
import json
import math
from pathlib import Path

b = importlib.import_module('measure-churn')
DATE = '20261027'


def phases(path):
    process = json.loads(path.with_suffix('.process.json').read_text(encoding='utf-8'))
    assert process['exit_code'] == 0, path
    return {r['phase']: r for r in map(json.loads, path.read_text(encoding='utf-8').splitlines())}


def p95(xs):
    return sorted(xs)[math.ceil(.95 * len(xs))-1]


def records(phase):
    return [r for thread in phase['results'] for r in thread['records']]


def main():
    result = dict(boundary=[], throughput=[], churn=[])
    cases=json.loads((b.LOGS/'churn-10k-20261024-corpus-index.json').read_text(encoding='utf-8'))['cases']
    lines = ['# Blok1 ham kanıttan türetilen tablolar', '']
    prefix = f'write-hardening-boundary-v4-10k-{DATE}'
    lines += ['| Çift/kol | n | Commit p95 /max ms | >50ms | Görünürlük max ms | OS peak private MiB |', '|---|---:|---:|---:|---:|---:|']
    for path in sorted(b.LOGS.glob(f'{prefix}-*-small-boundary.jsonl')):
        if not path.with_suffix('.process.json').exists():
            continue
        row = phases(path)['small-boundary']
        rs = row['records']
        xs = [r['commit_ms'] for r in rs]
        r = dict(label=path.name[len(prefix)+1:].split('-streamed-')[0], n=len(xs), p95_ms=p95(xs), max_ms=max(xs),
                 over50=sum(t>50 for t in xs), visibility_max_ms=max(r['write_to_verified_visibility_ms'] for r in rs),
                 memory=row['memory'], stats=row['stats'], oracle=row['oracle'], log=path.name)
        result['boundary'].append(r)
        lines.append(f'| {r["label"]} | {r["n"]} | {r["p95_ms"]:.3f} /{r["max_ms"]:.3f} | {r["over50"]} | {r["visibility_max_ms"]:.3f} | {r["memory"]["os_lifetime_peak_private_commit_bytes"]/2**20:.2f} |')
    prefix = f'write-hardening-throughput-v4-10k-{DATE}'
    lines += ['', '| Motor/grup/çift | Kontrol /yük ops/s | Oran | ≥%80 | Mutlak commit max ms (yük) |', '|---|---:|---:|---|---:|']
    for engine in ['fdb','sqlite']:
        for batch in ['one','100']:
            trials = []
            pool = {'snapshot_idle_four': [], 'snapshot_load': []}
            for pair in range(4):
                ps = {arm: b.LOGS/f'{prefix}-{engine}-{batch}-r{pair}-{arm}-{engine}-streamed-snapshot-{"control-" if arm=="control" else ""}{batch}.jsonl' for arm in ['load','control']}
                if not all(p.with_suffix('.process.json').exists() for p in ps.values()):
                    continue
                data = {arm: phases(p) for arm,p in ps.items()}
                load,control = [data[a]['snapshot_load'] for a in ['load','control']]
                assert load['commits']==control['commits'] and load['oracle'] and control['oracle']
                rs = [r for r in records(load) if r['writer_active']]
                idle = records(data['load']['snapshot_idle_four'])
                for phase,recs in [('snapshot_load',rs),('snapshot_idle_four',idle)]:pool[phase]+=recs
                coverage = load['maintenance'].get('details',{})
                query = []
                for case in range(38):
                    a = [r['engine_ms'] for r in idle if r['case_id']==case]
                    z = [r['engine_ms'] for r in rs if r['case_id']==case]
                    query.append(dict(case_id=case, idle_p95_ms=p95(a), load_p95_ms=p95(z) if z else None,
                                      active_n=len(z), pass_2x=(p95(z)-p95(a)<=2 if p95(a)<1 else p95(z)<=p95(a)*2) if z else False))
                r = dict(engine=engine,batch=batch,pair=pair,ratio=load['ops_s']/control['ops_s'],
                         load_ops_s=load['ops_s'],control_ops_s=control['ops_s'],commits=load['commits'],
                         load_elapsed_ms=load['writer_elapsed_ms'],control_elapsed_ms=control['writer_elapsed_ms'],
                         commit_max_ms=max(w['commit_ms'] for w in load['writes']),memory=load['memory'],
                         publication=load['publication'],maintenance=load['maintenance'],query=query,
                         normal_merges_completed=coverage.get('normal_merges_completed'),revision_rewrites_completed=coverage.get('revision_rewrites_completed'),
                         writer_waited=sum(r['writer_waited'] for r in rs), max_writer_wait_ms=max((r['max_writer_wait_ns'] for r in rs),default=0)/1e6,
                         all_four_overlap_maintenance=all(any(r['started_ms']<load['maintenance']['end_ms'] and r['finished_ms']>load['maintenance']['start_ms'] for r in rs if r['thread']==thread) for thread in range(4)))
                trials.append(r);result['throughput'].append(r)
                lines.append(f'| {engine}/{batch}/r{pair} | {r["control_ops_s"]:.2f} /{r["load_ops_s"]:.2f} | {r["ratio"]*100:.2f}% | {"GEÇTİ" if r["ratio"]>=.8 else "KALDI"} | {r["commit_max_ms"]:.3f} |')
            if trials:
                ratio=sum(t['control_elapsed_ms'] for t in trials)/sum(t['load_elapsed_ms'] for t in trials)
                passed=sum(t['ratio']>=.8 for t in trials)
                summary=dict(ratio=ratio,observed_pairs=len(trials),planned_pairs=4,passing_pairs=passed,
                             impossible_3of4=passed+4-len(trials)<3,
                             pass_gate=len(trials)==4 and ratio>=.8 and passed>=3)
                result.setdefault('pooled',{})[f'{engine}-{batch}']=summary
                lines.append(f'| {engine}/{batch}/pooled | gözlenen{len(trials)}/4 çift, eşit işlem | {ratio*100:.2f}% | {"GEÇTİ" if summary["pass_gate"] else "KALDI"} ({passed} geçen) | |')
                matrix=[]
                for case in range(38):
                    a=[r['engine_ms'] for r in pool['snapshot_idle_four'] if r['case_id']==case]
                    z=[r['engine_ms'] for r in pool['snapshot_load'] if r['case_id']==case]
                    matrix.append(dict(case_id=case,idle_p95_ms=p95(a),load_p95_ms=p95(z) if z else None,active_n=len(z),absolute_delta_ms=p95(z)-p95(a) if z else None))
                result.setdefault('matrices',{})[f'{engine}-{batch}']=matrix
                lines+=['',f'## {engine}/{batch}: gözlenen çiftlerde writer_active sorguları','',
                        '| Sorgu /offset/limit | Yük örnek n | Boşta4R p95 /yük p95 ms | Mutlak fark ms | Önceki2× /kısa+2ms sınırı |',
                        '|---|---:|---:|---:|---|']
                for q in matrix:
                    case=cases[q['case_id']];label=case['text'];label=label if len(label)<40 else label[:32]+'…'
                    if q['load_p95_ms'] is None:
                        lines.append(f'| {q["case_id"]}: {label} /{case["offset"]}/{case["limit"]} | 0 | {q["idle_p95_ms"]:.4f} /ölçülmedi | | ÖLÇÜLMEDİ |');continue
                    passed=(q['absolute_delta_ms']<=2 if q['idle_p95_ms']<1 else q['load_p95_ms']<=2*q['idle_p95_ms'])
                    q['pass_2x']=passed
                    lines.append(f'| {q["case_id"]}: {label} /{case["offset"]}/{case["limit"]} | {q["active_n"]} | {q["idle_p95_ms"]:.4f} /{q["load_p95_ms"]:.4f} | {q["absolute_delta_ms"]:+.4f} | {"GEÇTİ" if passed else "KALDI"} |')
    prefix=f'write-hardening-churn-v4-10k-{DATE}'
    lines+=['','| Kol/tur | Retained oran önce /sonra | Ölü MB | Tombstone | Commit toplam ms | Idle /bakım ms |','|---|---:|---:|---:|---:|---:|']
    for path in sorted(b.LOGS.glob(f'{prefix}-*-write-churn-30.jsonl')):
        if not path.with_suffix('.process.json').exists():continue
        rows=[json.loads(s) for s in path.read_text(encoding='utf-8').splitlines()]
        base=next(r for r in rows if r['phase']=='write_churn_baseline')['disk']['retained_bytes']
        for row in [r for r in rows if r['phase']=='write_churn_round']:
            live=row['liveness_after'] or []
            r=dict(label=path.name[len(prefix)+1:].split('-streamed-')[0],round=row['round'],baseline_bytes=base,
                   before_ratio=row['before']['retained_bytes']/base,after_ratio=row['after']['retained_bytes']/base,
                   dead_payload_bytes=sum(s['dead_payload_bytes'] for s in live),tombstones=sum(s['tombstones'] for s in live),
                   commit_sum_ms=sum(row['commits_ms']),commit_max_ms=max(row['commits_ms']),maintenance_ms=row['maintenance_ms'],idle_ms=row['idle_ms'],memory=row['memory'])
            result['churn'].append(r)
            lines.append(f'| {r["label"]}/{r["round"]} | {r["before_ratio"]:.3f} /{r["after_ratio"]:.3f} | {r["dead_payload_bytes"]/1e6:.3f} | {r["tombstones"]} | {r["commit_sum_ms"]:.2f} | {r["idle_ms"]:.1f} /{r["maintenance_ms"]:.1f} |')
    b.write(b.LOGS/f'write-hardening-10k-{DATE}-summary.json',result)
    (b.LOGS/f'write-hardening-10k-{DATE}-tables.md').write_text('\n'.join(lines)+'\n',encoding='utf-8')
    print(json.dumps(result.get('pooled',{})),flush=True)


if __name__=='__main__':main()
