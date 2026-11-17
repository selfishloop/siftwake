"""Verify the rejected read-only prototype and restored engine; never reset data."""
import importlib.util
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REPORTS = ROOT / 'docs/benchmarks'


def read(name):
    return json.loads((REPORTS / name).read_text(encoding='utf-8'))


def main():
    target = REPORTS / 'payload-batch-final-proof-20261017.json'
    if target.exists():
        raise RuntimeError('Preserve existing proof')
    spec = importlib.util.spec_from_file_location('stages', ROOT / 'tools/measure-search-stages.py')
    stages = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(stages)
    snapshot = stages.snapshot()
    snapshots = []
    programs = {}
    for phase in ['profile', 'ab', 'memory', 'pages', 'pages-aa']:
        stem = f'payload-batch-{phase}-10k-20261017'
        before, after = read(stem + '-before.json'), read(stem + '-after.json')
        assert before == after == snapshot
        summary = read(stem + '-summary.json')
        snapshots.append(dict(phase=phase, files=len(after), unchanged=True))
        for program in summary['design']['programs'].values():
            assert stages.sha(ROOT / program['path']) == program['sha256']
            programs[program['path']] = program['sha256']
    assert read('payload-layout-10k-20261017.json')['before'] == snapshot
    corpus = ROOT / 'data/rkyv-corpus-20261009/json.payload-bench'
    corpus_sha = stages.sha(corpus)  # Streams 1MiB chunks, never loads the corpus.
    assert corpus_sha == '11F826FCE3F4AE790FEB9FC6DF9B1169F52C2FD699E89FC4A8A4B77AAA41731B'
    runtime = {}
    for name in ['src/segment.rs', 'src/segment/window_tests.rs', 'Cargo.toml', 'Cargo.lock']:
        baseline = subprocess.run(['git', 'show', '6eb3a89:' + name], cwd=ROOT,
                                  capture_output=True, check=True).stdout
        current = (ROOT / name).read_bytes()
        assert current.replace(b'\r\n', b'\n') == baseline.replace(b'\r\n', b'\n')
        if name.startswith('src/'):
            assert current == baseline
        runtime[name] = dict(sha256=stages.sha(ROOT / name), byte_exact=current == baseline,
                             content_equals_git_blob_ignoring_crlf=True)
    patch = REPORTS / 'payload-batch-20261017-prototype.patch'
    subprocess.run(['git', 'apply', '--check', str(patch)], cwd=ROOT, check=True, capture_output=True)
    profile = read('payload-batch-profile-10k-20261017-summary.json')['queries']
    counts = lambda row, name: tuple(row['counters'].get(name, {}).get(k, 0) for k in ('min_calls', 'max_calls'))
    allowed = {'disk.read_at', 'query.payload_read', 'query.payload_read_requests',
               'query.payload_read_bytes', 'query.payload_window_hits'}
    reduced = 0
    for q in profile:
        a, b = q['control'], q['fdb']
        for name in (set(a['counters']) | set(b['counters'])) - allowed:
            assert counts(a, name) == counts(b, name), (q['text'], name)
        reduced += counts(b, 'query.payload_read_requests')[0] < counts(a, 'query.payload_read_requests')[0]
    assert len(profile) == 38 and reduced == 19
    tests = {}
    for mode, expected in [('default', (180, 7)), ('reference', (107, 3)), ('all', (199, 9))]:
        log = '\n'.join((REPORTS / f'payload-batch-{mode}-{kind}-20261017.log').read_text()
                        for kind in ['tests', 'examples'])
        rows = re.findall(r'test result: ok. (\d+) passed; (\d+) failed; (\d+) ignored', log)
        passed, failed, ignored = (sum(int(row[n]) for row in rows) for n in range(3))
        assert failed == 0 and (passed, ignored) == expected
        tests[mode] = dict(passed=passed, failed=failed, ignored=ignored)
    for phase in ['clippy', 'all-clippy', 'default-build']:
        log = (REPORTS / f'payload-batch-{phase}-20261017.log').read_text()
        assert 'Finished' in log and 'error:' not in log
    assert not (REPORTS / 'payload-batch-fmt-20261017.log').read_text().strip()
    matrix = read('payload-batch-ab-10k-20261017-summary.json')['queries']
    totals = {q['text']: q['total'] for q in matrix if q['kind'] != 'page'}
    requests = [dict(op='stats')] + [dict(op='search', query=dict(scope='shared',
                collection='texts', text=text, limit=200 if text == 'the' else 50))
                for text in ['the', 'holmes', 'watson', 'darcy', 'quixote', 'zx']]
    cli = ROOT / 'target/release/siftwake.exe'
    run = subprocess.run([str(cli), 'data/benchmark-work/fdb', '--stdio'], cwd=ROOT,
                         input=''.join(json.dumps(row) + '\n' for row in requests),
                         capture_output=True, text=True, encoding='utf-8', check=True,
                         creationflags=subprocess.CREATE_NO_WINDOW)
    responses = [json.loads(line) for line in run.stdout.splitlines()]
    assert len(responses) == len(requests) and all(row['ok'] for row in responses) and not run.stderr
    assert (responses[0]['result']['documents'], responses[0]['result']['sequence']) == (10000, 20)
    queries = []
    for request, response in zip(requests[1:], responses[1:], strict=True):
        text = request['query']['text']
        page = response['result']
        assert page['total'] == totals[text] and len(page['hits']) == min(totals[text], request['query']['limit'])
        assert page['sequence'] == 20
        for hit in page['hits']:
            assert hit['version'] == int(hit['document']['key']['id']) // 500 + 1
        queries.append(dict(text=text, total=page['total'], hits=len(page['hits'])))
    assert stages.snapshot() == snapshot
    fdb = [row for row in snapshot if row['path'].startswith('data\\benchmark-work\\fdb\\')]
    assert len(fdb) == 18 and sum(row['bytes'] for row in fdb) == 349805698
    stages.write(target, dict(decision='rejected; engine restored to 6eb3a89',
                 snapshots=snapshots, files=snapshot, programs=programs,
                 corpus=dict(bytes=corpus.stat().st_size, sha256=corpus_sha, new_corpus=False),
                 restored_engine_and_unchanged_dependencies=runtime,
                 patch_sha256=stages.sha(patch), patch_replay_check=True,
                 all38_verification_work_counts_preserved=True, reduced_read_cases=reduced,
                 tests=tests, cli=dict(sha256=stages.sha(cli), queries=queries, normal_default=True),
                 data_directories=sorted(p.name for p in (ROOT / 'data').iterdir() if p.is_dir())))
    print(json.dumps(dict(proof=target.name, decision='rejected', tests=tests)), flush=True)


if __name__ == '__main__':
    main()
