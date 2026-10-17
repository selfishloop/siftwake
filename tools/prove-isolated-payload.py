"""Finalize the rejected isolated-reader trial after builds/tests finish."""
import importlib.util
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REPORTS = ROOT / 'docs/benchmarks'
STEM = 'payload-isolated-focus-10k-20261017'


def read(name):
    return json.loads((REPORTS / name).read_text(encoding='utf-8'))


def main():
    target = REPORTS / 'payload-isolated-final-proof-20261017.json'
    if target.exists():
        raise RuntimeError('Preserve the existing proof')
    spec = importlib.util.spec_from_file_location('stages', ROOT / 'tools/measure-search-stages.py')
    stages = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(stages)
    before, after = read(STEM + '-before.json'), read(STEM + '-after.json')
    assert before == after == stages.snapshot()
    summary = read(STEM + '-summary.json')
    design = summary['design']
    assert (design['pairs'], design['blocks_per_pair'], design['repeats_per_block']) == (4, 4, 25)
    assert design['case_indices'] == [8, 11, 31] and not design['observe_thread']
    assert design['dense_selected_cases'] and not design['same_executable']
    identities = [('darcy', 0, 50), ('holmes', 0, 50), ('the', 0, 200)]
    for variant, program in design['programs'].items():
        assert stages.sha(ROOT / program['path']) == program['sha256']
        for pair in range(4):
            rows = [json.loads(line) for line in (REPORTS / f'{STEM}-{variant}-{pair}.jsonl').read_text().splitlines()]
            oracle = next(row for row in rows if row['phase'] == 'matrix_oracle')
            assert oracle['records'] == 10000 and oracle['text_bytes'] == 264842858 and oracle['source_crc_verified']
            matrices = [row for row in rows if row['phase'] == 'matrix_reads']
            assert len(matrices) == 4
            assert sum(row['phase'] == 'block_done' and row['database_released'] for row in rows) == 4
            assert all(row['pending_source_checks'] == 0 for row in rows if row['phase'] == 'block_opened')
            for matrix in matrices:
                assert matrix['full_document_checks'] and matrix['repeats'] == 25
                assert [(q['text'], q['offset'], q['limit']) for q in matrix['queries']] == identities
                assert all(q['timing']['n'] == len(q['timing']['samples']) == 25 for q in matrix['queries'])
    assert all(q[v]['n'] == 400 for q in summary['queries'] for v in ['control', 'fdb'])
    runtime = {}
    for name in ['src/segment.rs', 'src/segment/window_tests.rs']:
        baseline = subprocess.run(['git', 'show', 'f6d18dd:' + name], cwd=ROOT, check=True, capture_output=True).stdout
        assert (ROOT / name).read_bytes() == baseline
        runtime[name] = stages.sha(ROOT / name)
    changed = subprocess.run(['git', 'diff', '--name-only', 'f6d18dd', '--', 'src',
                              'Cargo.toml', 'Cargo.lock', 'tests/fixtures'], cwd=ROOT,
                             check=True, capture_output=True, text=True).stdout
    assert not changed.strip()
    patch = REPORTS / 'payload-isolated-20261017-prototype.patch'
    subprocess.run(['git', 'apply', '--check', str(patch)], cwd=ROOT, check=True, capture_output=True)
    backup = ROOT / 'output/payload-isolated-20261017-segment-backup.rs'
    structure = read('payload-isolated-structure-20261017.json')
    assert stages.sha(backup).lower() == structure['candidate_sha256']
    assert structure['original_functions_and_hot_loops_byte_identical_after_removing_dispatches']
    log = (REPORTS / 'payload-isolated-default-tests-20261017.log').read_text()
    rows = re.findall(r'test result: ok. (\d+) passed; (\d+) failed; (\d+) ignored', log)
    passed, failed, ignored = (sum(int(row[n]) for row in rows) for n in range(3))
    assert (passed, failed, ignored) == (180, 0, 7)
    assert not (REPORTS / 'payload-isolated-fmt-20261017.log').read_text().strip()
    for phase in ['clippy', 'all-clippy', 'default-build']:
        log = (REPORTS / f'payload-isolated-{phase}-20261017.log').read_text()
        assert 'Finished' in log and 'error:' not in log
    assert '3 passed; 0 failed' in (REPORTS / 'payload-isolated-targeted-new-20261017.log').read_text()
    corpus = ROOT / 'data/rkyv-corpus-20261009/json.payload-bench'
    corpus_sha = stages.sha(corpus)
    assert corpus_sha == '11F826FCE3F4AE790FEB9FC6DF9B1169F52C2FD699E89FC4A8A4B77AAA41731B'
    cli = ROOT / 'target/release/fdb.exe'
    requests = [dict(op='stats')] + [dict(op='search', query=dict(scope='shared',
                collection='texts', text=text, limit=limit)) for text, _, limit in identities]
    run = subprocess.run([str(cli), 'data/benchmark-work/fdb', '--stdio'], cwd=ROOT,
                         input=''.join(json.dumps(r) + '\n' for r in requests), check=True,
                         capture_output=True, text=True, encoding='utf-8', creationflags=subprocess.CREATE_NO_WINDOW)
    responses = [json.loads(line) for line in run.stdout.splitlines()]
    assert len(responses) == 4 and not run.stderr and all(row['ok'] for row in responses)
    assert (responses[0]['result']['documents'], responses[0]['result']['sequence']) == (10000, 20)
    for q, response in zip(summary['queries'], responses[1:], strict=True):
        page = response['result']
        assert page['total'] == q['total'] and len(page['hits']) == min(q['limit'], q['total']) and page['sequence'] == 20
        assert all(hit['version'] == int(hit['document']['key']['id']) // 500 + 1 for hit in page['hits'])
    assert stages.snapshot() == before
    fdb = [row for row in before if row['path'].startswith('data\\benchmark-work\\fdb\\')]
    assert len(fdb) == 18 and sum(row['bytes'] for row in fdb) == 349805698
    stages.write(target, dict(decision='rejected; grouping trials closed', base='f6d18dd',
                 completed_local_date='2026-10-18', files=before, all_files_unchanged=True,
                 corpus=dict(bytes=corpus.stat().st_size, sha256=corpus_sha, new_corpus=False),
                 tests=dict(default_all_targets_passed=passed, failed=failed, ignored=ignored,
                            prototype_targeted_passed=3, fmt=True, clippy=True, all_clippy=True),
                 runtime_exact_to_base=runtime, patch_sha256=stages.sha(patch), patch_replay_check=True,
                 cases=identities, samples_per_case_per_variant=400, programs=design['programs'],
                 cli_sha256=stages.sha(cli), new_full_matrix=False, new_memory_measurement=False,
                 data_directories=sorted(p.name for p in (ROOT / 'data').iterdir() if p.is_dir())))
    print(json.dumps(dict(proof=target.name, decision='rejected', default_tests=passed)), flush=True)


if __name__ == '__main__':
    main()
