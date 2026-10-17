"""Alternate short, exclusively opened blocks on the same fixed FDB slot."""
import argparse
import ctypes
import hashlib
import json
import math
import statistics
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def sha(path):
    value = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            value.update(chunk)
    return value.hexdigest().upper()


def snapshot():
    if not any((ROOT / 'data/benchmark-work/fdb').glob('generation-*.ready')):
        raise RuntimeError('Existing published FDB slot required; never initialize a read slot')
    paths = sorted((ROOT / 'data/benchmark-work/fdb').iterdir())
    paths.append(ROOT / 'data/benchmark-work/sqlite/sqlite.db')
    paths.extend([ROOT / 'tests/fixtures/v1/checkpoint.fdb', ROOT / 'tests/fixtures/v1/checkpoint.ready'])
    return [dict(path=str(p.relative_to(ROOT)), bytes=p.stat().st_size, sha256=sha(p)) for p in paths if p.is_file()]


def write(path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False), encoding='utf-8')


def percentile(values, p):
    values = sorted(values)
    return values[math.ceil(len(values) * p / 100) - 1]


class Worker:
    def __init__(self, executable, log_path, affinity, pages_only, observe_thread=False, case_indices=None):
        self.observe_thread = observe_thread
        self.log = log_path.open('w', encoding='utf-8')
        self.stderr = log_path.with_suffix('.stderr').open('w', encoding='utf-8')
        self.process = subprocess.Popen([str(executable), *(['--pages-only'] if pages_only else []), *(['--observe-thread'] if observe_thread else []), *(['--case-indices', ','.join(map(str, case_indices))] if case_indices else [])], cwd=ROOT, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.stderr, text=True, encoding='utf-8', creationflags=subprocess.CREATE_NO_WINDOW)
        api = ctypes.WinDLL('kernel32', use_last_error=True)
        api.SetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
        api.SetProcessAffinityMask.restype = ctypes.c_int
        if not api.SetProcessAffinityMask(int(self.process._handle), affinity):
            self.close()
            raise ctypes.WinError(ctypes.get_last_error())
        api.GetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_size_t), ctypes.POINTER(ctypes.c_size_t)]
        api.GetProcessAffinityMask.restype = ctypes.c_int
        actual = ctypes.c_size_t()
        system = ctypes.c_size_t()
        if not api.GetProcessAffinityMask(int(self.process._handle), ctypes.byref(actual), ctypes.byref(system)) or actual.value != affinity:
            self.close()
            raise RuntimeError('Requested processor affinity was not applied')
        self.metadata = dict(pid=self.process.pid, affinity_mask=affinity, executable=str(executable), executable_sha256=sha(executable))
        while True:
            row = self.read()
            if row['phase'] == 'worker_ready':
                assert row['cases'] == (len(case_indices) if case_indices else 8 if pages_only else 38) and not row['diagnostic_timing']
                if case_indices:
                    assert row['case_indices'] == case_indices
                break

    def read(self):
        line = self.process.stdout.readline()
        if not line:
            raise RuntimeError(f'Worker exited early: {self.process.poll()}')
        self.log.write(line)
        self.log.flush()
        return json.loads(line)

    def block(self, command):
        self.process.stdin.write(json.dumps(command) + '\n')
        self.process.stdin.flush()
        measured = None
        while True:
            row = self.read()
            if row['phase'] == 'matrix_reads':
                assert measured is None and row['seed'] == command['seed'] and row['repeats'] == command['repeats'] and row['full_document_checks']
                measured = row
            if row['phase'] == 'thread_observations':
                assert measured is not None and row['seed'] == command['seed']
                for query, observed in zip(measured['queries'], row['queries'], strict=True):
                    assert all(query[key] == observed[key] for key in ('text', 'offset', 'limit'))
                    assert len(observed['cycles']) == len(observed['cpu_ms']) == query['timing']['n']
                    query['thread_observation'] = observed
            if row['phase'] == 'block_done':
                assert row['id'] == command['id'] and row['database_released'] and measured is not None
                if self.observe_thread:
                    assert all('thread_observation' in query for query in measured['queries'])
                return measured

    def close(self):
        if self.process.poll() is None:
            self.process.stdin.close()
            try:
                self.process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
        self.process.stdout.close()
        self.log.close()
        self.stderr.close()
        if self.process.returncode:
            raise RuntimeError(f'Worker failed: {self.process.returncode}')


