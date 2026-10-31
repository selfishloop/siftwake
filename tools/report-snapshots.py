"""Reproducible derived tables, including every failing gate; no DB access."""
import importlib
import json
from pathlib import Path

b = importlib.import_module('measure-churn')
s = importlib.import_module('summarize-snapshots')
P = 'snapshot-10k-20261026'


def load(engine, batch, pair, arm):
    mode = f'snapshot-{batch}' if arm == 'load' else f'snapshot-control-{batch}'
    path = b.LOGS / f'{P}-{engine}-{batch}-r{pair}-{arm}-{engine}-streamed-{mode}.jsonl'
    process = json.loads(path.with_suffix('.process.json').read_text(encoding='utf-8'))
    assert process['exit_code'] == 0
    phases = {r['phase']: r for r in map(json.loads, path.read_text(encoding='utf-8').splitlines())}
    row = phases['snapshot_load']
    assert row['oracle'] and row['failure'] is None
    if arm == 'load':
        for phase in ['snapshot_idle_single', 'snapshot_idle_four', 'snapshot_load']:
            records = s.records(phases[phase])
            expected = 20 if engine == 'fdb' else 4
            for case in range(38):
                samples = [r for r in records if r['case_id'] == case]
                assert len(samples) == expected and all(r['oracle'] for r in samples)
            if phase == 'snapshot_load':
                assert all(r['writer_active'] for r in records)
        if engine == 'fdb':
            details = row['maintenance']['details']
            assert details['normal_merges_completed'] > 0
            assert details['revision_rewrites_completed'] > 0
            start, end = row['maintenance']['start_ms'], row['maintenance']['end_ms']
            assert all(any(r['started_ms'] < end and r['finished_ms'] > start
                           for r in s.records(row) if r['thread'] == thread) for thread in range(4))
    return path, phases


