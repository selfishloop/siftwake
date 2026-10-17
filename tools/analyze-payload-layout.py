"""Read-only planning on the existing SEG05 caches; no corpus or DB creation."""
import importlib.util
import json
import math
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / 'docs/benchmarks/payload-layout-10k-20261017.json'


def main():
    if OUT.exists():
        raise RuntimeError('Preserve existing layout evidence')
    spec = importlib.util.spec_from_file_location('stages', ROOT / 'tools/measure-search-stages.py')
    stages = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(stages)
    before = stages.snapshot()
    baseline = json.loads((ROOT / 'docs/benchmarks/search-hydration-profile-10k-20261017-summary.json').read_text())
    matrices = json.loads((ROOT / 'docs/benchmarks/search-hydration-ab-10k-20261017-summary.json').read_text())
    queries = matrices['queries']
    grams = lambda s: {s[i:i+3] for i in range(len(s)-2) if not any(c.isspace() for c in s[i:i+3])}
    # All matrix terms are already lowercase except these explicit oracle vectors.
    fold = lambda s: s.translate(str.maketrans('IİıÇçĞğÖöŞşÜü', 'iiiccggoossuu')).lower()
    terms = [fold(q['text']).split() for q in queries]
    wanted = set().union(*(grams(t) for ts in terms for t in ts))
    runs = []
    keys = set()
    for path in sorted((ROOT / 'data/benchmark-work/fdb').glob('run-*.idx')):
        if path.name.endswith('.records04.idx'):
            continue
        with path.open('rb') as f, path.open('rb') as postings:
            header = f.read(28)
            assert header[:8] == b'FDBSEG05'
            f.seek(-32, 2)
            footer = f.read(32)
            assert footer[:8] == b'FDBEND05' and zlib.crc32(footer[:28]) == struct.unpack_from('<I', footer, 28)[0]
            offset, length, checksum = struct.unpack_from('<QQI', footer, 8)
            f.seek(0)
            total = f.seek(0, 2) - 32
            assert offset + length == total
            f.seek(0)
            actual = 0
            remaining = total
            while remaining:
                chunk = f.read(min(1024 * 1024, remaining))
                assert chunk
                actual = zlib.crc32(chunk, actual)
                remaining -= len(chunk)
            assert actual == checksum
            f.seek(offset)
            assert f.read(9) == b'FDBDIC01\x01'
            count, term_count = struct.unpack('<II', f.read(8))
            def string():
                size = struct.unpack('<I', f.read(4))[0]
                assert size <= 256 * 1024 * 1024
                return f.read(size).decode('utf-8')
            records = []
            for _ in range(count):
                key = tuple(string() for _ in range(3))
                address, size, crc, version, deleted = struct.unpack('<QIIQB', f.read(25))
                assert not deleted and key[:2] == ('shared', 'texts') and key not in keys
                keys.add(key)
                records.append((address, size))
            lists = {}
            universal = []
            for _ in range(term_count):
                tag = f.read(1)[0]
                parts = tuple(string() for _ in range({0:1, 1:2, 2:4, 3:3, 4:2, 5:3}[tag]))
                if tag == 3:
                    f.read(8)
                address, entries = struct.unpack('<QI', f.read(12))
                if tag == 4 and entries == count and len(universal) < 64:
                    universal.append(parts[1])
                if tag == 4 and parts[0] == 'shared' and parts[1] in wanted:
                    postings.seek(address)
                    values = struct.unpack(f'<{entries}I', postings.read(entries * 4))
                    assert all(a < b for a, b in zip(values, values[1:])) and max(values) < count
                    lists[parts[1]] = set(values)
            assert f.tell() == offset + length
            runs.append(dict(name=path.name, records=records, lists=lists, universal=universal))
    assert len(keys) == 10000 and len(runs) == 5
    plans = [(64*1024, 42, 1), (128*1024, 4096, 5), (256*1024, 4096, 5),
             (256*1024, 16384, 10), (256*1024, 65536, 25)]
    results = []
    for n, query in enumerate(queries):
        by_run, gaps = [], []
        for run in runs:
            candidates = set(range(len(run['records'])))
            for term in terms[n]:
                for gram in grams(term):
                    candidates.intersection_update(run['lists'].get(gram, set()))
            verify = [t for t in terms[n] if len(t) != 3 and not (len(t) <= 2 and any(t in g for g in run['universal']))]
            rows = [run['records'][slot] for slot in sorted(candidates)] if verify else []
            by_run.append(rows)
            gaps.extend(b[0] - a[0] - a[1] for a, b in zip(rows, rows[1:]))
        count = sum(map(len, by_run))
        useful = sum(size for rows in by_run for _, size in rows)
        counter = baseline['queries'][n]['fdb']['counters']
        expected = counter.get('query.payload_candidates', {}).get('min_calls', 0)
        assert count == expected, (query['text'], count, expected)
        predictions = []
        for window, max_gap, overhead in plans:
            requests, read_bytes = 0, 0
            for rows in by_run:
                base = end = used = 0
                for address, size in rows:
                    new_end = address + size
                    if used and 0 <= address - end <= max_gap and new_end - base <= window and (new_end - base - used - size) * 100 <= (used + size) * overhead:
                        read_bytes += new_end - end
                        used += size
                        end = new_end
                    else:
                        requests += 1
                        read_bytes += size
                        base, end, used = address, new_end, size
            predictions.append(dict(window_bytes=window, max_gap=max_gap, max_overhead_percent=overhead,
                                    requests=requests, bytes=read_bytes, saved_exact_reads=count-requests,
                                    overhead_bytes=read_bytes-useful))
        gaps.sort()
        results.append(dict(text=query['text'], offset=query['offset'], limit=query['limit'], candidates=count,
                            useful_bytes=useful, baseline_requests=counter.get('query.payload_read_requests', {}).get('min_calls', 0),
                            gap_p50=gaps[len(gaps)//2] if gaps else None,
                            gap_p95=gaps[math.ceil(len(gaps)*.95)-1] if gaps else None,
                            gaps_at_most={str(t):sum(g<=t for g in gaps) for t in (42,1024,4096,16384,65536)},
                            predicted=predictions))
    assert stages.snapshot() == before
    stages.write(OUT, dict(before=before, after=before, queries=results,
                          scope='Offline potential, not measured runtime speed. Existing disjoint SEG05 runs, same first10k, index CRC checked; source/visibility/identity correctness is verified by the normal engine benchmark. Predicts ideal sorted-candidate windows with limited overread; no corpus/DB writes.'))
    print(json.dumps([q for q in results if q['text'] in ('holmes','darcy','watson','quixote') and q['offset']==0], ensure_ascii=False))


if __name__ == '__main__':
    main()
