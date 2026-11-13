"""Run the bounded, fixed-slot 50k live disk experiment in mirrored order."""

import argparse
import hashlib
import importlib
import json
import os
import shutil
import subprocess
import threading
import time
from datetime import datetime, timezone
from pathlib import Path


BASE = importlib.import_module('measure-churn')
ROOT = BASE.ROOT
OUT = ROOT / 'docs/benchmarks/live-disk-50k-20261111'
SLOTS = ROOT / 'data/benchmark-work'
REGISTRY = OUT.parent / 'active-benchmark-slots.json'
CORPUS = Path('D:/frunner-corpus/trwiki-100k-cap200k/corpus.payload-bench')
CATALOG = OUT.parent / 'trwiki-cap200k-20261029-v3-catalog.json'
AUDIT = OUT.parent / 'trwiki-cap200k-20261029-v3-audit.json'
FROZEN = ROOT / 'output/maintenance-candidate-cli-20261024.exe'
WRAPPER = ROOT / 'target/release/examples/repair_block3a.exe'
VERIFY_PROGRAMS = {
    'fdb': ROOT / 'target/release/examples/measure_churn.exe',
    'sqlite': ROOT / 'tools/native_sqlite/target/release/content_churn.exe',
}
PROGRAMS = {engine: path.with_name('live_disk50_timed.exe') for engine, path in VERIFY_PROGRAMS.items()}
ORDER = ('off', 'sqlite', 'on', 'on', 'sqlite', 'off')
MAX_SLOT_BYTES = 8 * 2**30
MIN_FREE_BYTES = 15 * 2**30
START = time.monotonic()
MAX_SECONDS = 120 * 60


def utc():
    return datetime.now(timezone.utc).isoformat()


def read(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))


def rows(path, phase):
    with path.open(encoding='utf-8') as stream:
        for line in stream:
            row = json.loads(line)
            if row['phase'] == phase:
                yield row


def save(path, data):
    temporary = path.with_suffix('.tmp')
    temporary.write_text(json.dumps(data, indent=2, ensure_ascii=False) + '\n', encoding='utf-8', newline='\n')
    os.replace(temporary, path)


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest().upper()


def environment():
    env = {key: value for key, value in os.environ.items() if not key.startswith('FDB_')}
    env.update(FDB_BENCH_CORPUS_PATH=str(CORPUS), FDB_BENCH_CORPUS_INDEX=str(CATALOG),
               FDB_BENCH_ORIGINAL_BODIES='1', FDB_BENCH_50K_WRITE='1', FDB_SCALE_DOCUMENTS='50000',
               FDB_LIVE_DISK50='1', FDB_STREAMED_WAL_LAYER='1', FDB_SEED_IDLE_CLEANUP='1',
               FDB_RECLAMATION_CONTROL='1')
    return env


