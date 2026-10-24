"""Three normal readers, same existing 38 cases, balanced sequential blocks."""
import ctypes
import importlib
import json
import statistics
import subprocess
import sys
import time
from pathlib import Path

bench = importlib.import_module('measure-interleaved-reads')
ROOT = bench.ROOT
PREFIX = ROOT / 'docs/benchmarks/consolidated-read-10k-20261024'
PROGRAMS = {
    'baseline': ROOT / 'output/intake-fold-recovery-candidate-read-20261022.exe',
    'current': ROOT / 'output/maintenance-candidate-read-20261024.exe',
    'sqlite': ROOT / 'tools/native_sqlite/target/release/content_reads.exe',
}


def report(suffix):
    return Path(f'{PREFIX}-{suffix}')


def sqlite_block(command, log):
    with log.open('w', encoding='utf-8') as out, log.with_suffix('.stderr').open('w') as err:
        process = subprocess.Popen(
            [str(PROGRAMS['sqlite']), '--matrix', str(command['seed']),
             '--matrix-repeats', str(command['repeats']), '--matrix-all-rounds'],
            cwd=ROOT, stdout=out, stderr=err, creationflags=subprocess.CREATE_NO_WINDOW)
        api = ctypes.WinDLL('kernel32', use_last_error=True)
        api.SetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
        api.SetProcessAffinityMask.restype = ctypes.c_int
        if not api.SetProcessAffinityMask(int(process._handle), 4):
            process.kill()
            raise ctypes.WinError(ctypes.get_last_error())
        assert process.wait() == 0
    rows = [json.loads(line) for line in log.read_text(encoding='utf-8').splitlines()]
    row = next(row for row in rows if row['phase'] == 'matrix_reads')
    assert row['full_document_checks'] and row['repeats'] == command['repeats']
    return row


def main():
    if report('design.json').exists():
        raise RuntimeError('Preserve existing reports')
    design = dict(pairs=2, blocks_per_pair=2, repeats_per_block=25,
                  samples_per_case_per_engine=100, affinity_mask=4,
                  programs={k: dict(path=str(p.relative_to(ROOT)), sha256=bench.sha(p))
                            for k, p in PROGRAMS.items()},
                  cache='warm OS cache; no eviction; untimed per-case/key warmup each block',
                  corpus='same existing first10000; no generation/reset/copy',
                  oracle='same shared read_matrix.rs; all38 full pages/order/versions and256 gets; checks outside timing',
                  workload='all38 cases each round including8 pages; same seeded query/get shuffle per matching block',
                  timing='count and full owned payload result; get interleaved; no CLI JSON roundtrip',
                  source_validation='FDB strict open full source/index checks before queries; SQLite no full integrity on open',
                  process='two resident FDB oracle workers; per-block exclusive DB open; fresh SQLite process per block; no overlapping queries',
                  limits='no power/hardware isolation; two process pairs; no significance claim; query OS RAM and true cold/concurrent unmeasured',
                  orders=[])
    bench.write(report('design.json'), design)
    before = bench.snapshot()
    bench.write(report('before.json'), before)
    records = {engine: [] for engine in PROGRAMS}
    for pair in range(2):
        workers = {}
        try:
            for engine in (['baseline', 'current'] if pair == 0 else ['current', 'baseline']):
                workers[engine] = bench.Worker(PROGRAMS[engine], report(f'{engine}-{pair}.jsonl'),
                                               4, False, case_indices=list(range(38)))
            for engine in records:
                records[engine].append([])
            bench.write(report(f'pair-{pair}-workers.json'), {k: w.metadata for k, w in workers.items()})
            for block in range(2):
                order = ['baseline', 'current', 'sqlite'] if (pair + block) % 2 == 0 else ['sqlite', 'current', 'baseline']
                command = dict(id=pair*2+block, seed=pair*10000+block, repeats=25)
                design['orders'].append(dict(pair=pair, block=block, engines=order, command=command))
                bench.write(report('design.json'), design)
                for engine in order:
                    began = time.monotonic()
                    row = sqlite_block(command, report(f'sqlite-{pair}-{block}.jsonl')) if engine == 'sqlite' else workers[engine].block(command)
                    assert len(row['queries']) == 38
                    assert all(q['timing']['n'] == 25 for q in row['queries'])
                    records[engine][pair].append(row)
                    bench.write(report('records.json'), records)
                    print(f'pair={pair} block={block} engine={engine} wall={time.monotonic()-began:.2f}s;38 full-page oracles passed', flush=True)
        finally:
            for worker in workers.values():
                worker.close()
    after = bench.snapshot()
    assert before == after
    bench.write(report('after.json'), after)
    summarize(design, records)