def main():
    output = dict(p95='nearest rank', pairs=[], matrices={}, raw=[])
    cases = json.loads((b.ROOT / 'docs/benchmarks/churn-10k-20261024-corpus-index.json').read_text(encoding='utf-8'))['cases']
    text = ['<!-- Generated tables; raw JSONL remains immutable. -->',
            '| Motor / grup / çift | Yazı sayısı yük/kontrol | ops/s kontrol → yük | Oran | Commit p95 yük, ms | Bakım kontrol → yük, ms |',
            '|---|---:|---:|---:|---:|---:|']
    for engine in ['fdb', 'sqlite']:
        for batch in ['one', '100']:
            collected = {p: [] for p in ['snapshot_idle_single', 'snapshot_idle_four', 'snapshot_load']}
            for pair in [0, 1]:
                path, phases = load(engine, batch, pair, 'load')
                control_path, control_phases = load(engine, batch, pair, 'control')
                row, control = phases['snapshot_load'], control_phases['snapshot_load']
                ratio = row['ops_s'] / control['ops_s']
                recs = s.records(row)
                record = dict(engine=engine, batch=batch, pair=pair, load_commits=row['commits'],
                              control_commits=control['commits'], ops_s=row['ops_s'], control_ops_s=control['ops_s'],
                              throughput_ratio=ratio, throughput_pass=ratio >= .8,
                              commit_p95=s.percentile([w['commit_ms'] for w in row['writes']]),
                              commit_max=max(w['commit_ms'] for w in row['writes']),
                              writer_elapsed_ms=row['writer_elapsed_ms'],control_elapsed_ms=control['writer_elapsed_ms'],
                              maintenance_ms=row['maintenance']['end_ms']-row['maintenance']['start_ms'],
                              control_maintenance_ms=control['maintenance']['end_ms']-control['maintenance']['start_ms'],
                              queries=len(recs), writer_waited=sum(r['writer_waited'] for r in recs),
                              max_writer_wait_ms=max(r['max_writer_wait_ns'] for r in recs)/1e6,
                              max_query_writer_wait_ms=max(r['writer_wait_ns'] for r in recs)/1e6,
                              writer_wait_sum_ms=sum(r['writer_wait_ns'] for r in recs)/1e6,
                              admission_waited=sum(r['admission']['contended'] for r in recs),
                              max_admission_ms=max(r['admission']['wait_ns'] for r in recs)/1e6,
                              max_release_ms=max(r['release_ms'] for r in recs),
                              release_reclaimed=sum(r['release']['reclaimed_files'] for r in recs),
                              max_engine_ms=max(r['engine_ms'] for r in recs),
                              max_held_ms=max(r['held_including_oracle_ms'] for r in recs),
                              memory=row['memory'],publication=row['publication'],
                              control_publication=control['publication'],
                              maintenance=row['maintenance'],log=path.name,control_log=control_path.name)
                record['read_fails'] = {}
                for baseline in ['single', 'four']:
                    idle_records = s.records(phases['snapshot_idle_'+baseline])
                    failed = []
                    for case in range(38):
                        a = s.percentile([r['engine_ms'] for r in idle_records if r['case_id'] == case])
                        z = s.percentile([r['engine_ms'] for r in recs if r['case_id'] == case])
                        if not (z-a <= 2 if a < 1 else z <= a*2):
                            failed.append(dict(case_id=case,idle_ms=a,load_ms=z,delta_ms=z-a))
                    record['read_fails'][baseline] = failed
                output['pairs'].append(record)
                text.append(f'| {engine}/{batch}/r{pair} | {row["commits"]}/{control["commits"]} | '
                            f'{control["ops_s"]:.3f} → {row["ops_s"]:.3f} | {ratio*100:.2f}% | '
                            f'{record["commit_p95"]:.3f} | {record["control_maintenance_ms"]:.3f} → {record["maintenance_ms"]:.3f} |')
                for phase in collected:
                    collected[phase].extend(s.records(phases[phase]))
                for raw in [path, control_path]:
                    output['raw'].append(dict(path=raw.name, bytes=raw.stat().st_size, sha256=b.sha(raw)))
            matrix = []
            for case in range(38):
                values = {phase: [r['engine_ms'] for r in records if r['case_id'] == case]
                          for phase, records in collected.items()}
                p95 = {phase: s.percentile(samples) for phase, samples in values.items()}
                loaded = p95['snapshot_load']
                item = dict(case_id=case, query=cases[case], n=len(values['snapshot_load']),
                            idle_single_ms=p95['snapshot_idle_single'], idle_four_ms=p95['snapshot_idle_four'],
                            loaded_p50_ms=s.percentile(values['snapshot_load'], .5), loaded_p95_ms=loaded,
                            loaded_max_ms=max(values['snapshot_load']))
                for baseline in ['single', 'four']:
                    idle = item[f'idle_{baseline}_ms']
                    item[f'{baseline}_pass'] = loaded-idle <= 2 if idle < 1 else loaded <= idle*2
                    item[f'{baseline}_delta_ms'] = loaded-idle
                    item[f'{baseline}_ratio'] = loaded/idle
                matrix.append(item)
            output['matrices'][f'{engine}-{batch}'] = matrix
    text += ['', '| FDB / çift | Writer bekleyen sorgu | En uzun tek bekleme ms | Admission sayı / max ms | Release max ms |',
             '|---|---:|---:|---:|---:|']
    for row in output['pairs']:
        if row['engine'] == 'fdb':
            text.append(f'| {row["batch"]}/r{row["pair"]} | {row["writer_waited"]}/{row["queries"]} | '
                        f'{row["max_writer_wait_ms"]:.4f} | {row["admission_waited"]} / {row["max_admission_ms"]:.6f} | {row["max_release_ms"]:.4f} |')
    text += ['', '| Motor / grup / çift | 4R tabanında aşan case | 1R tabanında aşan case |',
             '|---|---|---|']
    for row in output['pairs']:
        bad = {mode:', '.join(str(r['case_id']) for r in row['read_fails'][mode]) or 'yok' for mode in ['four','single']}
        text.append(f'| {row["engine"]}/{row["batch"]}/r{row["pair"]} | {bad["four"]} | {bad["single"]} |')
    for name, matrix in output['matrices'].items():
        text += ['', f'### {name}: her sorgunun pooled dağılımı', '',
                 '| # / sorgu / offset / limit | n | Boşta 1R p95 ms | Boşta 4R p95 ms | Yük p50 / p95 / max ms | Fark 4R ms | Oran 4R | 4R / 1R kapı |',
                 '|---|---:|---:|---:|---:|---:|---:|---|']
        for row in matrix:
            case = row['query']
            query = case['text'] if len(case['text']) < 40 else '(zx)×150'
            gate = lambda passed: 'GEÇTİ' if passed else 'KALDI'
            text.append(f'| {row["case_id"]} `{query}` /{case["offset"]}/{case["limit"]} | {row["n"]} | '
                        f'{row["idle_single_ms"]:.4f} | {row["idle_four_ms"]:.4f} | '
                        f'{row["loaded_p50_ms"]:.4f} / {row["loaded_p95_ms"]:.4f} / {row["loaded_max_ms"]:.4f} | '
                        f'{row["four_delta_ms"]:+.4f} | {row["four_ratio"]:.3f}× | '
                        f'{gate(row["four_pass"])} / {gate(row["single_pass"])} |')
    b.write(b.LOGS / f'{P}-summary.json', output)
    (b.LOGS / f'{P}-tables.md').write_text('\n'.join(text)+'\n', encoding='utf-8')
    for name, matrix in output['matrices'].items():
        print(name, {mode: [r['case_id'] for r in matrix if not r[f'{mode}_pass']] for mode in ['four','single']})
    for row in output['pairs']:
        print(row['engine'],row['batch'],row['pair'],f'{row["throughput_ratio"]*100:.2f}%')


if __name__ == '__main__':
    main()
