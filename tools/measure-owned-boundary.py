"""Post-commit CLI reads at the default delta watermark; existing first1000 only."""
import argparse
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prefix', required=True)
    parser.add_argument('--control', required=True)
    parser.add_argument('--candidate', required=True)
    args = parser.parse_args()
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
                  queries=texts, pairs=4, samples_per_process=20, warmup_per_case=1,
                  cache='warm OS cache;no eviction,CPU pinning,power or background isolation',
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
                    for batch in range(2):
                        changes = [dict(op='put', document=r['document'], guard=dict(kind='missing'))
                                   for r in documents[batch * 500:(batch + 1) * 500]]
                        result, _ = request(process, dict(op='commit', mutations=changes))
                        assert result['sequence'] == batch + 1
                    stats, _ = request(process, dict(op='stats'))
                    assert stats['documents'] == 1000 and stats['sequence'] == 2
                    assert stats['delta_keys'] == (1000 if variant == 'control' else 0)
                    timings = [[] for _ in texts]
                    for turn in range(21):
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
                        p50_ms=statistics.median(timings[i]), p95_ms=sorted(timings[i])[18])
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
        summary['queries'].append(dict(text=text, n=80, p95_ms=p95,
            change_percent=100 * (p95['candidate'] / p95['control'] - 1),
            candidate_lower_pairs=sum(a['p95_ms'] < b['p95_ms'] for a, b in zip(groups['candidate'], groups['control'], strict=True))))
    bench.write(Path(f'{prefix}-summary.json'), summary)
    print(json.dumps(summary['queries'], indent=2), flush=True)


if __name__ == '__main__':
    main()
