"""Read-only retention/allocation/OS-memory probes for the result buffer trial."""
import argparse
import ctypes
import importlib.util
import json
import statistics
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'tools' / filename)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--prefix', required=True)
    p.add_argument('--executable', required=True)
    p.add_argument('--phase', required=True, choices=['retention', 'memory'])
    p.add_argument('--pairs', type=int, default=4, choices=[1, 4])
    a = p.parse_args()
    shared = module('interleaved', 'measure-interleaved-reads.py')
    stages = module('stages', 'measure-search-stages.py')
    prefix = ROOT / a.prefix
    report = lambda suffix: Path(f'{prefix}-{suffix}')
    if report('design.json').exists():
        raise RuntimeError('Preserve existing reports')
    executable = ROOT / a.executable
    shared.write(report('design.json'), dict(executable=str(executable), sha256=shared.sha(executable), phase=a.phase, pairs=a.pairs, affinity=4, corpus='existing first10k; no reset, preparation or copies', cache='warm OS cache, no eviction', full_source_validation=True, independent_oracle=a.phase == 'retention', memory_scope='OS process lifetime including strict open; no oracle for memory; requested Rust heap only in separate diagnostic-memory builds'))
    before = shared.snapshot()
    shared.write(report('before.json'), before)
    rows = []
    api = ctypes.WinDLL('kernel32', use_last_error=True)
    api.SetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
    api.SetProcessAffinityMask.restype = ctypes.c_int
    api.GetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_size_t), ctypes.POINTER(ctypes.c_size_t)]
    api.GetProcessAffinityMask.restype = ctypes.c_int
    for pair in range(a.pairs):
        for reuse in ([False, True] if pair % 2 == 0 else [True, False]):
            name = 'reuse' if reuse else 'normal'
            with report(f'{name}-{pair}.jsonl').open('w', encoding='utf-8') as out, report(f'{name}-{pair}.stderr').open('w', encoding='utf-8') as err:
                flags = ['--memory-probe' if a.phase == 'memory' else '--retention-probe']
                if reuse:
                    flags.append('--reuse-results')
                process = subprocess.Popen([str(executable), *flags], cwd=ROOT, stdout=out, stderr=err, creationflags=subprocess.CREATE_NO_WINDOW)
                try:
                    if not api.SetProcessAffinityMask(int(process._handle), 4):
                        raise ctypes.WinError(ctypes.get_last_error())
                    actual, system = ctypes.c_size_t(), ctypes.c_size_t()
                    if not api.GetProcessAffinityMask(int(process._handle), ctypes.byref(actual), ctypes.byref(system)) or actual.value != 4:
                        raise RuntimeError('affinity failed')
                    if process.wait() != 0:
                        raise RuntimeError(f'{name} failed')
                    if a.phase == 'memory':
                        row = stages.memory_peaks(process)
                        row.update(pair=pair, reuse=reuse)
                        rows.append(row)
                        shared.write(report(f'{name}-{pair}.memory.json'), row)
                finally:
                    if process.poll() is None:
                        process.kill()
                        process.wait()
            data = [json.loads(line) for line in report(f'{name}-{pair}.jsonl').read_text(encoding='utf-8').splitlines()]
            if a.phase == 'memory':
                assert data[-1]['phase'] == 'query_memory_verified' and not data[-1]['oracle_loaded']
            else:
                retained = [row for row in data if row['phase'] == 'retention']
                assert len(retained) == 6 and retained[-1]['text_capacity'] == retained[-1]['result_capacity'] == 0
                assert all(row['full_document_checks'] for row in retained)
            print(f'{a.phase} pair={pair} {name} passed', flush=True)
    after = shared.snapshot()
    shared.write(report('after.json'), after)
    assert before == after
    result = dict(files_unchanged=True, rows=rows)
    if rows:
        for reuse in [False, True]:
            result['reuse' if reuse else 'normal'] = {metric: statistics.median(row[metric] for row in rows if row['reuse'] == reuse) for metric in ['peak_private_commit_bytes', 'peak_working_set_bytes']}
    shared.write(report('summary.json'), result)
    print(json.dumps(result), flush=True)


if __name__ == '__main__':
    main()
