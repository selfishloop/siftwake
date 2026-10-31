"""Post-timing read-only diagnostics/recovery; never resets a DB or edits corpus."""
import importlib.util
import json
import os
import subprocess
import time
from pathlib import Path

R = Path(__file__).resolve().parent.parent
L = R / 'docs/benchmarks'
P = 'trwiki-scale-20261028-v1'
spec = importlib.util.spec_from_file_location('scale', R/'tools/measure-trwiki-scale.py')
s = importlib.util.module_from_spec(spec)
spec.loader.exec_module(s)


def main():
    output = L/f'{P}-post-verification.json'
    assert not output.exists(), 'Preserve existing evidence'
    design = json.loads((L/f'{P}-design.json').read_text(encoding='utf-8'))
    assert design.get('protected_unchanged') and design.get('provided_source_unchanged')
    state = Path(design['ingest_failure']['partial_state'])
    pointer = json.loads(s.CURRENT.read_text(encoding='utf-8'))
    assert pointer['slots']['fdb']['documents'] == 66000
    assert s.b.sha(state) == pointer['slots']['fdb']['model_sha256']
    baseline = json.loads((L/'hybrid-cancel-20261028-final-audit.json').read_text(encoding='utf-8'))['final_source_sha256']
    changed = [p for p, sha in baseline.items() if s.b.sha(R/p) != sha]
    assert not changed, changed
    before = s.sources()
    protected = s.b.protected()
    env = os.environ.copy()
    env.update(PYTHONUTF8='1', FDB_SCALE_DOCUMENTS='100000',
               FDB_BENCH_CORPUS_PATH=str(s.CORPUS), FDB_BENCH_CORPUS_INDEX=str(s.CATALOG),
               FDB_BENCH_ORIGINAL_BODIES='1', FDB_STREAMED_WAL_LAYER='1',
               FDB_RECLAMATION_CONTROL='1', FDB_FIRST_QUERY='olduğu', FDB_FIRST_VARIANT='cap-hybrid')
    # This is a failure-cause counter probe, not progression into the blocked
    # normal-performance matrix. Diagnostic mutexes perturb elapsed time.
    binary = R/'output/trwiki-scale-20261028-v4-diagnostic.exe'
    de = env.copy()
    de.update(FDB_QUERY_CASES='0,23', FDB_QUERY_VARIANTS='cap,cap-four,cap-hybrid',
              FDB_SCALE_QUERY_REPEATS='1', FDB_DIAGNOSTIC_EXPECTED_ROWS='66000')
    log = L/f'{P}-partial66000-failure-cause-diagnostic.jsonl'
    assert not log.exists()
    started = time.monotonic()
    with log.open('w', encoding='utf-8') as out, log.with_suffix('.stderr').open('w', encoding='utf-8') as err:
        code = subprocess.run([str(binary), 'scale-query-experiments', 'streamed', str(state)],
                              cwd=R, env=de, stdout=out, stderr=err,
                              creationflags=subprocess.CREATE_NO_WINDOW).returncode
    assert code == 0, log
    profile = dict(binary=str(binary.relative_to(R)), sha256=s.b.sha(binary), log=log.name,
                   wall_s=time.monotonic()-started, exit_code=code,
                   scope='Read-only failure cause on current committed 66k prefix, not 50k/100k performance acceptance; diagnostic spans overlap')
    print(json.dumps(dict(diagnostic=profile)), flush=True)
    # Kill only the DB-opening process, after it reports Pending. No source,
    # cache or mutation is manufactured to create this window.
    stderr = L/f'{P}-pending-opening-kill.stderr'
    assert not stderr.exists()
    with stderr.open('w', encoding='utf-8') as err:
        child = subprocess.Popen([str(s.PROGRAMS['fdb']), 'first-background', 'streamed', str(state)],
                                 cwd=R, env=env, stdout=subprocess.PIPE, stderr=err,
                                 text=True, encoding='utf-8', creationflags=subprocess.CREATE_NO_WINDOW)
        try:
            first = json.loads(child.stdout.readline())
            assert first['phase'] == 'session_return' and first['status'].lower() == 'pending', first
            child.kill()
            code = child.wait(timeout=10)
        finally:
            if child.poll() is None:
                child.kill()
                child.wait(timeout=10)
    assert code != 0
    kill = dict(witness=first, exit_code=code, process_kill=True,
                source_mutations=False, power_loss_claim=False)
    s.b.write(L/f'{P}-pending-opening-kill.json', kill)
    reopened = s.run('fdb', 'first-strict', state, 'partial66000-reopen-after-kill', env)
    rows = s.rows(reopened)
    assert any(x['phase']=='first_result_oracle' and x['oracle'] for x in rows)
    assert before == s.sources() and protected == s.b.protected()
    assert design['provided_source_sha256'] == s.b.sha(s.CORPUS)
    result = dict(profile=profile, pending_kill=kill, reopen=reopened,
                  full_payload_version_oracle_after_kill=True,
                  authoritative_sources_unchanged=True, protected_unchanged=True,
                  provided_source_unchanged=True, engine_source_sha256=baseline,
                  engine_source_changed=[], performance_gate_still_blocked=True,
                  long_document_cancel_not_run=True)
    s.b.write(output, result)
    print('Post-timing diagnostics and Pending kill/reopen oracle passed', flush=True)


if __name__ == '__main__':
    main()