def summarize(records, design):
    queries = []
    for index, query in enumerate(records['control'][0][0]['queries']):
        identity = {key: query[key] for key in ('text', 'kind', 'offset', 'limit', 'total', 'late_matches')}
        result = dict(identity)
        pair_stats = {}
        for engine, pairs in records.items():
            values = []
            pair_stats[engine] = []
            block_medians = []
            for blocks in pairs:
                per_pair = []
                medians = []
                for block in blocks:
                    row = block['queries'][index]
                    assert all(row[key] == value for key, value in identity.items())
                    timing = row['timing']
                    assert timing['n'] == len(timing['samples']) and timing['p95_ms'] == percentile(timing['samples'], 95)
                    per_pair.extend(timing['samples'])
                    medians.append(statistics.median(timing['samples']))
                values.extend(per_pair)
                pair_stats[engine].append(percentile(per_pair, 95))
                block_medians.append(medians)
            result[engine] = dict(n=len(values), p50_ms=statistics.median(values), p95_ms=percentile(values, 95), p99_ms=percentile(values, 99), per_process_pair_p95=pair_stats[engine], block_medians=block_medians)
            if 'thread_observation' in pairs[0][0]['queries'][index]:
                for metric in ('cycles', 'cpu_ms'):
                    observed = [x for blocks in pairs for block in blocks for x in block['queries'][index]['thread_observation'][metric]]
                    result[engine][metric] = dict(p50=statistics.median(observed), p95=percentile(observed, 95))
        result['change_p95_percent'] = 100 * (result['fdb']['p95_ms'] / result['control']['p95_ms'] - 1)
        result['paired_p95_change_percent'] = [100 * (a / b - 1) for a, b in zip(pair_stats['fdb'], pair_stats['control'])]
        result['faster_process_pairs'] = sum(a < b for a, b in zip(pair_stats['fdb'], pair_stats['control']))
        paired = [100 * (a / b - 1) for aa, bb in zip(result['fdb']['block_medians'], result['control']['block_medians']) for a, b in zip(aa, bb)]
        result['paired_block_median_changes_percent'] = paired
        result['median_block_change_percent'] = statistics.median(paired)
        result['faster_blocks'] = sum(value < 0 for value in paired)
        result['leave_one_pair_out_p95_changes_percent'] = []
        for omitted in range(len(records['control'])):
            values = {engine: [x for pair, blocks in enumerate(pairs) if pair != omitted for block in blocks for x in block['queries'][index]['timing']['samples']] for engine, pairs in records.items()}
            result['leave_one_pair_out_p95_changes_percent'].append(100 * (percentile(values['fdb'], 95) / percentile(values['control'], 95) - 1))
        queries.append(result)
    return dict(design=design, queries=queries, lower_p95_cases=sum(q['change_p95_percent'] < 0 for q in queries), higher_p95_cases=sum(q['change_p95_percent'] > 0 for q in queries), interpretation='Short exclusive sessions, per-block source validation/warmup outside timing; two resident oracle workers, no concurrent database queries. Not identical to a continuously open single-process matrix. Pooled p95 and block medians describe different distributions; no automatic significance claim.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prefix', required=True)
    parser.add_argument('--control', required=True)
    parser.add_argument('--candidate', required=True)
    parser.add_argument('--pairs', type=int, default=4, choices=(2, 4))
    parser.add_argument('--blocks', type=int, default=8, choices=(2, 4, 8))
    parser.add_argument('--repeats', type=int, default=25, choices=(1, 5, 10, 25))
    parser.add_argument('--affinity', type=int, default=4)
    parser.add_argument('--full-matrix', action='store_true')
    parser.add_argument('--case-indices', help='Comma-separated indices from the existing38-case oracle; selected cases run every round')
    parser.add_argument('--observe-thread', action='store_true', help='Benchmark-only calling-thread CPU/cycle counters outside the wall bracket')
    args = parser.parse_args()
    if args.case_indices is not None:
        args.case_indices = [int(n) for n in args.case_indices.split(',')]
        if not args.case_indices or len(set(args.case_indices)) != len(args.case_indices) or any(n < 0 or n >= 38 for n in args.case_indices):
            parser.error('case indices must be unique and in0..37')
    pages_only = not args.full_matrix and not args.case_indices
    prefix = ROOT / args.prefix
    report = lambda suffix: Path(f'{prefix}-{suffix}')
    if report('design.json').exists():
        raise RuntimeError('Preserve existing reports; use an unused log prefix')
    programs = {key: ROOT / value for key, value in [('control', args.control), ('fdb', args.candidate)]}
    design = dict(pairs=args.pairs, blocks_per_pair=args.blocks, repeats_per_block=args.repeats, affinity_mask=args.affinity, pages_only=pages_only, case_indices=args.case_indices, dense_selected_cases=bool(args.case_indices), cache='warm OS cache; every block opens and verifies/warmups before query timing; no eviction', corpus='same existing first10000; no generation/reset/copy', source_validation='complete source check before timed search; unchanged CRC/schema/key/version checks', same_executable=sha(programs['control']) == sha(programs['fdb']), programs={key: dict(path=str(value.relative_to(ROOT)), sha256=sha(value)) for key, value in programs.items()}, balanced_block_first_position=True, orders=[])
    design['observe_thread'] = args.observe_thread
    write(report('design.json'), design)
    before = snapshot()
    write(report('before.json'), before)
    records = {key: [] for key in programs}
    for pair in range(args.pairs):
        workers = {}
        start_order = ['control', 'fdb'] if pair % 2 == 0 else ['fdb', 'control']
        try:
            for engine in start_order:
                workers[engine] = Worker(programs[engine], report(f'{engine}-{pair}.jsonl'), args.affinity, pages_only, args.observe_thread, args.case_indices)
                records[engine].append([])
            write(report(f'pair-{pair}-workers.json'), {key: value.metadata for key, value in workers.items()})
            for block in range(args.blocks):
                order = ['control', 'fdb'] if (pair + block) % 2 == 0 else ['fdb', 'control']
                command = dict(id=pair * args.blocks + block, seed=pair * 10000 + block, repeats=args.repeats)
                design['orders'].append(dict(pair=pair, block=block, engines=order, command=command))
                for engine in order:
                    began = time.monotonic()
                    records[engine][pair].append(workers[engine].block(command))
                    print(f'pair={pair} block={block} engine={engine} wall={time.monotonic()-began:.3f}s', flush=True)
        finally:
            failures = []
            for worker in workers.values():
                try:
                    worker.close()
                except RuntimeError as error:
                    failures.append(str(error))
            if failures:
                raise RuntimeError('; '.join(failures))
    write(report('design.json'), design)
    after = snapshot()
    write(report('after.json'), after)
    assert before == after, 'FDB/index/SQLite files changed'
    summary = summarize(records, design)
    write(report('summary.json'), summary)
    print(json.dumps(dict(lower=summary['lower_p95_cases'], higher=summary['higher_p95_cases'], same_executable=design['same_executable'], files_unchanged=True)), flush=True)
    for query in summary['queries']:
        print(query['text'][:32], query['offset'], query['limit'], round(query['change_p95_percent'], 2), query['faster_process_pairs'], 'block_median', round(query['median_block_change_percent'], 2), 'fasterblocks', query['faster_blocks'], flush=True)


if __name__ == '__main__':
    main()
