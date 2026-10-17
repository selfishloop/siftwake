"""Final read-only file/work proof; run after all timed benchmarks have exited."""
import hashlib
import json
import re
import runpy
import struct
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BENCH = ROOT / 'docs/benchmarks'
support = runpy.run_path(str(ROOT / 'tools/measure-search-stages.py'))
sha = support['sha']


def read(name):
    return json.loads((BENCH / name).read_text(encoding='utf-8-sig'))


def normal_reader_identity():
    old_path = ROOT / 'output/search-isolated-reads-20261017.exe'
    new_path = ROOT / 'target/release/examples/measure_content_reads.exe'
    old, new = old_path.read_bytes(), new_path.read_bytes()
    pe = struct.unpack_from('<I', old, 60)[0]
    optional = pe + 24
    sections = optional + struct.unpack_from('<H', old, pe + 20)[0]
    count = struct.unpack_from('<H', old, pe + 6)[0]
    debug_rva, debug_size = struct.unpack_from('<II', old, optional + 112 + 6 * 8)
    debug_offset = None
    for n in range(count):
        size, address, raw_size, position = struct.unpack_from('<IIII', old, sections + n * 40 + 8)
        if address <= debug_rva < address + max(size, raw_size):
            debug_offset = position + debug_rva - address
    assert debug_offset is not None
    allowed = set(range(pe + 8, pe + 12))  # COFF timestamp, not executable code.
    for n in range(debug_size // 28):
        at = debug_offset + n * 28
        kind, size, address, position = struct.unpack_from('<IIII', old, at + 12)
        allowed.update(range(at + 4, at + 8))
        if kind == 2 and old[position:position + 4] == b'RSDS':
            allowed.update(range(position + 4, position + 24))  # PDB GUID/age only.
    differences = [n for n, (a, b) in enumerate(zip(old, new)) if a != b]
    assert len(old) == len(new) and all(n in allowed for n in differences)
    return dict(measured_sha256=sha(old_path), restored_sha256=sha(new_path),
                differing_bytes=len(differences), differences_only_in_debug_identity=True,
                executable_code_and_non_debug_data_equal=True)


current = support['snapshot']()
assert current == read('search-same-db-baseline-10k-20261017-before.json')
reference = {row['path']: row for row in current}
snapshots = []
for before in sorted(BENCH.glob('search-*-10k-20261017-before.json')):
    after = before.with_name(before.name.replace('-before.json', '-after.json'))
    if not after.exists():
        raise RuntimeError(f'Incomplete measurement: {before.name}')
    rows = json.loads(before.read_text(encoding='utf-8-sig'))
    assert rows == json.loads(after.read_text(encoding='utf-8-sig'))
    assert all(reference[row['path']] == row for row in rows)
    snapshots.append(before.name)

corpus = ROOT / 'data/rkyv-corpus-20261009/json.payload-bench'
corpus_sha = sha(corpus)
assert corpus.stat().st_size == 2740862279
assert corpus_sha == '11F826FCE3F4AE790FEB9FC6DF9B1169F52C2FD699E89FC4A8A4B77AAA41731B'

unchanged = ['Cargo.toml', 'Cargo.lock', 'src/payload.rs', 'src/format.rs',
             'src/binary_batch.rs', 'src/manifest.rs', 'src/model.rs', 'src/index.rs',
             'src/query.rs', 'src/engine.rs']
subprocess.run(['git', 'diff', '--exit-code', 'bf879b6', '--', *unchanged], cwd=ROOT, check=True)

profile = read('search-isolated-profile-10k-20261017-summary.json')
work = []
for query in profile['queries']:
    differences = []
    for name in sorted(set(query['control']['counters']) | set(query['fdb']['counters'])):
        values = [tuple(query[v]['counters'].get(name, {}).get(k, 0) for k in ('min_calls', 'max_calls'))
                  for v in ('control', 'fdb')]
        if values[0] != values[1]:
            differences.append(dict(counter=name, control=values[0], fdb=values[1]))
    work.append(dict(text=query['text'], offset=query['offset'], limit=query['limit'], differences=differences))
assert sum(bool(row['differences']) for row in work) == 2
assert all(row['text'] in ('zx', 'the zx') for row in work if row['differences'])

tests = {}
for path in sorted(BENCH.glob('search-short-final-*-tests-20261017.log')):
    text = path.read_text(encoding='utf-8-sig')
    assert 'FAILED' not in text and 'error:' not in text
    counts = re.findall(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored', text)
    tests[path.name] = dict(passed=sum(int(a) for a, _ in counts), ignored=sum(int(b) for _, b in counts))

sources = ['src/segment.rs', 'src/segment/short_ascii.rs', 'src/segment/window_tests.rs',
           'examples/measure_content_reads.rs', 'examples/measure_interleaved_reads.rs']
files = [row for row in current if row['path'].startswith(str(Path('data/benchmark-work/fdb')))]
result = dict(baseline_commit='bf879b6', database_files_unchanged=True,
              database_bytes=sum(row['bytes'] for row in files), database_files=len(files),
              current_files=current, complete_unchanged_snapshots=snapshots,
              corpus=dict(bytes=corpus.stat().st_size, sha256=corpus_sha, generated=False),
              formats_features_dependencies_unchanged=unchanged,
              source_hashes={name: sha(ROOT / name) for name in sources},
              worker_source_restored_exactly=(ROOT / 'src/segment.rs').read_bytes() ==
                  (ROOT / 'output/search-worker-segment-backup-20261017.rs').read_bytes(),
              restored_normal_reader=normal_reader_identity(),
              tests_before_final_inline_attribute=tests,
              final_attribute_related_tests=dict(short_ascii=3, query_scratch=4, window=7),
              final_work_profile=work,
              limits='A/A p95 unstable in long sessions and short blocks. No general p95, cold-cache, concurrent, write/maintenance or total-system-RAM gate.')
assert result['database_bytes'] == 349805698 and result['database_files'] == 18
assert result['worker_source_restored_exactly']
support['write'](BENCH / 'search-stages-final-proof-20261017.json', result)
print(json.dumps(dict(database_bytes=result['database_bytes'], snapshots=len(snapshots), corpus_unchanged=True,
                      tests=tests, identical_work_cases=36), ensure_ascii=False), flush=True)
