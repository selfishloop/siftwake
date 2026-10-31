"""Closing hashes, sample counts, recovery and existing gate arithmetic."""
import hashlib
import json
import re
from pathlib import Path

R = Path(__file__).resolve().parent.parent
L = R / 'docs/benchmarks'
P = 'side-cost50-20261031'


def read(path):
    return json.loads(path.read_text(encoding='utf-8'))


def sha(path):
    with path.open('rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest().upper()


def main():
    prefixes = ['side-cost-cpu50-20261031', 'side-cost-broad50-20261031', 'side-cost-ram50-20261031']
    designs = [read(L / f'{p}-design.json') for p in prefixes]
    assert all(d['stage'] == 'completed' for d in designs)
    cpu, broad, ram = designs
    assert [len(d['completed']) for d in designs] == [40, 32, 32]
    current = {str(p.relative_to(R)): sha(p) for p in sorted((R / 'src').rglob('*.rs'))}
    assert current == cpu['engine_sources_before'] == cpu['engine_sources_after']
    assert current == read(L / 'estimator50-20261031-design.json')['engine_sources_after']
    assert 'Cell::new(true)' in (R / 'src/engine/first.rs').read_text()
    for d in designs:
        assert d['source_sha256'] == d['source_after'] == '881756DA400952500BA871F6C7BA8FF8D17E5348E5975420BC5CD90952A7CF12'
        assert d['protected_before'] == d['protected_after'] == cpu['protected_before']
    for f in cpu['protected_before']:
        assert sha(R / f['path']) == f['sha256'] and (R / f['path']).stat().st_size == f['bytes']
    assert sha(Path('D:/frunner-corpus/trwiki-100k-cap200k/corpus.payload-bench')) == cpu['source_sha256']
    assert cpu['before_readonly_sources'] == cpu['after_readonly_sources']
    assert broad['live_before'] == broad['live_after']
    for name, value in [('output/estimator50-20261031-fdb.exe', cpu['normal_sha256']),
                        ('output/side-cost-cpu50-20261031-diag.exe', cpu['diagnostic_sha256']),
                        ('output/hybrid-serial-ram-20261030.exe', ram['program_sha256'])]:
        assert sha(R / name) == value
    recovery = []
    for suffix in ['main-before-reset', 'final']:
        x = read(L / f'side-cost-ram50-20261031-{suffix}-recovery.json')
        assert len(x['kills']) == 3 and x['old_reader']['oracle'] and x['live_sources_unchanged']
        assert x['protected_before'] == x['protected_after']
        recovery.append(dict(scope=x['scope'], kills=3, old_reader=True, live_sources_unchanged=True))
    lifecycles = []
    for p in prefixes:
        v = [json.loads(x) for x in (L / f'{p}-lifecycle.jsonl').read_text().splitlines()]
        exits = [x for x in v if x['kind'] == 'worker_exit']
        assert exits and all(x['exit_code'] == 0 for x in exits)
        assert v[-1]['kind'] == 'driver_finish' and v[-1]['stage'] == 'completed'
        lifecycles.append(dict(prefix=p, children=len(exits), normal_exit=True))
    s = read(L / f'{P}-summary.json')
    assert all(x['n'] >= 60 for x in s['cpu'] + s['broad'])
    assert len(s['ram_pairs']) == 16
    for x in ram['completed']:
        q = x['measurement']
        assert q['documents'] == 50000
        if x['mode'] == 'loaded':
            assert q['writer_mutations'] == 16384 and len(q['query_counts']) == 4 and min(q['query_counts']) > 0
    a = next(x for x in s['pooled_writes'] if x['role']=='after' and not x['control'])
    sql = next(x for x in s['pooled_writes'] if x['role']=='sqlite' and not x['control'])
    broad_checks = []
    cpu_checks = []
    for pool, contexts, target, key in [
        (s['broad_pooled'], ['six-1R', 'six-4R'], broad_checks, 'p95'),
        (s['cpu_pooled'], ['six-1R', 'six-4R', 'full41-4R+W'], cpu_checks, 'cpu_mean')]:
        for b in pool:
            if b['role']!='before' or b['context'] not in contexts or b['case_id']==41:
                continue
            c = next(x for x in pool if x['role']=='after' and x['context']==b['context'] and x['case_id']==b['case_id'])
            target.append(dict(context=b['context'], case_id=b['case_id'], before=b[key], after=c[key], nonincrease=c[key]<=b[key]))
    gates = dict(write_retention_75_and_above_sqlite=a['retention']>=.75 and a['retention']>sql['retention'],
                 single_commit_p95_p99_p999=a['p95']<=20 and a['p99']<=50 and a['p999']<=100,
                 ram_ingest60_idle15=all(x['passed'] for x in s['ram_pairs'] if x['limit'] is not None),
                 ram_loaded_nonincrease=all(x['difference']<=0 for x in s['ram_pairs'] if x['mode']=='loaded'),
                 broad_pooled_nonincrease=all(x['nonincrease'] for x in broad_checks),
                 cpu_pooled_nonincrease=all(x['nonincrease'] for x in cpu_checks))
    tests = (L / f'{P}-tests.log').read_text(encoding='utf-8')
    matches = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored', tests)
    assert matches and all(f=='0' for _,f,_ in matches)
    out = dict(gates=gates, broad_checks=broad_checks, cpu_checks=cpu_checks,
               engine_sources_unchanged=True, corpus_fixture_hashes_equal=True, read_only_sources_equal=True,
               frozen_programs_equal=True, prototype_default=False, bounded_wait_implemented=False,
               recoveries=recovery, lifecycles=lifecycles,
               tests_passed=sum(int(p) for p,_,_ in matches), tests_ignored=sum(int(i) for _,_,i in matches),
               original_normal_reader_gates=read(L/'estimator50-20261031-summary.json')['role_gates'])
    (L / f'{P}-final-audit.json').write_text(json.dumps(out, indent=2), encoding='utf-8')
    print(json.dumps({k:v for k,v in out.items() if k not in ['broad_checks','cpu_checks']}))


if __name__ == '__main__':
    main()
