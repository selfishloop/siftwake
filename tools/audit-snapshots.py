"""Read-only audit of raw evidence and protected bytes; no measurements rerun."""
import importlib
import json

b = importlib.import_module('measure-churn')
P = 'snapshot-10k-20261026'


def read(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))


def main():
    protected = b.protected()
    files = []
    for engine in ['fdb', 'sqlite']:
        for batch in ['one', '100']:
            for pair in [0, 1]:
                path = b.LOGS / f'{P}-{engine}-{batch}-r{pair}-design.json'
                design = read(path)
                assert design['complete']
                assert design['protected_before'] == design['protected_after'] == protected
                assert b.sha(b.ROOT/design['program']) == design['sha256']
                for process in design['completed']:
                    assert process['exit_code'] == 0
                    assert read((b.LOGS/process['log']).with_suffix('.process.json')) == process
                    files.append(b.LOGS/process['log'])
                files.append(path)
    summary = read(b.LOGS / f'{P}-summary.json')
    for record in summary['raw']:
        assert b.sha(b.LOGS/record['path']) == record['sha256']
    kill = read(b.LOGS / 'snapshot-kill-10k-20261026-summary.json')
    assert kill['protected_unchanged'] and not kill['power_loss_claim']
    assert kill['old_reader']['exit_code'] == 0
    assert len(kill['results']) == 7
    for record in kill['results']:
        if 'mode' in record:
            assert record['oracle'] and record['verify_exit_code'] == 0
            assert record['marker_oracle']['all_passed']
        else:
            assert record['coverage_observed']
            assert record['reopen_full_payload_matrix_oracle_exit_code'] == 0
            assert record['pending_old_new_marker_oracle']['all_passed']
    files += list(b.LOGS.glob('snapshot-kill-10k-20261026-*.jsonl'))
    files.append(b.LOGS / 'snapshot-kill-10k-20261026-summary.json')
    validation = read(b.LOGS / f'{P}-validation-v2.json')
    assert len(validation) == 8 and all(r['exit_code'] == 0 for r in validation)
    files += [b.ROOT/r['log'] for r in validation]
    wait_design = read(b.LOGS/'snapshot-waits-10k-20261026-design.json')
    assert wait_design['complete']
    assert wait_design['protected_before'] == wait_design['protected_after'] == protected
    assert b.sha(b.ROOT/wait_design['program']) == wait_design['sha256']
    files.append(b.LOGS/'snapshot-waits-10k-20261026-design.json')
    for process in wait_design['completed']:
        assert process['exit_code'] == 0
        files.append(b.LOGS/process['log'])
    waits = read(b.LOGS/'snapshot-waits-10k-20261026-summary.json')
    for run in waits['runs']:
        assert b.sha(b.LOGS/run['log']) == run['sha256']
        assert run['queries'] == 760
    kill_v3 = read(b.LOGS/'snapshot-kill-v3-10k-20261026-summary.json')
    assert kill_v3['program_sha256'] == wait_design['sha256']
    assert kill_v3['protected_unchanged'] and not kill_v3['power_loss_claim']
    assert kill_v3['old_reader']['exit_code'] == 0 and len(kill_v3['results']) == 7
    for record in kill_v3['results']:
        if 'mode' in record:
            assert record['oracle'] and record['verify_exit_code'] == 0
            assert record['marker_oracle']['all_passed']
        else:
            assert record['coverage_observed'] and record['reopen_full_payload_matrix_oracle_exit_code'] == 0
            assert record['pending_old_new_marker_oracle']['all_passed']
    files += list(b.LOGS.glob('snapshot-kill-v3-10k-20261026-*.jsonl'))
    files.append(b.LOGS/'snapshot-kill-v3-10k-20261026-summary.json')
    validation_v3 = read(b.LOGS/f'{P}-validation-v3.json')
    assert len(validation_v3) == 8 and all(r['exit_code'] == 0 for r in validation_v3)
    files += [b.ROOT/r['log'] for r in validation_v3]
    result = dict(protected=protected, oracle=True, kill_windows_per_version=7, total_kill_windows=14, old_reader=True,
                  versions_with_separate_kill_windows=['v2','v3'],
                  validation_commands=8, validation_versions=['v2','v3'],power_loss_claim=False,
                  evidence=[dict(path=p.name, bytes=p.stat().st_size, sha256=b.sha(p)) for p in files])
    b.write(b.LOGS / f'{P}-audit.json', result)
    print('Audit passed:', len(files), 'evidence files, protected corpus/fixtures unchanged')


if __name__ == '__main__':
    main()
