"""Actual engine: native JSON, positional reusable JSON control, binary overlay."""
import json
import statistics
import sys
from pathlib import Path

prefix = sys.argv[1] if len(sys.argv) > 1 else 'docs/benchmarks/engine-binary-20261011'
expected = {'common:0':100000, 'common:50':100000, 'common:99950':100000,
            'rare:0':264, 'rare:50':264, 'short:0':100000, 'short:50':100000,
            'and:0':16537, 'and:50':16537}
variants = {}
for variant in ['native', 'json', 'binary']:
    runs = []
    for run in range(3):
        name = f'{prefix}-{variant}-{run}'
        rows = [json.loads(line) for line in Path(name+'.jsonl').read_text(encoding='utf-8-sig').splitlines() if line.strip()]
        memory = json.loads(Path(name+'.memory.json').read_text(encoding='utf-8-sig'))
        assert memory['exit_code'] == 0 and memory['variant'] == variant
        opening = next(r for r in rows if r['phase'] == 'open')
        assert opening['overlay'] == variant
        assert opening['stats']['documents'] == 100000
        assert opening['stats']['resident_documents'] == 0 and opening['stats']['segments'] == 1
        queries = {f"{r['query']}:{r['offset']}":r for r in rows if r['phase']=='query'}
        assert set(queries) == set(expected)
        for key, row in queries.items():
            assert row['api']=='Database::search' and row['full_page_oracle_verified'] and not row['profiled']
            assert row['total']==expected[key]
        runs.append({'opening':opening,'queries':queries,'memory':memory})
    variants[variant] = {'runs':runs,'median':{
        'open_ms':statistics.median(r['opening']['open_ms'] for r in runs),
        'attach_ms':statistics.median(r['opening']['attach_ms'] for r in runs),
        'total_ready_ms':statistics.median(r['opening']['total_ready_ms'] for r in runs),
        'overlay_bytes':statistics.median(r['opening']['overlay_bytes'] for r in runs),
        'peak_working_set_bytes':statistics.median(r['memory']['peak_working_set_bytes'] for r in runs),
        'sampled_peak_private_bytes':statistics.median(r['memory']['sampled_peak_private_bytes'] for r in runs),
        'queries_ms':{q:statistics.median(r['queries'][q]['p50_ms'] for r in runs) for q in expected},
    }}
for run in range(3):
    native = variants['native']['runs'][run]
    for variant in ['json', 'binary']:
        other = variants[variant]['runs'][run]
        assert native['memory']['database'] == other['memory']['database']
        assert native['opening']['stats'] == other['opening']['stats']
        for key in expected:
            assert all(native['queries'][key][k] == other['queries'][key][k] for k in ['text','total','offset','cache','api'])
physical_root = Path(variants['native']['runs'][0]['memory']['database'])
physical_files = {p.name:p.stat().st_size for p in physical_root.iterdir() if p.suffix in ['.fdb','.idx','.ready']}
physical_base = sum(physical_files.values())
for variant in variants.values():
    variant['median']['total_physical_db_bytes'] = physical_base + variant['median']['overlay_bytes']
comparisons = {}
for baseline in ['native', 'json']:
    before, after = [variants[v]['median'] for v in [baseline, 'binary']]
    ratios = {q:[variants['binary']['runs'][r]['queries'][q]['p50_ms']/variants[baseline]['runs'][r]['queries'][q]['p50_ms'] for r in range(3)] for q in expected}
    comparisons[baseline] = {
        'all_pages_repeatably_faster':all(x < 1 for ratios_q in ratios.values() for x in ratios_q),
        'paired_change_percent':{q:[100*(x-1) for x in xs] for q, xs in ratios.items()},
        'working_set_no_regression':after['peak_working_set_bytes'] <= before['peak_working_set_bytes'],
        'sampled_private_no_regression':after['sampled_peak_private_bytes'] <= before['sampled_peak_private_bytes'],
        'disk_no_regression':after['total_physical_db_bytes'] <= before['total_physical_db_bytes'],
    }
gates = comparisons['native']
accepted = all(gates[k] for k in ['all_pages_repeatably_faster','working_set_no_regression','sampled_private_no_regression','disk_no_regression'])
print(json.dumps({'variants':variants, 'comparisons':comparisons, 'original_physical_files':physical_files, 'measured_search_calls':sum(sum(len(q['samples_ms']) for q in run['queries'].values())+4 for v in variants.values() for run in v['runs']),
                 'scope':'actual Database::search; native SEG03 + optional disposable payload overlay; source JSON retained',
                 'decision':'passed_engine_overlay_gate' if accepted else 'rejected_engine_overlay_gate',
                 'default_enabled':False,'persistent_format_changed':False},indent=2))
