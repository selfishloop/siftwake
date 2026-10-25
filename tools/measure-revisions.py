"""Step 1 only: existing 10k, fixed slots, same-program policy A/B + SQLite."""
import hashlib
import importlib
import json
import os
import subprocess
import threading
import time
from pathlib import Path

bench = importlib.import_module('measure-churn')
ROOT, LOGS = bench.ROOT, bench.LOGS
PREFIX = 'revisions-10k-20261025-v2'
FDB = ROOT / 'output/revisions-fdb-20261025-v2.exe'
SQLITE = ROOT / 'output/revisions-sqlite-20261025-v2.exe'


def run(label, mode, api, state, pair):
    engine = 'sqlite' if label == 'sqlite' else 'fdb'
    db = ROOT / 'data/benchmark-work' / engine
    log = LOGS / f'{PREFIX}-r{pair}-{label}-{api}-{mode}.jsonl'
    if log.exists():
        raise RuntimeError(f'Never overwrite evidence: {log}')
    env = os.environ.copy()
    env.pop('FDB_RECLAMATION_CONTROL', None)
    if label == 'control':
        env['FDB_RECLAMATION_CONTROL'] = '1'
    maxima = {'peak_retained_bytes': 0, 'samples': 0}
    phase = [dict(phase='opening', round=0)]
    rounds = {}
    done = threading.Event()

    def sample():
        while not done.is_set():
            try:
                size = sum(p.stat().st_size for p in db.iterdir() if p.is_file())
                maxima['peak_retained_bytes'] = max(maxima['peak_retained_bytes'], size)
                maxima['samples'] += 1
                at = str(phase[0]['round'])
                rounds[at] = max(rounds.get(at, 0), size)
            except (FileNotFoundError, PermissionError):
                pass  # publication/reclamation race; next sample retries
            done.wait(.005)

    began = time.monotonic()
    sampler = threading.Thread(target=sample, daemon=True)
    sampler.start()
    try:
        with log.open('w', encoding='utf-8') as out, log.with_suffix('.stderr').open('w', encoding='utf-8') as err:
            child = subprocess.Popen([str(SQLITE if engine == 'sqlite' else FDB), mode, api, str(state)],
                                     cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=err,
                                     text=True, encoding='utf-8', creationflags=subprocess.CREATE_NO_WINDOW)
            for line in child.stdout:
                out.write(line)
                out.flush()
                row = json.loads(line)
                if row['phase'] == 'churn_baseline':
                    phase[0] = dict(phase='round', round=1)
                elif row['phase'] == 'churn_round':
                    phase[0] = dict(phase='round', round=row['round'] + 1)
                    print(json.dumps(dict(label=label, pair=pair, round=row['round'], retained=row['after']['retained_bytes'],
                                          maintenance_ms=row['maintenance_ms'])), flush=True)
            code = child.wait()
    finally:
        done.set()
        sampler.join()
    result = dict(label=label, engine=engine, pair=pair, mode=mode, api=api, exit_code=code,
                  wall_s=time.monotonic()-began, disk_sampling_interval_ms=5,
                  sampled_disk=maxima, per_round_peak_retained_bytes=rounds,
                  limitation='sampled disk peak can miss sub-5ms files; oracle/matrix work included in process lifetime')
    bench.write(log.with_suffix('.process.json'), result)
    assert code == 0, result
    return result


def main():
    before = bench.protected()
    design_path = LOGS / f'{PREFIX}-design.json'
    assert not design_path.exists(), 'No overwrite/resume without slot audit'
    design = dict(baseline_commit='3e4559d', source=bench.sha(ROOT/'src/engine/revisions.rs'),
                  programs={str(p.relative_to(ROOT)): bench.sha(p) for p in [FDB, SQLITE]},
                  protected_before=before, orders=[['control','candidate','sqlite'],['sqlite','candidate','control']],
                  same_program_AB='control disables only explicit stale revision reclamation; all previous optimizations retained',
                  corpus='same existing first10000, same marker/donor workload as prior churn',
                  cache='warm OS, no cold eviction, no background isolation', completed=[])
    bench.write(design_path, design)
    for pair, order in enumerate(design['orders']):
        for label in order:
            engine = 'sqlite' if label == 'sqlite' else 'fdb'
            state = LOGS / f'{PREFIX}-r{pair}-{label}-state.json'
            bench.reset(engine)
            bench.write(LOGS/f'{PREFIX}-slot-{engine}.json',dict(label=label, pair=pair, state=str(state)))
            for mode in ['seed', 'revisions-10', 'latency-single', 'verify']:
                design['completed'].append(run(label,mode,'streamed',state,pair))
                bench.write(design_path,design)
            design['completed'].append(run(label,'latency-single','owned',state,pair))
            if label == 'candidate':
                compat = LOGS / f'{PREFIX}-r{pair}-old-reader-verify.jsonl'
                assert not compat.exists()
                with compat.open('w',encoding='utf-8') as out, compat.with_suffix('.stderr').open('w',encoding='utf-8') as err:
                    checked = subprocess.run([str(ROOT/'output/churn-fdb-20261024.exe'),'verify','streamed',str(state)],
                                             cwd=ROOT,stdout=out,stderr=err,creationflags=subprocess.CREATE_NO_WINDOW)
                assert checked.returncode == 0, 'Old reader failed'
                design['completed'].append(dict(mode='old-reader-verify',pair=pair,exit_code=checked.returncode))
            bench.write(design_path,design)
    # The requested continuous-use extension is a separate fresh candidate DB.
    # Keep paired latency/search states identical at round10 for all backends.
    state = LOGS/f'{PREFIX}-r2-candidate-state.json'
    bench.reset('fdb')
    bench.write(LOGS/f'{PREFIX}-slot-fdb.json',dict(label='candidate',pair=2,state=str(state)))
    for mode in ['seed','revisions-30','verify']:
        design['completed'].append(run('candidate',mode,'streamed',state,2))
        bench.write(design_path,design)
    design['protected_after'] = bench.protected()
    assert before == design['protected_after']
    design['complete'] = True
    bench.write(design_path, design)


if __name__ == '__main__':
    main()
