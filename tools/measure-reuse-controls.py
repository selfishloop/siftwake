"""Read-only same-process off/off/on controls for an archived reuse probe executable.

No build, corpus creation, database copy or reset. Timing and allocation are
separate diagnostic executables; their query timings are not normal speed data.
"""
import argparse
import ctypes
import importlib.util
import itertools
import json
import statistics
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('blocks', ROOT / 'tools/measure-interleaved-reads.py')
blocks = importlib.util.module_from_spec(spec)
spec.loader.exec_module(blocks)


def commands(process, repeats, indices, pattern, get_interleave):
    permutations = list(itertools.permutations(range(3)))
    for cycle in range(6):
        order = permutations[(cycle + process) % 6]
        for position, label in enumerate(order):
            yield dict(id=process * 1000 + cycle * 3 + position,
                       seed=process * 10000 + cycle, repeats=repeats,
                       label=label, reuse=label == 2, pattern=pattern,
                       case_indices=indices, get_interleave=get_interleave), cycle


class Worker:
    def __init__(self, executable, path, affinity, phase):
        self.log = path.open('x', encoding='utf-8')
        self.stderr = path.with_suffix('.stderr').open('x', encoding='utf-8')
        self.process = subprocess.Popen([str(executable)], cwd=ROOT,
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.stderr,
            text=True, encoding='utf-8', creationflags=subprocess.CREATE_NO_WINDOW)
        try:
            api = ctypes.WinDLL('kernel32', use_last_error=True)
            api.SetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
            api.SetProcessAffinityMask.restype = ctypes.c_int
            api.GetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_size_t), ctypes.POINTER(ctypes.c_size_t)]
            api.GetProcessAffinityMask.restype = ctypes.c_int
            actual, system = ctypes.c_size_t(), ctypes.c_size_t()
            if not api.SetProcessAffinityMask(int(self.process._handle), affinity):
                raise ctypes.WinError(ctypes.get_last_error())
            if not api.GetProcessAffinityMask(int(self.process._handle), ctypes.byref(actual), ctypes.byref(system)) or actual.value != affinity:
                raise RuntimeError('affinity not applied')
            self.metadata = dict(pid=self.process.pid, affinity_mask=actual.value,
                executable=str(executable.relative_to(ROOT)), executable_sha256=blocks.sha(executable))
            while True:
                row = self.read()
                if row['phase'] == 'worker_ready':
                    assert row['cases'] == 38
                    assert row['diagnostic_timing'] == (phase == 'timing')
                    assert row['diagnostic_memory'] == (phase == 'allocation')
                    break
        except BaseException:
            self.close()
            raise

    def read(self):
        line = self.process.stdout.readline()
        if not line:
            raise RuntimeError(f'worker exited: {self.process.poll()}')
        self.log.write(line)
        self.log.flush()
        return json.loads(line)

    def block(self, command):
        self.process.stdin.write(json.dumps(command) + '\n')
        self.process.stdin.flush()
        measured = None
        while True:
            row = self.read()
            if row['phase'] == 'reuse_block':
                assert measured is None and row['full_document_checks']
                assert all(row[k] == command[k] for k in ('id', 'seed', 'repeats', 'label', 'reuse', 'pattern', 'get_interleave'))
                assert [q['case_index'] for q in row['queries']] == sorted(command['case_indices'])
                for query in row['queries']:
                    assert query['timing']['n'] == command['repeats'] == len(query['timing']['samples'])
                    assert row['order'].count(query['case_index']) == command['repeats']
                measured = row
            if row['phase'] == 'block_done':
                assert row['id'] == command['id'] and row['database_released'] and measured
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
            raise RuntimeError(f'worker failed: {self.process.returncode}')


