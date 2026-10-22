"""Post-commit CLI reads at the default delta watermark; existing first1000 only."""
import argparse
import ctypes
import importlib
import json
import math
import statistics
import struct
import subprocess
import time
import zlib
from pathlib import Path

bench = importlib.import_module('measure-write-stages')
ROOT = bench.ROOT


def corpus():
    records = []
    with (ROOT / 'data/rkyv-corpus-20261009/json.payload-bench').open('rb') as source:
        header = source.read(20)
        assert header[:8] == b'PLOADB01' and zlib.crc32(header[:16]) == struct.unpack('<I', header[16:])[0]
        for index in range(1000):
            header = source.read(16)
            length, checksum, number, header_crc = struct.unpack('<IIII', header)
            assert number == index and zlib.crc32(header[:12]) == header_crc and length <= 4 * 1024 * 1024
            payload = source.read(length)
            assert len(payload) == length and zlib.crc32(payload) == checksum
            record = json.loads(payload)
            assert record['document']['key']['id'] == f'{index:010}' and record['version'] == index // 500 + 1
            records.append(record)
    return records


def fold(text):
    substitutions = str.maketrans('IİıÇçĞğÖöŞşÜü', 'iiiccggoossuu')
    return ''.join(character.translate(substitutions).lower() for character in text)


def request(process, value):
    encoded = (json.dumps(value, ensure_ascii=False, separators=(',', ':')) + '\n').encode('utf-8')
    start = time.perf_counter_ns()
    process.stdin.write(encoded)
    process.stdin.flush()
    response = process.stdout.readline()
    elapsed_ms = (time.perf_counter_ns() - start) / 1e6
    if not response:
        raise RuntimeError(f'CLI exited: {process.poll()}')
    result = json.loads(response)
    assert result['ok'], result
    return result['result'], elapsed_ms


def set_affinity(process, mask):
    if not mask:
        return
    api = ctypes.WinDLL('kernel32', use_last_error=True)
    api.SetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
    api.SetProcessAffinityMask.restype = ctypes.c_int
    api.GetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_size_t), ctypes.POINTER(ctypes.c_size_t)]
    api.GetProcessAffinityMask.restype = ctypes.c_int
    if not api.SetProcessAffinityMask(int(process._handle), mask):
        raise ctypes.WinError(ctypes.get_last_error())
    actual, system = ctypes.c_size_t(), ctypes.c_size_t()
    if not api.GetProcessAffinityMask(int(process._handle), ctypes.byref(actual), ctypes.byref(system)) or actual.value != mask:
        raise RuntimeError('Requested CPU affinity was not applied')


