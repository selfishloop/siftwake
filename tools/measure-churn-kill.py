"""Process-kill coverage on the final fixed FDB slot; source files never edited."""
import ctypes
import json
import queue
import subprocess
import threading
import time
import zlib
from pathlib import Path

import importlib
bench = importlib.import_module('measure-churn')
ROOT = bench.ROOT
LOGS = bench.LOGS
DB = ROOT / 'data/benchmark-work/fdb'
PREFIX = 'churn-kill-10k-20261025'


def snapshot():
    files = {}
    for path in DB.iterdir():
        try:
            if path.is_file():
                files[path.name] = path.stat().st_size
        except FileNotFoundError:
            # A pending publication/build file can disappear between directory
            # enumeration and stat. It is not evidence of a failed engine write.
            continue
    return files


def published_sources():
    """Read the latest sealed manifest, independent of delayed stdout delivery."""
    ready=sorted(DB.glob('generation-*.ready'))
    if not ready:
        return None
    path=ready[-1]
    data=path.read_bytes()
    if data[:8]!=b'FDBMAN03' or len(data)<24:
        return None
    length=int.from_bytes(data[16:20],'little')
    if length+24!=len(data) or zlib.crc32(data[:-4])!=int.from_bytes(data[-4:],'little'):
        return None
    manifest=json.loads(data[20:20+length])
    names=[f"{'run' if source['kind']=='Run' else 'generation'}-{source['id']:016x}.fdb" for source in manifest['runs']]
    names.append(f"generation-{manifest['active']['id']:016x}.fdb")
    return dict(manifest=path.name,source_names=names)


def pending_marker_oracle(state, stage):
    pending_file=Path(f'{state}.pending')
    if not pending_file.exists():
        return dict(checks=0,reason='cache repair has no new mutation group')
    model=json.loads(state.read_text(encoding='utf-8'))
    changes=json.loads(pending_file.read_text(encoding='utf-8'))
    cli=ROOT/'output/maintenance-candidate-cli-20261024.exe'
    child=subprocess.Popen([str(cli),str(DB),'--stdio'],cwd=ROOT,stdin=subprocess.PIPE,
                           stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,encoding='utf-8',
                           creationflags=subprocess.CREATE_NO_WINDOW)
    checked=[]
    try:
        for change in changes:
            actual=model['rows'].get(str(change['id']))
            for name in ['old','new']:
                row=change[name]
                if row is None:
                    continue
                token=f"zqv{change['id']:010}r{row['revision']:010}zq"
                request=dict(op='search',query=dict(scope='shared',collection='texts',text=token,limit=200))
                child.stdin.write(json.dumps(request)+'\n');child.stdin.flush()
                response=json.loads(child.stdout.readline())
                assert response['ok'],response
                expected=int(actual is not None and actual['revision']==row['revision'])
                assert response['result']['total']==expected,(stage,change['id'],name,response)
                if expected:
                    assert response['result']['hits'][0]['document']['key']['id']==f"{change['id']:010}"
                checked.append(dict(id=change['id'],revision=row['revision'],variant=name,expected=expected))
        child.stdin.close()
        assert child.wait(timeout=20)==0
    finally:
        if child.poll() is None:
            child.kill();child.wait()
    bench.write(LOGS/f'{PREFIX}-{stage}-marker-oracle.json',dict(checks=checked,all_passed=True))
    return dict(checks=len(checked),all_passed=True)


