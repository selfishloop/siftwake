"""Finalize this read-only iteration after all timed processes have exited."""
import importlib.util
import json
import re
import struct
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REPORTS = ROOT / 'docs/benchmarks'
PREFIXES = ['search-observed-aa', 'search-hydration-ab', 'search-hydration-dense',
            'search-hydration-dense-aa', 'search-hydration-profile',
            'search-hydration-memory', 'search-hydration-sqlite',
            'search-hydration-sqlite-memory']


def read(name):
    return json.loads((REPORTS / name).read_text(encoding='utf-8'))


def reader_identity(sha):
    old_path = ROOT / 'output/search-hydration-20261017-candidate-measure_content_reads-normal.exe'
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
    allowed = set(range(pe + 8, pe + 12))  # COFF timestamp only.
    for n in range(debug_size // 28):
        at = debug_offset + n * 28
        kind, size, address, position = struct.unpack_from('<IIII', old, at + 12)
        allowed.update(range(at + 4, at + 8))
        if kind == 2 and old[position:position + 4] == b'RSDS':
            allowed.update(range(position + 4, position + 24))  # PDB GUID/age only.
    differences = [n for n, (a, b) in enumerate(zip(old, new)) if a != b]
    assert len(old) == len(new) and all(n in allowed for n in differences)
    return dict(measured_sha256=sha(old_path), restored_sha256=sha(new_path),
                differing_bytes=len(differences), executable_code_and_non_debug_data_equal=True,
                differences_only_in_debug_identity=True)


def main():
    target = REPORTS / 'search-hydration-final-proof-20261017.json'
    if target.exists():
        raise RuntimeError('Preserve existing proof')
    spec = importlib.util.spec_from_file_location('stages', ROOT / 'tools/measure-search-stages.py')
    stages = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(stages)
    normal_reader = reader_identity(stages.sha)
    current = stages.snapshot()
    by_path = {row['path']: row for row in current}
    snapshots = []
    for name in PREFIXES:
        stem = name + '-10k-20261017'
        before, after = read(stem + '-before.json'), read(stem + '-after.json')
        assert before == after
        assert all(by_path[row['path']] == row for row in after)
        assert (REPORTS / (stem + '-summary.json')).is_file(), 'Finish all timed phases first'
        snapshots.append(dict(prefix=stem, files=len(after), unchanged=True))
    corpus = ROOT / 'data/rkyv-corpus-20261009/json.payload-bench'
    corpus_sha = stages.sha(corpus)
    assert corpus_sha == '11F826FCE3F4AE790FEB9FC6DF9B1169F52C2FD699E89FC4A8A4B77AAA41731B'
    assert (ROOT / 'src/segment.rs').read_bytes() == (ROOT / 'output/search-hydration-20261017-segment-backup.rs').read_bytes()
    changed = subprocess.run(['git', 'diff', '--name-only', 'b804de8', '--', 'src',
                              'Cargo.toml', 'Cargo.lock', 'tests/fixtures'], cwd=ROOT,
                             check=True, capture_output=True, text=True).stdout.splitlines()
    assert set(changed) == {'src/segment.rs', 'src/segment/grouped_tests.rs'}
    matrix = read('search-hydration-ab-10k-20261017-summary.json')['queries']
    comparison = read('search-hydration-sqlite-10k-20261017-summary.json')
    for variant in ('control', 'fdb'):
        program = comparison['design']['programs'][variant]
        assert stages.sha(ROOT / program['path']) == program['sha256']
        for pair in range(4):
            rows = [json.loads(line) for line in (REPORTS / f'search-hydration-sqlite-10k-20261017-{variant}-{pair}.jsonl').read_text().splitlines()]
            oracle = next(row for row in rows if row['phase'] == 'matrix_oracle')
            measured = next(row for row in rows if row['phase'] == 'matrix_reads')
            assert oracle['source_crc_verified'] and oracle['records'] == 10000 and oracle['text_bytes'] == 264842858
            assert measured['full_document_checks'] and measured['seed'] == pair and measured['repeats'] == 20
            assert len(measured['queries']) == 38
            assert all(q['timing']['n'] == len(q['timing']['samples']) == (5 if q['kind'] == 'page' else 20) for q in measured['queries'])
    profile = read('search-hydration-profile-10k-20261017-summary.json')['queries']
    counts = lambda row, name: tuple(row['counters'].get(name, {}).get(k, 0) for k in ('min_calls', 'max_calls'))
    for query, measured in zip(matrix, profile, strict=True):
        assert all(query[k] == measured[k] for k in ('text', 'offset', 'limit'))
        hits = min(query['limit'], max(0, query['total'] - query['offset']))
        grouped_hits = hits if query['offset'] + query['limit'] <= 4096 else 0
        names = set(measured['control']['counters']) | set(measured['fdb']['counters'])
        for name in names - {'query.record_metadata', 'query.record_identity'}:
            assert counts(measured['control'], name) == counts(measured['fdb'], name), (query['text'], name)
        for name, difference in [('query.record_metadata', -grouped_hits), ('query.record_identity', grouped_hits)]:
            assert tuple(b - a for a, b in zip(counts(measured['control'], name), counts(measured['fdb'], name))) == (difference, difference)
    tests = {}
    for mode, expected in [('default', (180, 7)), ('reference', (107, 3)), ('all', (199, 9))]:
        log = (REPORTS / f'search-hydration-{mode}-tests-20261017.log').read_text()
        rows = re.findall(r'test result: ok. (\d+) passed; (\d+) failed; (\d+) ignored', log)
        passed, failed, ignored = (sum(int(row[n]) for row in rows) for n in range(3))
        assert failed == 0 and (passed, ignored) == expected
        tests[mode] = dict(passed=passed, failed=failed, ignored=ignored)
    for name in ['clippy', 'all-clippy', 'default-build', 'sqlite-build']:
        log = (REPORTS / f'search-hydration-{name}-20261017.log').read_text()
        assert 'Finished' in log and 'error:' not in log
    assert not (REPORTS / 'search-hydration-fmt-20261017.log').read_text().strip()
    requests = [dict(op='stats')] + [dict(op='search', query=dict(scope='shared', collection='texts', text=text, limit=limit)) for text, limit in [('the', 200), ('quixote', 50), ('zx', 50)]]
    cli = ROOT / 'target/release/fdb.exe'
    run = subprocess.run([str(cli), 'data/benchmark-work/fdb', '--stdio'], cwd=ROOT,
                         input=''.join(json.dumps(row) + '\n' for row in requests),
                         capture_output=True, text=True, encoding='utf-8', check=True,
                         creationflags=subprocess.CREATE_NO_WINDOW)
    responses = [json.loads(line) for line in run.stdout.splitlines()]
    assert len(responses) == 4 and all(row['ok'] for row in responses) and not run.stderr
    assert (responses[0]['result']['documents'], responses[0]['result']['sequence']) == (10000, 20)
    queries = []
    for request, response, total in zip(requests[1:], responses[1:], [10000, 56, 0], strict=True):
        page = response['result']
        assert page['total'] == total and len(page['hits']) == min(total, request['query']['limit']) and page['sequence'] == 20
        for hit in page['hits']:
            assert hit['version'] == int(hit['document']['key']['id']) // 500 + 1
        queries.append(dict(text=request['query']['text'], total=total, hits=len(page['hits'])))
    assert stages.snapshot() == current
    fdb_files = [row for row in current if row['path'].startswith('data\\benchmark-work\\fdb\\')]
    assert len(fdb_files) == 18 and sum(row['bytes'] for row in fdb_files) == 349805698
    proof = dict(snapshots=snapshots, files=current, fdb_bytes=349805698,
                 corpus=dict(bytes=corpus.stat().st_size, sha256=corpus_sha, new_corpus=False),
                 tests=tests, all38_verification_work_counts_preserved=True,
                 grouped_metadata_calls_reduced_by_returned_hits=True,
                 source_restored_exactly=True, runtime_paths_changed=changed,
                 cli=dict(sha256=stages.sha(cli), queries=queries, normal_default=True, no_mutations=True),
                 restored_normal_reader=normal_reader,
                 data_directories=sorted(p.name for p in (ROOT / 'data').iterdir() if p.is_dir()))
    target.write_text(json.dumps(proof, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
    print(json.dumps(dict(proof=target.name, unchanged=True, tests=tests)), flush=True)


if __name__ == '__main__':
    main()
