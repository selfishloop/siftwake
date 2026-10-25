"""Fixed-slot churn/concurrency measurement; never generates a document corpus."""
import argparse
import hashlib
import json
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LOGS = ROOT / 'docs/benchmarks'
PREFIX = 'churn-concurrent-10k-20261024'
PROGRAMS = {'fdb': ROOT / 'output/churn-fdb-20261024.exe',
            'sqlite': ROOT / 'output/churn-sqlite-20261024.exe'}


def sha(path):
    value = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            value.update(block)
    return value.hexdigest().upper()


def write(path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False), encoding='utf-8')


def protected():
    return [{'path': str(p.relative_to(ROOT)), 'bytes': p.stat().st_size, 'sha256': sha(p)}
            for p in [ROOT / 'data/rkyv-corpus-20261009/json.payload-bench',
                      ROOT / 'tests/fixtures/v1/checkpoint.fdb', ROOT / 'tests/fixtures/v1/checkpoint.ready']]


def reset(engine):
    subprocess.run(['powershell.exe', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command',
                    f". ./tools/benchmark-workspace.ps1; Reset-BenchmarkDatabase {engine}"],
                   cwd=ROOT, check=True, creationflags=subprocess.CREATE_NO_WINDOW)


def run(engine, mode, api, state, label):
    log = LOGS / f'{PREFIX}-{label}-{engine}-{api}-{mode}.jsonl'
    if log.exists():
        process_log=log.with_suffix('.process.json')
        if process_log.exists():
            recorded=json.loads(process_log.read_text(encoding='utf-8'))
            if recorded['exit_code']==0:
                print(f"resume completed {log.name}",flush=True)
                return recorded
        raise RuntimeError(f'Incomplete/failed run needs investigation, never overwrite: {log.name}')
    started = time.monotonic()
    with log.open('w', encoding='utf-8') as output, log.with_suffix('.stderr').open('w', encoding='utf-8') as error:
        child = subprocess.run([str(PROGRAMS[engine]), mode, api, str(state)], cwd=ROOT,
                               stdout=output, stderr=error, creationflags=subprocess.CREATE_NO_WINDOW)
    result = dict(engine=engine, mode=mode, api=api, log=log.name, exit_code=child.returncode,
                  wall_s=time.monotonic() - started)
    write(log.with_suffix('.process.json'), result)
    print(json.dumps(result), flush=True)
    if child.returncode:
        raise RuntimeError(f'Worker failed; retain all evidence: {log.name}')
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--section', choices=('latency', 'churn', 'concurrent', 'owned', 'all'), default='all')
    parser.add_argument('--pairs', type=int, choices=(1, 2), default=2)
    parser.add_argument('--resume', action='store_true')
    args = parser.parse_args()
    sections = ['latency', 'churn', 'concurrent', 'owned'] if args.section == 'all' else [args.section]
    manifest = LOGS / f'{PREFIX}-{args.section}-design.json'
    if manifest.exists() and not args.resume:
        raise RuntimeError('Existing design: do not rerun completed workloads')
    before = protected()
    design = dict(sections=sections, pairs=args.pairs, programs={e: {'path': str(p.relative_to(ROOT)), 'sha256': sha(p)} for e, p in PROGRAMS.items()},
                  corpus='same existing first10000; bodies cap51140 UTF8bytes plus unique revision marker<=51200; metadata index only',
                  cache='warm OS cache; no eviction or power/background isolation',
                  source='unchanged normal FDB strict/synced engine; SQLite FTS5trigram detailnone FULLWAL cache2MiB mmap0 tempFILE',
                  concurrency='same external Mutex per backend; one reader,one writer; full payload oracle outside mutex; small model capture overhead separately recorded',
                  sample_counts='latency32/12/6/3 per type at batch1/10/100/1000; matrix5 samples/case; concurrent10/case; two balanced engine pairs',
                  memory='Windows private/WS plus OS lifetime peaks;10ms phase sample may miss spikes; harness descriptor model included',
                  orders=[], completed=[], protected_before=before)
    if args.resume and manifest.exists():
        prior=json.loads(manifest.read_text(encoding='utf-8'))
        assert prior['programs']==design['programs'] and prior['protected_before']==before
        design=prior
    write(manifest, design)
    for section in sections:
        for pair in range(args.pairs):
            order = ['fdb', 'sqlite'] if pair % 2 == 0 else ['sqlite', 'fdb']
            design['orders'].append(dict(section=section, pair=pair, engines=order))
            write(manifest, design)
            for engine in order:
                labels = ['one', '100'] if section == 'concurrent' else ['main']
                for sub in labels:
                    label = f'{section}-r{pair}-{sub}'
                    state = LOGS / f'{PREFIX}-{label}-{engine}-state.json'
                    api = 'owned' if section == 'owned' else 'streamed'
                    if section in ('latency', 'owned'):
                        modes = ['idle', 'latency', 'delta']
                        if section == 'owned':
                            modes += ['burst', 'dirty', 'open']
                    elif section == 'churn':
                        modes = ['idle', 'churn', 'burst', 'dirty', 'open']
                    else:
                        modes = [f'concurrent-{sub}', 'verify']
                    process_files=[LOGS/f'{PREFIX}-{label}-{engine}-{api}-{m}.process.json' for m in modes]
                    if all(p.exists() and json.loads(p.read_text())['exit_code']==0 for p in process_files):
                        print(f'resume complete section {label}/{engine}',flush=True)
                        continue
                    seed_result=LOGS/f'{PREFIX}-{label}-{engine}-streamed-seed.process.json'
                    pointer=LOGS/f'{PREFIX}-slot-{engine}.json'
                    if seed_result.exists():
                        assert args.resume and pointer.exists()
                        assert json.loads(pointer.read_text())['label']==label,'Fixed DB belongs to a later measurement; cannot replay an older partial stage'
                    else:
                        reset(engine)
                        write(pointer,dict(label=label,state=str(state)))
                    design['completed'].append(run(engine, 'seed', 'streamed', state, label))
                    for mode in modes:
                        design['completed'].append(run(engine, mode, api, state, label))
                        write(manifest, design)
                    write(LOGS / f'{PREFIX}-{label}-{engine}-final-disk.json',
                          [{'name': p.name, 'bytes': p.stat().st_size} for p in (ROOT / f'data/benchmark-work/{engine}').iterdir() if p.is_file()])
    design['protected_after'] = protected()
    assert design['protected_after'] == before
    design['protected_unchanged'] = True
    write(manifest, design)


if __name__ == '__main__':
    main()
