"""Paired current-engine writes using only the existing first10k and owned slots."""
import argparse
import hashlib
import json
import statistics
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def sha(path):
    value = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            value.update(block)
    return value.hexdigest().upper()


def write(path, value):
    path.write_text(json.dumps(value, indent=2), encoding='utf-8')


def protected():
    return [dict(path=str(p.relative_to(ROOT)), bytes=p.stat().st_size, sha256=sha(p))
            for p in [ROOT / 'data/rkyv-corpus-20261009/json.payload-bench',
                      ROOT / 'tests/fixtures/v1/checkpoint.fdb',
                      ROOT / 'tests/fixtures/v1/checkpoint.ready']]


def core(engine):
    return [dict(name=p.name, bytes=p.stat().st_size, sha256=sha(p))
            for p in sorted((ROOT / f'data/benchmark-work/{engine}').iterdir())
            if p.is_file() and (p.suffix in ('.fdb', '.idx', '.ready') or p.name == 'sqlite.db')]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prefix', required=True)
    parser.add_argument('--control', required=True)
    parser.add_argument('--candidate', required=True)
    parser.add_argument('--sqlite', default='tools/native_sqlite/target/release/content_memory.exe')
    parser.add_argument('--pairs', type=int, choices=(2, 4), default=4)
    parser.add_argument('--owned-pairs', type=int, choices=(2, 4), default=2)
    parser.add_argument('--apis', nargs='+', choices=('streamed', 'owned'),
                        default=['streamed', 'owned'])
    parser.add_argument('--skip-sqlite', action='store_true',
                        help='Isolated FDB follow-up;do not repeat the existing SQLite comparison')
    args = parser.parse_args()
    args.apis = list(dict.fromkeys(args.apis))
    prefix = ROOT / args.prefix
    report = lambda suffix: Path(f'{prefix}-{suffix}')
    if prefix.parent != ROOT / 'docs/benchmarks' or report('design.json').exists():
        raise RuntimeError('Unused docs/benchmarks prefix required')
    programs = dict(control=ROOT / args.control, candidate=ROOT / args.candidate,
                    sqlite=ROOT / args.sqlite)
    sqlite_program = programs['sqlite']
    if args.skip_sqlite:
        programs.pop('sqlite')
    design = dict(
        programs={k: dict(path=str(v.relative_to(ROOT)), sha256=sha(v)) for k, v in programs.items()},
        streamed_pairs=args.pairs if 'streamed' in args.apis else 0,
        owned_pairs=args.owned_pairs if 'owned' in args.apis else 0,
        sqlite_remeasured=not args.skip_sqlite,
        corpus='same existing first10000;264842858 text bytes;no corpus generation/download',
        transactions='500 records/batch;20 acknowledged synced WAL commits;unchanged normal durability',
        timing='normal release;no diagnostic features;input CRC/JSON and40 boundary gets included in ingest',
        maintenance='final flush/drain for FDB;FTS optimize+WAL checkpoint for SQLite;not identical algorithms',
        api='FDB APIs: ' + ','.join(args.apis) + (';SQLite not remeasured' if args.skip_sqlite else ';SQLite input remains streaming'),
        sqlite='bundled FTS5 trigram;FULL WAL;cache2MiB;mmap0;tempFILE;not all FDB field/link/guard features',
        cache='warm OS cache;no eviction;sequential processes;no power/hardware isolation',
        memory='OS lifetime private commit/working set;not physical RAM/system RAM;phase samples may miss peaks',
        storage='only fixed FDB/SQLite slots reset through benchmark-workspace.ps1;no DB copies',
        verification='all10000 payloads after every run outside timing;FDB also4 full contains page oracles',
        orders=[])
    write(report('design.json'), design)
    before = protected()
    write(report('protected-before.json'), before)
    records = []
    for api, pairs in [('streamed', args.pairs), ('owned', args.owned_pairs)]:
        if api not in args.apis:
            continue
        reference = None
        for pair in range(pairs):
            # Reverse adjacent pairs, including SQLite's position.
            order = ['control', 'candidate', 'sqlite'] if pair % 2 == 0 else ['sqlite', 'candidate', 'control']
            order = [variant for variant in order if variant in programs]
            design['orders'].append(dict(api=api, pair=pair, engines=order))
            write(report('design.json'), design)
            for variant in order:
                engine = 'sqlite' if variant == 'sqlite' else 'fdb'
                leaf = f'{prefix.name}-{api}-{variant}-{pair}'
                command = ['powershell.exe', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File',
                           str(ROOT / 'tools/measure-content-memory.ps1'), '-Prefix', leaf,
                           '-FdbExecutable', str(programs[variant]),
                           '-SqliteExecutable', str(sqlite_program),
                           '-FdbFeatures', 'normal bounded-engine;no diagnostics',
                           '-Engines', engine, '-IdleRuns', '0']
                if api == 'streamed' and engine == 'fdb':
                    command.append('-StreamedInput')
                result = subprocess.run(command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                        text=True, encoding='utf-8', creationflags=subprocess.CREATE_NO_WINDOW)
                report(f'{api}-{variant}-{pair}-launcher.log').write_text(result.stdout + result.stderr, encoding='utf-8')
                result.check_returncode()
                stem = ROOT / f'docs/benchmarks/{leaf}-{engine}-ingest-10000-0'
                rows = [json.loads(s) for s in stem.with_suffix('.jsonl').read_text(encoding='utf-8-sig').splitlines()]
                measured = next(row for row in rows if row['phase'] == 'result')
                memory = json.loads(Path(f'{stem}.memory.json').read_text(encoding='utf-8-sig'))
                assert memory['exit_code'] == 0 and measured['text_bytes'] == 264842858
                with report(f'{api}-{variant}-{pair}-verify.jsonl').open('w', encoding='utf-8') as log:
                    subprocess.run([str(programs[variant]), 'verify', str(ROOT / f'data/benchmark-work/{engine}'), '0', '10000'],
                                   cwd=ROOT, stdout=log, stderr=subprocess.PIPE, check=True, creationflags=subprocess.CREATE_NO_WINDOW)
                verify = [json.loads(s) for s in report(f'{api}-{variant}-{pair}-verify.jsonl').read_text(encoding='utf-8').splitlines()]
                assert any(row.get('full_document_checks') == 10000 for row in verify)
                files = core(engine)
                write(report(f'{api}-{variant}-{pair}-files.json'), files)
                if engine == 'fdb':
                    if reference is None:
                        reference = files
                    assert files == reference, 'Source/index/ready bytes differ between normal writers'
                records.append(dict(api=api, pair=pair, engine=variant, result=measured, memory=memory))
                write(report('records.json'), records)
                print(f'{api} pair={pair} {variant}: write={measured["ingest_ms"]:.1f}ms final={measured["maintenance_ms"]:.1f}ms;10000 payloads verified', flush=True)
    after = protected()
    assert before == after, 'Protected corpus or legacy source changed'
    write(report('protected-after.json'), after)
    summary = dict(design=design, protected_unchanged=True, fdb_sources_indexes_ready_byte_equal=True, apis={})
    for api in args.apis:
        groups = {variant: [r for r in records if r['api'] == api and r['engine'] == variant] for variant in programs}
        for group in groups.values():
            for row in group:
                row['total_ms'] = row['result']['ingest_ms'] + row['result']['maintenance_ms']
        metrics = {}
        for name, metric in [
            ('write_ms', lambda r: r['result']['ingest_ms']),
            ('maintenance_ms', lambda r: r['result']['maintenance_ms']),
            ('total_ms', lambda r: r['total_ms']),
            ('cpu_ms', lambda r: r['memory']['cpu_ms']),
            ('peak_private_bytes', lambda r: r['memory']['os_lifetime_peaks']['peak_private_commit_bytes']),
            ('peak_working_set_bytes', lambda r: r['memory']['os_lifetime_peaks']['peak_working_set_bytes']),
            ('disk_bytes', lambda r: r['memory']['retained_disk_bytes'])]:
            medians = {key: statistics.median(metric(r) for r in group) for key, group in groups.items()}
            metrics[name] = dict(medians=medians,
                candidate_change_percent=100 * (medians['candidate'] / medians['control'] - 1),
                candidate_lower_pairs=sum(metric(a) < metric(b) for a, b in zip(groups['candidate'], groups['control'], strict=True)),
                pairs=len(groups['candidate']),
                paired_changes_percent=[100 * (metric(a) / metric(b) - 1) for a, b in zip(groups['candidate'], groups['control'], strict=True)])
        summary['apis'][api] = metrics
    write(report('summary.json'), summary)
    print(json.dumps(summary['apis'], indent=2), flush=True)


if __name__ == '__main__':
    main()