def kill_stage(stage, state):
    before = snapshot()
    source_hash_before={p.name:bench.sha(p) for p in DB.iterdir() if p.suffix in ('.fdb','.ready')}
    removed_cache = None
    if stage == 'cache':
        # Only an existing derived main index, never .fdb/.ready/manifest.
        choices = [p for p in DB.glob('run-*.idx') if len(p.stem) == 20]
        chosen = max(choices, key=lambda p: p.stat().st_size)
        assert chosen.resolve().parent == DB.resolve() and chosen.suffix == '.idx'
        removed_cache = dict(name=chosen.name, bytes=chosen.stat().st_size, sha256=bench.sha(chosen))
        chosen.unlink()
    mode = 'open' if stage == 'cache' else ('kill-revisions' if stage == 'revisions' else f'kill-{stage}')
    log = LOGS / f'{PREFIX}-{stage}.jsonl'
    if log.exists():
        raise RuntimeError('Do not overwrite a kill attempt')
    pending = queue.Queue()
    rows = []
    with log.open('w', encoding='utf-8') as out, log.with_suffix('.stderr').open('w', encoding='utf-8') as error:
        child = subprocess.Popen([str(bench.PROGRAMS['fdb']), mode, 'owned', str(state)],
                                 cwd=ROOT, stdout=subprocess.PIPE, stderr=error, text=True,
                                 encoding='utf-8', creationflags=subprocess.CREATE_NO_WINDOW)

        def drain():
            for line in child.stdout:
                out.write(line)
                out.flush()
                row = json.loads(line)
                rows.append(row)
                pending.put(row)

        reader = threading.Thread(target=drain, daemon=True)
        reader.start()
        began = time.monotonic()
        phase = False
        witness = None
        coverage = False
        suspended_witness=None
        while time.monotonic() - began < 30 and child.poll() is None:
            while not pending.empty():
                row = pending.get_nowait()
                if stage == 'iterator' and row['phase'] == 'kill_iterator_barrier':
                    phase = True
                    witness = row
                if stage == 'flush' and row['phase'] == 'flush_start':
                    phase = True
                if stage in ('merge', 'revisions') and row['phase'] == 'merge_scheduled':
                    phase = row['stats']['compaction_running'] is True
            files = snapshot()
            building = [n for n in files if f'.build-{child.pid}-' in n]
            if stage == 'cache' and building:
                phase = True
                witness = dict(temporary_files={n: files[n] for n in building}, cache=removed_cache)
            if stage in ('flush', 'merge', 'revisions') and phase:
                new_sources = {n: size for n, size in files.items() if n not in before and n.endswith('.fdb')}
                if building or (stage == 'flush' and new_sources):
                    witness = dict(new_sources=new_sources, temporary_files={n: files[n] for n in building})
            if phase and witness is not None:
                # Stop all child threads before recording the observed file state.
                nt = ctypes.WinDLL('ntdll')
                nt.NtSuspendProcess.argtypes = [ctypes.c_void_p]
                nt.NtSuspendProcess.restype = ctypes.c_long
                assert nt.NtSuspendProcess(int(child._handle)) == 0
                suspended_files = snapshot()
                coverage = stage == 'iterator' or any(f'.build-{child.pid}-' in n for n in suspended_files)
                # Flush can be interrupted while its new source is being written,
                # before index-builder construction. This is an observed window.
                if stage == 'flush' and not coverage:
                    publication=published_sources()
                    candidates=[n for n in suspended_files if n not in before and n.startswith('run-') and n.endswith('.fdb')]
                    unsealed=[]
                    read_errors=[]
                    for name in candidates:
                        try:
                            with (DB/name).open('rb') as source:
                                if suspended_files[name]>=8:
                                    source.seek(-8,2)
                                if source.read()!=b'FDBDONE1':
                                    unsealed.append(name)
                        except OSError as error:
                            read_errors.append(dict(file=name,error=repr(error)))
                    unpublished=[n for n in candidates if publication is not None and n not in publication['source_names']]
                    coverage=bool(unsealed or unpublished)
                    suspended_witness=dict(files=suspended_files,latest_publication=publication,
                                           unsealed_new_sources=unsealed,unpublished_new_sources=unpublished,
                                           source_read_errors=read_errors,
                                           proof='unsealed checkpoint footer or absent from checked latest manifest; stdout queue timing is not proof')
                else:
                    suspended_witness=dict(files=suspended_files,proof='iterator barrier or process-owned live builder temporary file')
                child.kill()
                child.wait(timeout=10)
                break
            time.sleep(0.001)
        if child.poll() is None:
            child.kill()
            child.wait(timeout=10)
        reader.join(timeout=5)
    result = dict(stage=stage, pid=child.pid, exit_code=child.returncode,
                  elapsed_s=time.monotonic() - began, coverage_observed=coverage,
                  witness=witness, removed_derived_cache=removed_cache,
                  suspended_witness=suspended_witness,
                  scope='iterator barrier is deterministic; flush/merge/cache are externally observed temporary-file windows plus process suspension, not internal instruction hooks',
                  before_files=before, after_kill_files=snapshot(), acknowledged_rows=[r for r in rows if r['phase'] == 'kill_acknowledged'])
    bench.write(LOGS / f'{PREFIX}-{stage}-attempt.json', result)
    verify = bench.run('fdb', 'verify', 'owned', state, f'kill-{stage}')
    result['reopen_full_payload_matrix_oracle_exit_code'] = verify['exit_code']
    result['pending_old_new_marker_oracle']=pending_marker_oracle(state,stage)
    result['after_recovery_files'] = snapshot()
    result['source_hash_before']=source_hash_before
    result['source_hash_after']={p.name:bench.sha(p) for p in DB.iterdir() if p.suffix in ('.fdb','.ready')}
    if stage=='cache':
        assert result['source_hash_before']==result['source_hash_after'],'Derived cache repair changed authoritative sources'
        result['authoritative_sources_unchanged']=True
    bench.write(LOGS / f'{PREFIX}-{stage}-attempt.json', result)
    return result


def main():
    design = json.loads((LOGS / f'{bench.PREFIX}-all-design.json').read_text(encoding='utf-8'))
    if not design.get('protected_unchanged'):
        raise RuntimeError('All ordinary measurement modes must finish before kills')
    state = Path(json.loads((LOGS / f'{bench.PREFIX}-slot-fdb.json').read_text())['state'])
    protection = bench.protected()
    results = []
    for stage in ['iterator', 'flush', 'merge', 'cache']:
        result = kill_stage(stage, state)
        results.append(result)
        print(json.dumps({k: result[k] for k in ('stage', 'coverage_observed', 'reopen_full_payload_matrix_oracle_exit_code')}), flush=True)
    assert protection == bench.protected()
    bench.write(LOGS / f'{PREFIX}-summary.json', dict(results=results, protected_unchanged=True,
        power_loss_claim=False, sqlite_kill_not_measured=True))


if __name__ == '__main__':
    main()
