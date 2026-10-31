"""Latest-release diagnostics; keep distinct from the v2 balanced A/B evidence."""
import importlib
import json

b = importlib.import_module('measure-churn')
s = importlib.import_module('summarize-snapshots')
P = 'snapshot-waits-10k-20261026'


def main():
    output = dict(scope='v3 final-reference counter fix; two supplementary loads, no writer-only A/B', runs=[])
    lines = ['# v3 ek bekleme ölçümü', '',
             'Kontrol/yük dengeli A/B tekrarı değildir. v2 kötü sonuçları korunur.', '',
             '| Grup | Writer bekleyen sorgu | Tek bekleme max ms | Admission sayı / max ms | Release max ms | Release unlink dosya |',
             '|---|---:|---:|---:|---:|---:|']
    for batch in ['one', '100']:
        path = b.LOGS/f'{P}-{batch}-fdb-streamed-snapshot-{batch}.jsonl'
        assert json.loads(path.with_suffix('.process.json').read_text(encoding='utf-8'))['exit_code'] == 0
        phases = {r['phase']: r for r in map(json.loads, path.read_text(encoding='utf-8').splitlines())}
        load = phases['snapshot_load']
        recs = s.records(load)
        assert len(recs) == 760 and all(r['oracle'] and r['writer_active'] for r in recs)
        assert all(r['release']['cleanup_error'] is None for r in recs)
        details = load['maintenance']['details']
        assert details['normal_merges_completed'] and details['revision_rewrites_completed']
        row = dict(batch=batch, queries=len(recs),writer_waited=sum(r['writer_waited'] for r in recs),
                   max_writer_wait_ms=max(r['max_writer_wait_ns'] for r in recs)/1e6,
                   writer_wait_sum_ms=sum(r['writer_wait_ns'] for r in recs)/1e6,
                   admission_waited=sum(r['admission']['contended'] for r in recs),
                   max_admission_ms=max(r['admission']['wait_ns'] for r in recs)/1e6,
                   max_release_ms=max(r['release_ms'] for r in recs),
                   reclaimed_files=sum(r['release']['reclaimed_files'] for r in recs),
                   commits=load['commits'],ops_s_informational=load['ops_s'],memory=load['memory'],
                   maintenance=load['maintenance'],log=path.name,sha256=b.sha(path),matrix=[])
        for case in range(38):
            data = {name:[r['engine_ms'] for r in s.records(phases[name]) if r['case_id'] == case]
                    for name in ['snapshot_idle_single','snapshot_idle_four','snapshot_load']}
            assert all(len(values) == 20 for values in data.values())
            item = dict(case_id=case,load_p95=s.percentile(data['snapshot_load']),
                        load_p50=s.percentile(data['snapshot_load'],.5),load_max=max(data['snapshot_load']))
            for baseline in ['single','four']:
                idle = s.percentile(data['snapshot_idle_'+baseline]);diff=item['load_p95']-idle
                item.update({baseline+'_idle':idle,baseline+'_delta':diff,
                             baseline+'_pass':diff<=2 if idle<1 else item['load_p95']<=idle*2})
            row['matrix'].append(item)
        output['runs'].append(row)
        lines.append(f'| {batch} | {row["writer_waited"]}/760 | {row["max_writer_wait_ms"]:.4f} | '
                     f'{row["admission_waited"]} / {row["max_admission_ms"]:.6f} | {row["max_release_ms"]:.4f} | {row["reclaimed_files"]} |')
    for row in output['runs']:
        lines += ['',f'## {row["batch"]} — n20/sorgu', '',
                  '| Case | Boşta1R /4R p95 ms | Yük p50 /p95 /max ms | 4R fark ms | 4R /1R kapı |',
                  '|---|---:|---:|---:|---|']
        for r in row['matrix']:
            gate=lambda x:'GEÇTİ' if x else 'KALDI'
            lines.append(f'| {r["case_id"]} | {r["single_idle"]:.4f} / {r["four_idle"]:.4f} | '
                         f'{r["load_p50"]:.4f} / {r["load_p95"]:.4f} / {r["load_max"]:.4f} | '
                         f'{r["four_delta"]:+.4f} | {gate(r["four_pass"])} / {gate(r["single_pass"])} |')
        print(row['batch'], '4R failed',[r['case_id'] for r in row['matrix'] if not r['four_pass']])
    b.write(b.LOGS/f'{P}-summary.json', output)
    (b.LOGS/f'{P}-tables.md').write_text('\n'.join(lines)+'\n',encoding='utf-8')


if __name__ == '__main__':
    main()