def summarize(design, records):
    """Summarize captured measurements without starting a worker or touching a DB."""
    assert design['pairs'] == 2 and design['blocks_per_pair'] == 2
    assert design['repeats_per_block'] == 25
    assert set(records) == {'baseline', 'current', 'sqlite'}
    commands = {(row['pair'], row['block']): row['command'] for row in design['orders']}
    for pairs in records.values():
        assert len(pairs) == 2
        for pair_index, pair in enumerate(pairs):
            assert len(pair) == 2
            for block_index, block in enumerate(pair):
                assert block['seed'] == commands[pair_index, block_index]['seed']
                assert block['full_document_checks'] and block['repeats'] == 25
                assert len(block['queries']) == 38 and len(block['gets']) == 5
                assert all(q['timing']['n'] == len(q['timing']['samples']) == 25
                           for q in block['queries'])
                assert [row['bin'] for row in block['gets']] == list(range(5))
                assert all(row['timing']['n'] == len(row['timing']['samples'])
                           for row in block['gets'])
    queries = []
    gets = []
    for group, count, output in [('queries', 38, queries), ('gets', 5, gets)]:
        for index in range(count):
            initial = records['baseline'][0][0][group][index]
            identity = {k: v for k, v in initial.items() if k != 'timing'}
            result = dict(identity)
            for engine, pairs in records.items():
                samples = []
                pair_p95 = []
                medians = []
                for blocks in pairs:
                    per_pair = []
                    for block in blocks:
                        row = block[group][index]
                        assert {k: v for k, v in row.items() if k != 'timing'} == identity
                        values = row['timing']['samples']
                        samples.extend(values)
                        per_pair.extend(values)
                        medians.append(statistics.median(values))
                    pair_p95.append(bench.percentile(per_pair, 95))
                result[engine] = dict(n=len(samples), p50_ms=statistics.median(samples),
                                      p95_ms=bench.percentile(samples, 95), pair_p95_ms=pair_p95,
                                      block_median_ms=medians)
            for reference in ('baseline', 'sqlite'):
                result[f'current_vs_{reference}'] = dict(
                    p95_change_percent=100*(result['current']['p95_ms']/result[reference]['p95_ms']-1),
                    faster_pairs=sum(a < b for a, b in zip(result['current']['pair_p95_ms'], result[reference]['pair_p95_ms'])),
                    paired_p95_changes_percent=[100*(a/b-1) for a, b in zip(result['current']['pair_p95_ms'], result[reference]['pair_p95_ms'])],
                    median_block_change_percent=statistics.median(100*(a/b-1) for a, b in zip(result['current']['block_median_ms'], result[reference]['block_median_ms'])))
            output.append(result)
    summary = dict(design=design, files_unchanged=True, queries=queries, gets=gets,
                   lower_p95_cases={r: sum(q[f'current_vs_{r}']['p95_change_percent'] < 0 for q in queries) for r in ('baseline','sqlite')})
    bench.write(report('summary.json'), summary)
    print(json.dumps(summary['lower_p95_cases']), flush=True)


def summarize_saved():
    design = json.loads(report('design.json').read_text(encoding='utf-8'))
    records = json.loads(report('records.json').read_text(encoding='utf-8'))
    corrections = []
    raw_logs = []
    for engine, pairs in records.items():
        for pair, blocks in enumerate(pairs):
            if engine != 'sqlite':
                path = report(f'{engine}-{pair}.jsonl')
                raw = [json.loads(line) for line in path.read_text(encoding='utf-8').splitlines()]
                raw = [row for row in raw if row['phase'] == 'matrix_reads']
                assert raw == blocks, f'Raw FDB record mismatch: {path}'
                raw_logs.append(dict(path=str(path.relative_to(ROOT)), sha256=bench.sha(path)))
                continue
            for block, captured in enumerate(blocks):
                path = report(f'sqlite-{pair}-{block}.jsonl')
                raw = [json.loads(line) for line in path.read_text(encoding='utf-8').splitlines()]
                raw = next(row for row in raw if row['phase'] == 'matrix_reads')
                recovered = json.loads(json.dumps(captured))
                for index, (saved, source) in enumerate(zip(recovered['queries'], raw['queries'], strict=True)):
                    if saved['text'] != source['text']:
                        assert saved['text'].encode('cp1252').decode('utf-8') == source['text']
                        corrections.append(dict(pair=pair, block=block, query_index=index,
                                                captured=saved['text'], utf8_source=source['text']))
                        saved['text'] = source['text']
                assert recovered == raw, f'Non-text SQLite record mismatch: {path}'
                blocks[block] = raw
                raw_logs.append(dict(path=str(path.relative_to(ROOT)), sha256=bench.sha(path)))
    assert len(corrections) == 8
    before = json.loads(report('before.json').read_text(encoding='utf-8'))
    after = json.loads(report('after.json').read_text(encoding='utf-8'))
    assert before == after == bench.snapshot()
    protected_before = ROOT / 'docs/benchmarks/consolidated-write-10k-20261024-protected-before.json'
    protected_after = ROOT / 'docs/benchmarks/consolidated-write-10k-20261024-protected-after.json'
    protected = json.loads(protected_before.read_text(encoding='utf-8-sig'))
    assert protected == json.loads(protected_after.read_text(encoding='utf-8-sig'))
    for item in protected:
        path = ROOT / item['path']
        assert path.stat().st_size == item['bytes'] and bench.sha(path) == item['sha256']
    bench.write(report('recovered-records.json'), records)
    bench.write(report('recovery-audit.json'), dict(
        cause='SQLite UTF-8 JSONL was read with Windows cp1252 by log.read_text(); only two query text fields per block were misdecoded.',
        original_records_unchanged=True, original_records_sha256=bench.sha(report('records.json')),
        recovered_records_sha256=bench.sha(report('recovered-records.json')), corrections=corrections,
        raw_logs=raw_logs, all_other_fields_equal=True, saved_and_live_manifests_equal=True,
        protected_corpus_and_v01_fixtures_equal=True,
        query_rows=456, samples_per_case_per_engine=100,
        method='Read raw JSONL explicitly as UTF-8; assert each changed text reverses cp1252 misdecoding; deep-compare every other field and timing; retain original records. No benchmark rerun.'))
    summarize(design, records)


if __name__ == '__main__':
    if sys.argv[1:] == ['--summarize-only']:
        summarize_saved()
    elif sys.argv[1:]:
        raise SystemExit('Usage: measure-consolidated-reads.py [--summarize-only]')
    else:
        main()
