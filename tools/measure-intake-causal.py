"""Same executable, two test-only intake modes;only the existing first1k."""
import argparse
import importlib
import json
import math
import os
import statistics
import subprocess
from pathlib import Path

bench = importlib.import_module('measure-write-stages')
boundary = importlib.import_module('measure-owned-boundary')
ROOT = bench.ROOT


def p95(values):
    return sorted(values)[math.ceil(.95 * len(values)) - 1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prefix', required=True)
    parser.add_argument('--test-executable', required=True)
    parser.add_argument('--modes', nargs='+', choices=('scalar', 'fast', 'rounded'),
                        default=['scalar', 'fast', 'rounded'])
    args = parser.parse_args()
    prefix = ROOT / args.prefix
    report = lambda suffix: Path(f'{prefix}-{suffix}')
    if prefix.parent != ROOT / 'docs/benchmarks' or report('design.json').exists():
        raise RuntimeError('Unused docs/benchmarks prefix required')
    program = ROOT / args.test_executable
    assert len(args.modes) in (2, 3) and len(set(args.modes)) == len(args.modes) and 'scalar' in args.modes
    design = dict(program=str(program.relative_to(ROOT)), sha256=bench.sha(program),
                  same_executable=True, pairs=4, modes=args.modes, affinity_mask=4, affinity_verified=True,
                  source_sha256={p: bench.sha(ROOT / p) for p in ['src/index.rs', 'src/index/intake_fold_tests.rs', 'tools/measure-intake-causal.py']},
                  orders=[],
                  corpus='same existing first1000 of permitted first10k;two500-row synced owned commits;no corpus generation',
                  control='test-only scalar/fast intake switch;one compiled search and serialization path;not a production runtime option',
                  timing='native search,page-to-Value,response wrapper,Value Display to fresh Vec separately;one warmup+200 samples/case/process;checks/CRC/drops outside brackets',
                  validation='independent total/key/version/page oracle;all encoded lengths/CRCs cross-mode identical;not an independent full-payload oracle',
                  cache='warm OS cache;no eviction or power/background isolation;query/serialization interleaved as in CLI preparation',
                  limits='release test binary,not normal release speed;no pipe/IPC/system-I/O in these timings;no RAM claim',
                  storage='only owned fixed FDB slot reset;no DB copies;restore10k after this probe')
    bench.write(report('design.json'), design)
    before = bench.protected()
    bench.write(report('before.json'), dict(protected=before, core=bench.core('fdb')))
    documents = boundary.corpus()
    texts = ['the', 'quixote', 'he', 'the man', 'zx', 'transubstantiation']
    bodies = [boundary.fold(r['document']['title'] + '\n' + r['document']['text']) for r in documents]
    expected = []
    for text in texts:
        hits = [r for r, body in zip(documents, bodies, strict=True) if all(term in body for term in text.split())]
        expected.append(dict(total=len(hits), keys_versions=[[r['document']['key'], r['version']] for r in hits[:50]]))
    rows = []
    reference_files = None
    for pair in range(4):
        if len(args.modes) == 2:
            order = args.modes if pair % 2 == 0 else args.modes[::-1]
        else:
            permutations = [[0, 1, 2], [2, 1, 0], [1, 0, 2], [2, 0, 1]]
            order = [args.modes[n] for n in permutations[pair]]
        design['orders'].append(order)
        bench.write(report('design.json'), design)
        for mode in order:
            subprocess.run(['powershell.exe', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command',
                            '. ./tools/benchmark-workspace.ps1; Reset-BenchmarkDatabase fdb'], cwd=ROOT,
                           check=True, capture_output=True, creationflags=subprocess.CREATE_NO_WINDOW)
            env = dict(os.environ, FDB_INTAKE_MODE=mode, FDB_INTAKE_SEED=str(pair))
            with report(f'{pair}-{mode}.log').open('w', encoding='utf-8') as log:
                process = subprocess.Popen([str(program), '--ignored', '--exact',
                                           'index::intake_fold_tests::existing_corpus_postwrite_control',
                                           '--nocapture', '--test-threads=1'], cwd=ROOT, env=env,
                                           stdout=log, stderr=subprocess.STDOUT, creationflags=subprocess.CREATE_NO_WINDOW)
                try:
                    boundary.set_affinity(process, 4)
                    if process.wait() != 0:
                        raise RuntimeError(f'{mode} failed;inspect log')
                finally:
                    if process.poll() is None:
                        process.kill()
                        process.wait()
            line = next(s for s in report(f'{pair}-{mode}.log').read_text(encoding='utf-8').splitlines() if '"phase":"intake_causal"' in s)
            row = json.loads(line[line.index('{'):])
            assert row['mode'] == mode and row['queries'] == texts and row['repeats'] == 200
            assert (row['stats']['documents'], row['stats']['delta_keys'], row['stats']['sequence']) == (1000, 1000, 2)
            for actual, wanted in zip(row['checks'], expected, strict=True):
                assert actual['total'] == wanted['total'] and actual['keys_versions'] == wanted['keys_versions']
            assert all(len(samples) == 200 for group in row['samples_ms'] for samples in group)
            if rows:
                assert row['checks'] == rows[0]['checks'], 'Serialized results differ'
            files = bench.core('fdb')
            if reference_files is None:
                reference_files = files
            assert files == reference_files, 'Writer source bytes differ'
            rows.append(dict(pair=pair, **row))
            bench.write(report('records.json'), rows)
            print(f'pair={pair} mode={mode};native/Value/format complete;page oracle and source SHA equal', flush=True)
    assert bench.protected() == before
    assert bench.sha(program) == design['sha256']
    summary = dict(design=design, protected_unchanged=True, writer_core_byte_equal=True, queries=[])
    for index, text in enumerate(texts):
        entry = dict(text=text, stages={})
        for stage, name in enumerate(rows[0]['stages']):
            groups = {mode: [r['samples_ms'][stage][index] for r in rows if r['mode'] == mode] for mode in args.modes}
            pooled = {mode: [x for group in groups[mode] for x in group] for mode in groups}
            points = {mode: dict(n=len(values), p50_ms=statistics.median(values), p95_ms=p95(values)) for mode, values in pooled.items()}
            comparisons = {mode: dict(change_percent=100 * (points[mode]['p95_ms'] / points['scalar']['p95_ms'] - 1),
                                      faster_pairs=sum(p95(a) < p95(b) for a, b in zip(groups[mode], groups['scalar'], strict=True)))
                           for mode in args.modes if mode != 'scalar'}
            entry['stages'][name] = dict(points=points, comparisons=comparisons)
        summary['queries'].append(entry)
    bench.write(report('summary.json'), summary)
    for query in summary['queries']:
        print(query['text'], {k: {m: (round(c['change_percent'], 2), c['faster_pairs']) for m, c in v['comparisons'].items()}
                              for k, v in query['stages'].items()}, flush=True)


if __name__ == '__main__':
    main()