class Experiment:
    def __init__(self, registry):
        self.registry = registry
        self.active = None
        self.ledger = {'started_utc': utc(), 'order': ORDER, 'completed': [],
                       'max_seconds': MAX_SECONDS, 'max_slot_bytes': MAX_SLOT_BYTES,
                       'min_free_bytes': MIN_FREE_BYTES, 'status': 'running'}

    def event(self, kind, **fields):
        with (OUT / 'events.jsonl').open('a', encoding='utf-8') as stream:
            stream.write(json.dumps(dict(utc=utc(), kind=kind, **fields), ensure_ascii=False) + '\n')

    def slot(self, engine, label, status, model):
        info = dict(documents=50000, status=status, label=label, state=str(model))
        if model.is_file():
            info['model_sha256'] = sha(model)
            info['sequence'] = read(model)['sequence']
        self.registry['slots'][engine] = info
        save(REGISTRY, self.registry)

    def run(self, engine, label, mode, model, env, *, kill=False, monitor=False, legacy=None):
        log = OUT / f'{label}.jsonl'
        programs = VERIFY_PROGRAMS if mode == 'live-disk50-verify' else PROGRAMS
        args = ([WRAPPER, 'legacy', 'verify', legacy] if legacy else
                [programs[engine], mode, 'streamed', model])
        started = time.monotonic()
        started_utc = utc()
        peak = 0
        exit_code = None
        failure = None
        killed = False
        sampled_at = 0.0
        disk_log = None
        if monitor:
            disk_log = (OUT / f'{label}-disk.jsonl').open('x', encoding='utf-8')
        try:
            with log.open('x', encoding='utf-8') as stdout, log.with_suffix('.stderr').open('x', encoding='utf-8') as stderr:
                child = subprocess.Popen(args, cwd=ROOT, env=env, stdout=stdout, stderr=stderr,
                                         creationflags=subprocess.CREATE_NO_WINDOW)
                self.event('process_start', label=label, mode=mode, pid=child.pid)
                try:
                    while child.poll() is None:
                        now = time.monotonic()
                        if now - START >= MAX_SECONDS:
                            raise TimeoutError('120 minute experiment window expired')
                        if disk_log:
                            retained = 0
                            with os.scandir(SLOTS / engine) as files:
                                for file in files:
                                    try:
                                        if file.is_file(follow_symlinks=False):
                                            retained += file.stat(follow_symlinks=False).st_size
                                    except FileNotFoundError:
                                        pass
                            disk_log.write(json.dumps(dict(ms=(now - started)*1000, bytes=retained)) + '\n')
                            peak = max(peak, retained)
                            if peak >= MAX_SLOT_BYTES:
                                raise RuntimeError('8 GiB benchmark slot safety ceiling reached')
                        if now - sampled_at >= 1:
                            sampled_at = now
                            if shutil.disk_usage(SLOTS).free < MIN_FREE_BYTES:
                                raise RuntimeError('benchmark workspace free space fell below 15 GiB')
                        if kill and log.stat().st_size:
                            contents = log.read_text(encoding='utf-8')
                            if contents.endswith('\n') and '"phase":"live_disk50_kill_ready"' in contents:
                                child.kill()
                                killed = True
                                break
                        time.sleep(.05 if monitor or kill else .025)
                    exit_code = child.wait(timeout=15)
                    if (kill and (not killed or exit_code == 0)) or (not kill and exit_code != 0):
                        raise RuntimeError(f'{label} exit {exit_code}; inspect {log.with_suffix(".stderr").name}')
                finally:
                    if child.poll() is None:
                        child.kill()
                        exit_code = child.wait(timeout=15)
        except BaseException as exc:
            failure = repr(exc)
            raise
        finally:
            if disk_log:
                disk_log.close()
            result = dict(engine=engine, label=label, mode=mode, log=log.name, exit_code=exit_code,
                          wall_s=time.monotonic()-started, started_utc=started_utc, ended_utc=utc(),
                          observed_peak_bytes=peak, killed_at_ack=killed, failure=failure)
            save(log.with_suffix('.process.json'), result)
            self.event('process_exit', **result)
        return log

    def warm(self, engine):
        buffer = bytearray(1024 * 1024)
        for path in sorted((SLOTS / engine).iterdir()):
            if path.is_file():
                with path.open('rb', buffering=0) as stream:
                    while stream.readinto(buffer):
                        pass

    def checkpoint(self, engine, label, model, env):
        self.warm(engine)
        samples = []
        for n in range(10):
            log = self.run(engine, f'{label}-open-{n:02}', 'live-disk50-first', model, env)
            sample, = rows(log, 'live_disk50_first')
            samples.append(sample)
        assert len({(r['sequence'], r['digest']) for r in samples}) == 1
        return samples

    def arm(self, number, arm, resume_seed=False):
        engine = 'sqlite' if arm == 'sqlite' else 'fdb'
        label = f'{number}-{arm}'
        model = OUT / f'{label}-model.json'
        self.active = (engine, label, model)
        env = environment()
        self.slot(engine, label, 'measuring_live_disk50', model)
        if resume_seed:
            self.event('reuse_verified_clean_seed', engine=engine, label=label)
            seed_check = self.run(engine, f'{label}-seed-reopen-oracle', 'live-disk50-verify', model, env)
            witness, = rows(seed_check, 'live_disk50_verified')
            assert witness['oracle'] and witness['sequence'] == 100
        else:
            self.event('fixed_slot_reset', engine=engine, label=label)
            BASE.reset(engine)
            self.run(engine, f'{label}-seed', 'scale-seed', model, env)
        if arm == 'on':
            env.pop('FDB_RECLAMATION_CONTROL')
        rerun = '-verified-seed' if resume_seed else ''
        openings = {0: self.checkpoint(engine, f'{label}{rerun}-r00', model, env)}
        raw_rounds = []
        peaks = []
        baseline = None
        for first in (0, 10, 20):
            end = first + 10
            env.update(FDB_LIVE_DISK_FIRST=str(first), FDB_LIVE_DISK_END=str(end))
            tag = f'{label}{rerun}-churn-{first+1:02}-{end:02}'
            log = self.run(engine, tag, 'live-disk50-churn', model, env, monitor=True)
            peaks.append(read(log.with_suffix('.process.json'))['observed_peak_bytes'])
            if first == 0:
                baseline, = rows(log, 'live_disk50_baseline')
            group = list(rows(log, 'live_disk50_round'))
            assert [r['round'] for r in group] == list(range(first + 1, end + 1))
            assert all(len(r['commits']) == 100 for r in group)
            raw_rounds.extend(group)
            openings[end] = self.checkpoint(engine, f'{label}-r{end:02}', model, env)
            print(f'{label}: {end}/30 rounds, {group[-1]["after"]["retained_bytes"]:,} bytes', flush=True)
        initial = baseline['disk']['retained_bytes']
        ratios = [r['after']['retained_bytes'] / initial for r in raw_rounds]
        result = dict(label=label, arm=arm, engine=engine, initial_bytes=initial, ratios=ratios,
                      final_ratio=ratios[-1], max_ratio=max(ratios), sampled_peak_bytes=max(peaks),
                      openings=openings, disk_gate=all(r['after']['retained_bytes'] * 10 <= initial * 13
                                                      for r in raw_rounds))
        save(OUT / f'{label}-performance.json', result)
        self.run(engine, f'{label}-kill', 'live-disk50-kill', model, env, kill=True)
        self.verify_arm(engine, label, result, model, env)

    def verify_arm(self, engine, label, result, model, env):
        log = self.run(engine, f'{label}-reopen-oracle-bounded', 'live-disk50-verify', model, env)
        proof, = rows(log, 'live_disk50_verified')
        result['reopen'] = proof
        if engine == 'fdb':
            config = OUT / f'{label}-legacy-config.json'
            save(config, dict(engine='fdb', state=str(model), donors='', setup_sequence=0,
                              expected_versions=None, before_delete=''))
            old = self.run(engine, f'{label}-legacy', 'legacy', model, env, legacy=config)
            get, = rows(old, 'main_get_oracle')
            assert get['documents'] == 50000 and get['sequence'] == proof['sequence'] and get['oracle']
            cases = list(rows(old, 'case_digest'))
            assert len(cases) == 41 and all(case['oracle'] for case in cases)
            actual = list(rows(log, 'live_disk50_case'))
            assert [case['actual_crc32'] for case in cases] == [case['digest'] for case in actual]
            result['frozen_reader'] = True
        self.slot(engine, label, 'ready', model)
        self.ledger['completed'].append({key: result[key] for key in result if key not in ('openings', 'ratios')})
        save(OUT / f'{label}-performance.json', result)
        save(OUT / 'status.json', self.ledger)
        self.active = None


