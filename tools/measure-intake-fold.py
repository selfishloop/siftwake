"""Read-only scalar/fast normalization probe on the permitted existing first10k."""
import argparse
import ctypes
import importlib
import json
import math
import os
import statistics
import subprocess

bench = importlib.import_module('measure-write-stages')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prefix', required=True)
    parser.add_argument('--test-executable', required=True)
    args = parser.parse_args()
    prefix = bench.ROOT / args.prefix
    report = lambda suffix: prefix.with_name(prefix.name + '-' + suffix)
    if prefix.parent != bench.ROOT / 'docs/benchmarks' or report('design.json').exists():
        raise RuntimeError('Unused docs/benchmarks prefix required')
    program = bench.ROOT / args.test_executable
    design = dict(program=str(program.relative_to(bench.ROOT)), sha256=bench.sha(program),
                  processes=4, affinity_mask=4, corpus='same existing first10000;264842858 text bytes',
                  scope='fresh scalar fold versus existing fold_into with input-sized String;normal release test binary',
                  timing='200 texts/group;1 warmup and4 timed rounds/mode/group;String allocation+fold included;Vec reserve,input CRC/JSON,equality,output drop excluded',
                  cache='warm group and OS cache;alternate mode order;fresh processes;no eviction or power/background isolation',
                  limits='not DB write/query timing;capacity sums are not simultaneous RAM or process memory;no DB opened/reset/copied or corpus generated')
    bench.write(report('design.json'), design)
    before = bench.protected()
    rows = []
    api = ctypes.WinDLL('kernel32', use_last_error=True)
    api.SetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
    api.SetProcessAffinityMask.restype = ctypes.c_int
    api.GetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_size_t), ctypes.POINTER(ctypes.c_size_t)]
    api.GetProcessAffinityMask.restype = ctypes.c_int
    for run in range(4):
        env = dict(os.environ, FDB_INTAKE_PROBE_RUN=str(run))
        with report(f'{run}.log').open('w', encoding='utf-8') as log:
            process = subprocess.Popen([str(program), '--ignored', '--exact',
                                       'index::intake_fold_tests::existing_corpus_normalization_probe',
                                       '--nocapture', '--test-threads=1'], cwd=bench.ROOT, env=env,
                                       stdout=log, stderr=subprocess.STDOUT, creationflags=subprocess.CREATE_NO_WINDOW)
            try:
                if not api.SetProcessAffinityMask(int(process._handle), 4):
                    raise ctypes.WinError(ctypes.get_last_error())
                actual, system = ctypes.c_size_t(), ctypes.c_size_t()
                if not api.GetProcessAffinityMask(int(process._handle), ctypes.byref(actual), ctypes.byref(system)) or actual.value != 4:
                    raise RuntimeError('CPU affinity not applied')
                if process.wait() != 0:
                    raise RuntimeError('Normalization probe failed; inspect log')
            finally:
                if process.poll() is None:
                    process.kill()
                    process.wait()
        line = next(s for s in report(f'{run}.log').read_text(encoding='utf-8').splitlines() if '"phase":"intake_fold_probe"' in s)
        row = json.loads(line[line.index('{'):])
        assert row['byte_equal'] and row['documents'] == 10000 and row['text_bytes'] == 264842858
        row['timings'] = {}
        for mode in ('scalar', 'fast'):
            values = sorted(row[mode + '_ms'])
            assert len(values) == 200
            row['timings'][mode] = dict(total_ms=sum(values), p50_group_ms=statistics.median(values), p95_group_ms=values[math.ceil(.95 * len(values)) - 1])
        rows.append(row)
        print(f'run{run}: scalar={row["timings"]["scalar"]["total_ms"]:.2f}ms fast={row["timings"]["fast"]["total_ms"]:.2f}ms;all10000 byte-equal', flush=True)
    after = bench.protected()
    assert before == after, 'Protected source changed'
    bench.write(report('summary.json'), dict(design=design, rows=rows, protected_before=before,
                                           protected_after=after, protected_unchanged=True))


if __name__ == '__main__':
    main()