def native(args):
    prefix = ROOT / args.prefix
    report = lambda suffix: Path(f'{prefix}-{suffix}')
    if prefix.parent != ROOT / 'docs/benchmarks' or report('design.json').exists():
        raise RuntimeError('Unused docs/benchmarks prefix required')
    programs = {name: ROOT / getattr(args, name) for name in ('control', 'candidate')}
    design = dict(programs={k: dict(path=str(p.relative_to(ROOT)), sha256=bench.sha(p)) for k, p in programs.items()},
                  phase='native API/JSON attribution', pairs=4, affinity_mask=4,
                  corpus='same existing first1000 of permitted first10k;two500-row synced owned commits',
                  timing='native Database::search and serde_json::to_vec timed separately;100 samples/case/process;one warmup;CRC/checks/drops outside timing',
                  cache='warm OS cache;no eviction or power/background isolation',
                  limits='not exact CLI serializer/IPC;no RAM claim;warm expected hashes are cross-reader checks,not an independent oracle;separate CLI test supplies full-page oracle',
                  storage='only owned fixed FDB slot reset;no new corpus or DB copies')
    bench.write(report('design.json'), design)
    before = bench.protected()
    records = []
    for pair in range(4):
        for variant in (['control', 'candidate'] if pair % 2 == 0 else ['candidate', 'control']):
            subprocess.run(['powershell.exe', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command',
                            '. ./tools/benchmark-workspace.ps1; Reset-BenchmarkDatabase fdb'], cwd=ROOT,
                           check=True, capture_output=True, creationflags=subprocess.CREATE_NO_WINDOW)
            with report(f'{pair}-{variant}.jsonl').open('w', encoding='utf-8') as log:
                process = subprocess.Popen([str(programs[variant]), str(pair)], cwd=ROOT,
                                           stdout=log, stderr=subprocess.PIPE, creationflags=subprocess.CREATE_NO_WINDOW)
                try:
                    set_affinity(process, 4)
                    _, errors = process.communicate()
                    if process.returncode:
                        raise RuntimeError(errors.decode('utf-8'))
                finally:
                    if process.poll() is None:
                        process.kill()
                        process.wait()
            row = next(json.loads(s) for s in report(f'{pair}-{variant}.jsonl').read_text(encoding='utf-8').splitlines() if '"phase":"native_boundary"' in s)
            records.append(dict(pair=pair, variant=variant, **row))
            bench.write(report('records.json'), records)
            print(f'native pair={pair} {variant} complete', flush=True)
    assert before == bench.protected(), 'Protected source changed'
    assert all(r['checks'] == records[0]['checks'] for r in records), 'Reader results differ'
    summary = dict(design=design, protected_unchanged=True, results_equal=True, queries=[])
    for index, text in enumerate(records[0]['queries']):
        entry = dict(text=text)
        for stage in ('search', 'json'):
            groups = {v: [r[f'{stage}_samples_ms'][index] for r in records if r['variant'] == v] for v in programs}
            p95 = lambda values: sorted(values)[math.ceil(len(values) * .95) - 1]
            pooled = {v: p95([x for samples in group for x in samples]) for v, group in groups.items()}
            entry[stage] = dict(p95_ms=pooled, change_percent=100 * (pooled['candidate'] / pooled['control'] - 1),
                                candidate_lower_pairs=sum(p95(a) < p95(b) for a, b in zip(groups['candidate'], groups['control'], strict=True)))
        summary['queries'].append(entry)
    bench.write(report('summary.json'), summary)
    print(json.dumps(summary['queries'], indent=2), flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prefix', required=True)
    parser.add_argument('--control', required=True)
    parser.add_argument('--candidate', required=True)
    parser.add_argument('--candidate-delta-keys', type=int, choices=(0, 1000), default=0,
                        help='Expected candidate delta;1000 for an isolated change without eager flush')
    parser.add_argument('--samples', type=int, choices=(20, 100, 200), default=20)
    parser.add_argument('--native', action='store_true', help='Use dedicated native API/JSON attribution workers')
    parser.add_argument('--affinity', type=int, choices=(0, 4), default=0,
                        help='CLI process affinity mask;0 preserves the historical unpinned measurement')
    args = parser.parse_args()
    if args.native:
        native(args)
        return
    prefix = ROOT / args.prefix
    if prefix.parent != ROOT / 'docs/benchmarks' or Path(f'{prefix}-design.json').exists():
        raise RuntimeError('Unused docs/benchmarks prefix required')
    documents = corpus()
    bodies = [fold(r['document']['title'] + '\n' + r['document']['text']) for r in documents]
    texts = ['the', 'quixote', 'he', 'the man', 'zx', 'transubstantiation']
    expected = [[r for r, body in zip(documents, bodies, strict=True) if all(t in body for t in text.split())]
                for text in texts]
    programs = {name: ROOT / getattr(args, name) for name in ('control', 'candidate')}
    design = dict(programs={k: dict(path=str(p.relative_to(ROOT)), sha256=bench.sha(p)) for k, p in programs.items()},
                  corpus='same first1000 of allowed first10k;no generation/download',
                  write='two500-row default synced owned commits;intake/commits outside read timing',
                  timing='CLI request/response wall time;includes native JSON serialization+IPC;parent encode/decode+oracle excluded;not pure Rust API timing',
                  queries=texts, pairs=4, samples_per_process=args.samples, warmup_per_case=1,
                  candidate_delta_keys=args.candidate_delta_keys,
                  cache='warm OS cache;no eviction,power or background isolation',
                  affinity_mask=args.affinity, affinity_verified=bool(args.affinity),
                  memory='not measured;caller/oracle stays in separate Python process',
                  storage='only owned fixed FDB slot reset;SQLite not touched;no DB copies')
    bench.write(Path(f'{prefix}-design.json'), design)
    before = bench.protected()
    rows = []
    for pair in range(4):
        order = ['control', 'candidate'] if pair % 2 == 0 else ['candidate', 'control']
        for variant in order:
            subprocess.run(['powershell.exe', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command',
                            '. ./tools/benchmark-workspace.ps1; Reset-BenchmarkDatabase fdb'], cwd=ROOT,
                           check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, creationflags=subprocess.CREATE_NO_WINDOW)
            with Path(f'{prefix}-{pair}-{variant}.stderr').open('wb') as errors:
                process = subprocess.Popen([str(programs[variant]), 'data/benchmark-work/fdb', '--stdio'],
                                           cwd=ROOT, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                           stderr=errors, creationflags=subprocess.CREATE_NO_WINDOW)
                try:
                    set_affinity(process, args.affinity)
                    for batch in range(2):
                        changes = [dict(op='put', document=r['document'], guard=dict(kind='missing'))
                                   for r in documents[batch * 500:(batch + 1) * 500]]
                        result, _ = request(process, dict(op='commit', mutations=changes))
                        assert result['sequence'] == batch + 1
                    stats, _ = request(process, dict(op='stats'))
                    assert stats['documents'] == 1000 and stats['sequence'] == 2
                    assert stats['delta_keys'] == (1000 if variant == 'control' else args.candidate_delta_keys)
                    timings = [[] for _ in texts]
                    for turn in range(args.samples + 1):
                        indices = list(range(len(texts)))
                        if (turn + pair) % 2:
                            indices.reverse()
                        for case in indices:
                            page, elapsed = request(process, dict(op='search', query=dict(
                                scope='shared', collection='texts', text=texts[case], limit=50)))
                            assert page['sequence'] == 2 and page['total'] == len(expected[case])
                            assert page['hits'] == expected[case][:50], texts[case]
                            if turn:
                                timings[case].append(elapsed)
                    row = dict(pair=pair, variant=variant, stats=stats, queries=[dict(
                        text=text, total=len(expected[i]), samples_ms=timings[i],
                        p50_ms=statistics.median(timings[i]),
                        p95_ms=sorted(timings[i])[math.ceil(args.samples * .95) - 1])
                        for i, text in enumerate(texts)], full_page_oracle=True)
                    rows.append(row)
                    bench.write(Path(f'{prefix}-records.json'), rows)
                    print(f'pair={pair} {variant};delta={stats["delta_keys"]};6 query full-page oracles passed', flush=True)
                finally:
                    process.stdin.close()
                    try:
                        process.wait(timeout=30)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()
                        raise
                    process.stdout.close()
                assert process.returncode == 0
    assert before == bench.protected(), 'Protected corpus/fixture changed'
    summary = dict(design=design, protected_unchanged=True, queries=[])
    for index, text in enumerate(texts):
        groups = {variant: [r['queries'][index] for r in rows if r['variant'] == variant] for variant in programs}
        values = {variant: sorted(s for r in group for s in r['samples_ms']) for variant, group in groups.items()}
        p95 = {variant: group[math.ceil(len(group) * .95) - 1] for variant, group in values.items()}
        summary['queries'].append(dict(text=text, n=4 * args.samples, p95_ms=p95,
            change_percent=100 * (p95['candidate'] / p95['control'] - 1),
            candidate_lower_pairs=sum(a['p95_ms'] < b['p95_ms'] for a, b in zip(groups['candidate'], groups['control'], strict=True))))
    bench.write(Path(f'{prefix}-summary.json'), summary)
    print(json.dumps(summary['queries'], indent=2), flush=True)


if __name__ == '__main__':
    main()