def diagnostic_summary(records, phase):
    result = []
    for n, first in enumerate(records[0][0][0]['queries']):
        row = {k: first[k] for k in ('case_index', 'text', 'kind', 'offset', 'limit', 'total')}
        for label in range(3):
            profiles = [p for process in records[label] for block in process for p in block['queries'][n]['profiles']]
            assert profiles
            if phase == 'allocation':
                row[str(label)] = {k: dict(median=statistics.median([p[k] for p in profiles]),
                    minimum=min(p[k] for p in profiles), maximum=max(p[k] for p in profiles)) for k in profiles[0]}
            else:
                keys = sorted(set().union(*(p.keys() for p in profiles)))
                row[str(label)] = {k: dict(calls_median=statistics.median([p.get(k, {}).get('calls', 0) for p in profiles]),
                    calls_minimum=min(p.get(k, {}).get('calls', 0) for p in profiles),
                    calls_maximum=max(p.get(k, {}).get('calls', 0) for p in profiles),
                    elapsed_ms_median=statistics.median([p.get(k, {}).get('elapsed_ns', 0) / 1e6 for p in profiles])) for k in keys}
        result.append(row)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prefix', required=True)
    parser.add_argument('--executable', required=True)
    parser.add_argument('--phase', choices=('normal', 'timing', 'allocation'), default='normal')
    parser.add_argument('--processes', type=int, choices=(2, 4), default=4)
    parser.add_argument('--repeats', type=int, choices=(1, 3, 5, 10), default=5)
    parser.add_argument('--pattern', choices=('mixed', 'single'), default='mixed')
    parser.add_argument('--case-indices', default=','.join(map(str, range(38))))
    parser.add_argument('--get-interleave', action='store_true')
    parser.add_argument('--affinity', type=int, default=4)
    args = parser.parse_args()
    indices = [int(n) for n in args.case_indices.split(',')]
    if not indices or len(set(indices)) != len(indices) or any(n < 0 or n >= 38 for n in indices):
        parser.error('unique cases in 0..37 required')
    prefix = ROOT / args.prefix
    report = lambda suffix: Path(f'{prefix}-{suffix}')
    if report('design.json').exists():
        raise RuntimeError('preserve existing reports: prefix already used')
    executable = (ROOT / args.executable).resolve()
    design = dict(phase=args.phase, processes=args.processes, cycles_per_process=6,
        repeats_per_cycle=args.repeats, pattern=args.pattern, case_indices=indices,
        get_interleave=args.get_interleave, affinity_mask=args.affinity,
        labels={'0':'off0', '1':'off1 placebo', '2':'on'}, same_process=True,
        executable=str(executable.relative_to(ROOT)), executable_sha256=blocks.sha(executable),
        cache='warm OS cache, no eviction; strict source/index open and oracle warmup outside query clocks',
        corpus='existing same first10000 from json.payload-bench; no creation/reset/copy',
        source_validation='strict full open, candidate CRC/schema/UTF8/key/version; on reuses checked owned bytes at hydration',
        query_clock='Query construction plus search; full oracle check and hit disposal outside clock',
        instrumented_timings_are_normal_speed=False, orders=[], workers=[])
    blocks.write(report('design.json'), design)
    before = blocks.snapshot()
    blocks.write(report('before.json'), before)
    records = {n: [] for n in range(3)}
    for process in range(args.processes):
        worker = Worker(executable, report(f'worker-{process}.jsonl'), args.affinity, args.phase)
        design['workers'].append(worker.metadata)
        for label in records:
            records[label].append([])
        try:
            for command, cycle in commands(process, args.repeats, indices, args.pattern, args.get_interleave):
                design['orders'].append(dict(process=process, cycle=cycle, command=command))
                began = time.monotonic()
                row = worker.block(command)
                records[command['label']][process].append(row)
                print(f'process={process} cycle={cycle} label={command["label"]} wall={time.monotonic()-began:.3f}s', flush=True)
        finally:
            worker.close()
            blocks.write(report('design.json'), design)
    after = blocks.snapshot()
    blocks.write(report('after.json'), after)
    assert before == after, 'source/index/SQLite/fixture changed'
    summary = dict(design=design, files_unchanged=True, comparisons={})
    for label, name in [(1, 'off1_vs_off0'), (2, 'on_vs_off0')]:
        summary['comparisons'][name] = blocks.summarize({'control':records[0], 'fdb':records[label]}, design)
    summary['comparisons']['on_vs_off1'] = blocks.summarize({'control':records[1], 'fdb':records[2]}, design)
    for value in summary['comparisons'].values():
        value['interpretation'] = 'One resident oracle worker, same process/executable, three balanced conditions; strict open/warmup outside query clocks. Pooled p95 and typical paired block medians are different statistics; no subtraction of placebo percentages or automatic significance claim.'
    if args.phase != 'normal':
        summary['diagnostics'] = diagnostic_summary(records, args.phase)
    blocks.write(report('summary.json'), summary)
    print(json.dumps({name:dict(lower=value['lower_p95_cases'], higher=value['higher_p95_cases']) for name, value in summary['comparisons'].items()}), flush=True)
    for query in summary['comparisons']['on_vs_off0']['queries']:
        print(query['text'][:24], query['offset'], query['limit'], round(query['change_p95_percent'], 2), 'processes', query['faster_process_pairs'], 'block', round(query['median_block_change_percent'], 2), flush=True)


if __name__ == '__main__':
    main()