def main():
    global START
    parser = argparse.ArgumentParser()
    resume = parser.add_mutually_exclusive_group()
    resume.add_argument('--resume-first-clean', action='store_true')
    resume.add_argument('--resume-first-oracle', action='store_true')
    args = parser.parse_args()
    assert REGISTRY.is_file()
    registry = read(REGISTRY)
    assert set(registry['slots']) == {'fdb', 'sqlite'}
    assert all(s['documents'] == 50000 for s in registry['slots'].values())
    assert registry['slots']['sqlite']['status'] == 'ready'
    resuming = args.resume_first_clean or args.resume_first_oracle
    assert registry['slots']['fdb']['status'] == ('blocked_partial_live_disk50' if resuming else 'ready')
    assert (SLOTS / '.fdb-benchmark-owned').read_text(encoding='utf-8-sig').strip() == 'FDB benchmark workspace v1'
    for path in (REGISTRY, FROZEN, WRAPPER, CATALOG, CORPUS, *PROGRAMS.values(), *VERIFY_PROGRAMS.values()):
        assert path.is_file(), path
    assert shutil.disk_usage(SLOTS).free > MIN_FREE_BYTES + MAX_SLOT_BYTES
    audit = read(AUDIT)
    assert sha(CORPUS) == audit['payload_sha256'] == registry['sha256']
    assert sha(CATALOG) == audit['catalog_sha256']
    assert sha(FROZEN) == 'B7845966E330624407D66416225F9395ED20272DD0065BAA996E4CC26D93D939'
    protected = BASE.protected()
    exp = Experiment(registry)
    if resuming:
        prior = read(OUT / 'status.json')
        assert prior['status'] == 'stopped' and prior['failed_arm'] == '1-off' and not prior['completed']
        state = registry['slots']['fdb']
        model = OUT / '1-off-model.json'
        assert Path(state['state']) == model and sha(model) == state['model_sha256']
        assert read(model)['sequence'] == state['sequence']
        if args.resume_first_clean:
            failed = OUT / '1-off-churn-01-10.jsonl'
            assert '(2048..=200_000)' in failed.with_suffix('.stderr').read_text(encoding='utf-8')
            assert not list(rows(failed, 'live_disk50_round'))
            assert {r['sequence'] for r in rows(failed, 'live_disk50_read')} == {100}
            assert state['sequence'] == 100
            assert not any(p.name.startswith('input-') or '.build-' in p.name for p in (SLOTS / 'fdb').iterdir())
            assert not (OUT / 'initial-failure.json').exists()
            save(OUT / 'initial-failure.json', prior)
            exp.event('resume_after_short_corpus_body_harness_fix', failed_process=failed.name)
        else:
            assert '1-off-reopen-oracle exit' in prior['error']
            assert len(read(OUT / '1-off-performance.json')['ratios']) == 30
            assert read(OUT / '1-off-kill.process.json')['killed_at_ack'] and state['sequence'] == 3101
            assert prior['programs'] == {key: sha(path) for key, path in PROGRAMS.items()}
            assert not (OUT / 'unbounded-oracle-stop.json').exists()
            save(OUT / 'unbounded-oracle-stop.json', prior)
            exp.event('resume_same_measured_slot_with_bounded_marker_oracle')
        exp.ledger['started_utc'] = prior['started_utc']
        START -= (datetime.now(timezone.utc) - datetime.fromisoformat(prior['started_utc'])).total_seconds()
    else:
        OUT.mkdir(exist_ok=False)
    exp.ledger.update(corpus_sha256=audit['payload_sha256'], catalog_sha256=audit['catalog_sha256'],
                      programs={key: sha(path) for key, path in PROGRAMS.items()},
                      verification_programs={key: sha(path) for key, path in VERIFY_PROGRAMS.items()},
                      frozen_sha256=sha(FROZEN),
                      seed_settings='both FDB arms seed with reclaim off; streamed WAL layer opt-in for both',
                      disk_gate='retained/S0 <= 1.3 after every round; S0 is the initial maintained retained bytes',
                      source_sha256={str(p.relative_to(ROOT)):sha(p) for p in sorted((ROOT/'src').rglob('*.rs'))})
    save(OUT / 'status.json', exp.ledger)
    try:
        for n, arm in enumerate(ORDER, 1):
            if args.resume_first_oracle and n == 1:
                label = '1-off'
                exp.active = ('fdb', label, model)
                exp.slot('fdb', label, 'measuring_live_disk50', model)
                exp.verify_arm('fdb', label, read(OUT / f'{label}-performance.json'), model, environment())
            else:
                exp.arm(n, arm, resume_seed=args.resume_first_clean and n == 1)
        exp.ledger['status'] = 'complete'
    except BaseException as exc:
        exp.ledger.update(status='stopped', error=repr(exc), failed_arm=exp.active[1] if exp.active else None)
        if exp.active:
            engine, label, model = exp.active
            exp.slot(engine, label, 'blocked_partial_live_disk50', model)
        raise
    finally:
        exp.ledger.update(ended_utc=utc(), protected_unchanged=BASE.protected() == protected,
                          corpus_unchanged=sha(CORPUS) == audit['payload_sha256'])
        save(OUT / 'status.json', exp.ledger)


if __name__ == '__main__':
    main()
