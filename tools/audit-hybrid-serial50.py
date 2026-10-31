"""Closing integrity checks, separate from all timed acceptance arms."""
import hashlib
import json
import re
from pathlib import Path

R = Path(__file__).resolve().parent.parent
L = R / 'docs/benchmarks'
P = 'hybrid-serial50-20261030'


def read(path):
    return json.loads(path.read_text(encoding='utf-8'))


def sha(path):
    with path.open('rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest().upper()


def main():
    designs = [read(L / f'{name}-design.json') for name in [
        P, 'hybrid-serial-extra50-20261030', 'hybrid-serial-ram50-20261030']]
    assert all(d['stage'] == 'completed' for d in designs)
    source_sha = '881756DA400952500BA871F6C7BA8FF8D17E5348E5975420BC5CD90952A7CF12'
    assert sha(Path('D:/frunner-corpus/trwiki-100k-cap200k/corpus.payload-bench')) == source_sha
    for d in designs:
        assert d['source_sha256'] == d['source_after'] == source_sha
        assert d['protected_before'] == d['protected_after'] == designs[0]['protected_before']
    for f in designs[0]['protected_before']:
        assert (R / f['path']).stat().st_size == f['bytes']
        assert sha(R / f['path']) == f['sha256']
    for p in designs[0]['programs'].values():
        assert sha(Path(p['path'])) == p['sha256']
    ex = designs[1]
    assert ex['before_readonly_sources'] == ex['after_readonly_sources']
    assert sha(R / 'output/hybrid-serial-cpu-20261030.exe') == ex['normal_sha256']
    assert sha(R / 'output/hybrid-serial-diag-20261030.exe') == ex['diagnostic_sha256']
    assert sha(R / 'output/hybrid-serial-ram-20261030.exe') == designs[2]['program_sha256']
    base = read(L / 'mask-delta-20261029-start.json')['source_sha256']
    changed = [name for name, value in base.items() if sha(R / name) != value]
    allowed = {r'src\engine\snapshot.rs', r'src\engine\first.rs', r'src\engine\light.rs',
               r'src\timing.rs', r'src\segment\paged.rs', r'src\segment\term_pages.rs'}
    assert set(changed) <= allowed, changed
    assert 'Cell::new(true)' in (R / 'src/engine/first.rs').read_text(encoding='utf-8')
    recoveries = []
    for suffix in ['main-before-reset', 'final']:
        path = L / f'hybrid-serial-ram50-20261030-{suffix}-recovery.json'
        d = read(path)
        assert len(d['kills']) == 3 and d['old_reader']['oracle'] and d['live_sources_unchanged']
        assert d['protected_before'] == d['protected_after']
        recoveries.append(dict(log=path.name, kills=3, old_reader=True,
                               live_sources_unchanged=True, scope=d['scope']))
    tests = []
    for name in ['hybrid-serial-20261030-post-tests.log',
                 'hybrid-serial-20261030-post-diag-tests.log']:
        text = (L / name).read_text(encoding='utf-8')
        results = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored', text)
        assert results and all(failed == '0' for _, failed, _ in results)
        tests.append(dict(log=name, passed=sum(int(passed) for passed, _, _ in results),
                          ignored=sum(int(ignored) for _, _, ignored in results)))
    normal_events = []
    for name in [P, 'hybrid-serial-extra50-20261030', 'hybrid-serial-ram50-20261030']:
        events = [json.loads(line) for line in (L / f'{name}-lifecycle.jsonl').read_text().splitlines()]
        assert events[-1]['stage'] == 'completed'
        exits = [x for x in events if x['kind'] == 'worker_exit']
        assert exits and all(x['exit_code'] == 0 for x in exits)
        normal_events.append(dict(log=f'{name}-lifecycle.jsonl', children=len(exits),
                                  all_normal_exits=True))
    result = dict(source_sha256=source_sha, protected=designs[0]['protected_before'],
                  source_changes_since_pre_mask_baseline=changed,
                  format_wal_manifest_unchanged_since_pre_mask=True,
                  frozen_programs_still_equal=True, readonly_sources_equal=True,
                  prototype_default=False, recoveries=recoveries, tests=tests,
                  lifecycles=normal_events)
    (L / f'{P}-final-audit.json').write_text(json.dumps(result, indent=2), encoding='utf-8')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
