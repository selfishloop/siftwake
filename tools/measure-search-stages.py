"""Read requests on the existing fixed slot; no explicit reset or repack.

Normal engine open/recovery remains enabled; hashes reject a changed cache.
"""
import argparse
import ctypes
import hashlib
import json
import math
import statistics
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest().upper()


def snapshot():
    if not any((ROOT / 'data/benchmark-work/fdb').glob('generation-*.ready')):
        raise RuntimeError('Existing published database required; never initialize a read slot')
    files = list((ROOT / 'data/benchmark-work/fdb').iterdir())
    files += [ROOT / 'data/benchmark-work/sqlite/sqlite.db',
              ROOT / 'tests/fixtures/v1/checkpoint.fdb',
              ROOT / 'tests/fixtures/v1/checkpoint.ready']
    return [dict(path=str(p.relative_to(ROOT)), bytes=p.stat().st_size, sha256=sha(p))
            for p in sorted(files) if p.is_file()]


def write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')


def memory_peaks(process):
    class Counters(ctypes.Structure):
        _fields_ = [('cb', ctypes.c_ulong), ('faults', ctypes.c_ulong)] + [
            (name, ctypes.c_size_t) for name in ('peak_ws', 'ws', 'peak_paged', 'paged',
                                               'peak_nonpaged', 'nonpaged', 'pagefile',
                                               'peak_pagefile', 'private')]
    counters = Counters()
    counters.cb = ctypes.sizeof(counters)
    api = ctypes.WinDLL('psapi', use_last_error=True)
    api.GetProcessMemoryInfo.argtypes = [ctypes.c_void_p, ctypes.POINTER(Counters), ctypes.c_ulong]
    api.GetProcessMemoryInfo.restype = ctypes.c_int
    if not api.GetProcessMemoryInfo(int(process._handle), ctypes.byref(counters), counters.cb):
        raise ctypes.WinError(ctypes.get_last_error())
    return dict(peak_private_commit_bytes=counters.peak_pagefile,
                peak_working_set_bytes=counters.peak_ws,
                scope='OS process lifetime high-water marks; not phase-specific or total system RAM; private commit is not physical RAM')


def pooled(rows):
    for row in rows:
        assert row['n'] == len(row['samples'])
        assert row['p95_ms'] == sorted(row['samples'])[math.ceil(row['n'] * .95) - 1]
    values = sorted(x for row in rows for x in row['samples'])
    return dict(n=len(values), p50_ms=statistics.median(values),
                p95_ms=values[math.ceil(len(values) * .95) - 1],
                p99_ms=values[math.ceil(len(values) * .99) - 1],
                run_p95=[row['p95_ms'] for row in rows])


def summarize(prefix, design):
    matrices = {}
    for variant in ('control', 'fdb'):
        matrices[variant] = []
        for pair in range(design['pairs']):
            rows = [json.loads(line) for line in Path(f'{prefix}-{variant}-{pair}.jsonl').read_text(encoding='utf-8').splitlines()]
            assert rows[0]['phase'] == 'opened'
            if design.get('control_engine') == 'sqlite' and variant == 'control':
                assert rows[0]['engine'] == 'sqlite' and not rows[0]['full_integrity_on_open']
            else:
                assert rows[0]['engine'] == 'fdb' and rows[0]['pending_source_checks'] == 0
            matrix = next(row for row in rows if row['phase'] == 'matrix_reads')
            assert matrix['full_document_checks']
            matrices[variant].append(matrix)
    queries = []
    for n, row in enumerate(matrices['control'][0]['queries']):
        identity = {key: row[key] for key in ('text', 'kind', 'offset', 'limit', 'total', 'late_matches')}
        groups = {}
        for variant, series in matrices.items():
            qs = [matrix['queries'][n] for matrix in series]
            assert all(all(q[key] == value for key, value in identity.items()) for q in qs)
            groups[variant] = pooled([q['timing'] for q in qs])
        a, b = groups['control'], groups['fdb']
        queries.append(dict(**identity, **groups,
                            change_percent=100 * (b['p95_ms'] / a['p95_ms'] - 1),
                            lower_pairs=sum(y < x for x, y in zip(a['run_p95'], b['run_p95'])),
                            paired_change_percent=[100 * (y / x - 1) for x, y in zip(a['run_p95'], b['run_p95'])]))
    gets = []
    for n in range(5):
        groups = {v: pooled([m['gets'][n]['timing'] for m in ms]) for v, ms in matrices.items()}
        gets.append(dict(bin=n, **groups, change_percent=100 * (groups['fdb']['p95_ms'] / groups['control']['p95_ms'] - 1)))
    result = dict(design=design, queries=queries, gets=gets,
                  lower_p95=sum(q['change_percent'] < 0 for q in queries),
                  higher_p95=sum(q['change_percent'] > 0 for q in queries),
                  files_unchanged=True)
    write(Path(f'{prefix}-summary.json'), result)
    print(json.dumps({k: result[k] for k in ('lower_p95', 'higher_p95')}, ensure_ascii=False), flush=True)
    for q in sorted(queries, key=lambda q: q['fdb']['p95_ms'] - q['control']['p95_ms'], reverse=True):
        print(q['text'][:32], q['offset'], q['limit'], round(q['change_percent'], 2), q['lower_pairs'], flush=True)


def profile_summary(prefix, design):
    groups = {}
    for variant in ('control', 'fdb'):
        for pair in range(design['pairs']):
            for line in Path(f'{prefix}-{variant}-{pair}.jsonl').read_text(encoding='utf-8').splitlines():
                row = json.loads(line)
                if row['phase'] == 'selectivity_profile':
                    identity = (row['text'], row['offset'], row['limit'])
                    groups.setdefault(identity, {}).setdefault(variant, []).append(row)
    queries = []
    for (text, offset, limit), variants in groups.items():
        entry = dict(text=text, offset=offset, limit=limit)
        for variant, rows in variants.items():
            names = sorted(set().union(*(row['counters'] for row in rows)))
            counters = {}
            for name in names:
                cs = [row['counters'].get(name, dict(calls=0, elapsed_ns=0)) for row in rows]
                counters[name] = dict(median_ms=statistics.median(c['elapsed_ns'] / 1e6 for c in cs),
                                      min_calls=min(c['calls'] for c in cs), max_calls=max(c['calls'] for c in cs))
            entry[variant] = dict(n=len(rows), median_ms=statistics.median(row['elapsed_ms'] for row in rows), counters=counters)
        queries.append(entry)
    write(Path(f'{prefix}-summary.json'), dict(design=design, queries=queries, files_unchanged=True,
                                             limits='Instrumented wall spans nest and include timer/bookkeeping overhead. Do not sum nested spans or present these medians as normal release p95.'))


def process_summary(prefix, design):
    rows = []
    for variant in ('control', 'fdb'):
        for pair in range(design['pairs']):
            events = {row['phase']: row for row in map(json.loads, Path(f'{prefix}-{variant}-{pair}.jsonl').read_text(encoding='utf-8').splitlines())}
            if design['phase'] == 'open':
                opened, first = events['opened'], events['deferred_first_use']
                assert opened['pending_source_checks'] == first['pending_on_open'] == 0
                values = dict(open_ms=opened['open_ms'],
                              first_use_ms=opened['open_ms'] + first['first_get_ms'] + first['first_search_ms'])
            else:
                peaks = json.loads(Path(f'{prefix}-{variant}-{pair}.memory.json').read_text(encoding='utf-8'))
                values = {key: peaks[key] for key in ('peak_private_commit_bytes', 'peak_working_set_bytes')}
            rows.append(dict(variant=variant, pair=pair, **values))
    keys = [key for key in rows[0] if key not in ('variant', 'pair')]
    medians = {variant: {key: statistics.median(row[key] for row in rows if row['variant'] == variant)
                         for key in keys} for variant in ('control', 'fdb')}
    comparisons = {}
    for key in keys:
        old = [row[key] for row in rows if row['variant'] == 'control']
        new = [row[key] for row in rows if row['variant'] == 'fdb']
        comparisons[key] = dict(change_percent=100 * (medians['fdb'][key] / medians['control'][key] - 1),
                                lower_pairs=sum(y < x for x, y in zip(old, new)),
                                paired_change_percent=[100 * (y / x - 1) for x, y in zip(old, new)])
    write(Path(f'{prefix}-summary.json'), dict(design=design, rows=rows, medians=medians,
                                             comparisons=comparisons, files_unchanged=True))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prefix', required=True)
    parser.add_argument('--control', required=True)
    parser.add_argument('--candidate', required=True)
    parser.add_argument('--pairs', type=int, choices=(2, 4), default=4)
    parser.add_argument('--phase', choices=('matrix', 'dense', 'stress', 'profile', 'regressions', 'open', 'memory'), default='matrix')
    parser.add_argument('--affinity', type=int, default=4)
    parser.add_argument('--sqlite-control', action='store_true', help='Read-only native SQLite comparator; matrix/memory only')
    parser.add_argument('--resume', action='store_true', help='Resume an interrupted matrix, preserving completed logs and partial attempts')
    args = parser.parse_args()
    if args.sqlite_control and args.phase not in ('matrix', 'memory'):
        parser.error('--sqlite-control supports only matrix/memory phases')
    if args.resume and args.phase != 'matrix':
        parser.error('--resume supports only matrix')
    prefix = ROOT / args.prefix
    report = lambda suffix: Path(f'{prefix}-{suffix}')
    if report('design.json').exists() and not args.resume:
        raise RuntimeError('Preserve existing reports; choose an unused log prefix')
    programs = {'control': ROOT / args.control, 'fdb': ROOT / args.candidate}
    design = dict(pairs=args.pairs, phase=args.phase, affinity_mask=args.affinity,
                  same_executable=sha(programs['control']) == sha(programs['fdb']),
                  diagnostic_timing=args.phase in ('profile', 'regressions'),
                  programs={v: dict(path=str(p.relative_to(ROOT)), sha256=sha(p)) for v, p in programs.items()},
                  cache='warm OS cache, fresh processes, per-case untimed warmup, no eviction',
                  validation='normal strict open; complete source/index CRC and schema; pending0; payload CRC/schema/key/version preserved',
                  corpus='same existing first10000, 264842858 text bytes; no generation/reset/repack/copy',
                  limits='No concurrent load, true cold cache or hardware/power isolation. Matrix oracle memory excluded from engine memory claims. Diagnostic spans nest; never sum as exclusive elapsed time.',
                  orders=[])
    design['control_engine'] = 'sqlite' if args.sqlite_control else 'fdb'
    if args.sqlite_control:
        design['validation'] = 'FDB strict source/index open and payload CRC/schema/identity; SQLite read-only without full integrity on open. Both full document/version oracle checks outside query timing.'
        design['sqlite'] = 'Bundled FTS5 trigram/LIKE; FULL-WAL-built existing DB; cache2MiB, mmap0, tempFILE, prepared statement cache64. Different source-integrity and field/link capabilities.'
    if args.resume:
        if report('summary.json').exists() or report('after.json').exists():
            raise RuntimeError('Completed measurements are immutable; no resume needed')
        saved = json.loads(report('design.json').read_text(encoding='utf-8'))
        for key in ('pairs', 'phase', 'affinity_mask', 'programs', 'control_engine'):
            if saved[key] != design[key]:
                raise RuntimeError(f'Resume configuration/executable mismatch: {key}')
        before = json.loads(report('before.json').read_text(encoding='utf-8'))
        if snapshot() != before:
            raise RuntimeError('Database/fixture bytes changed since the interrupted run')
        if not report('resume-original-design.json').exists():
            write(report('resume-original-design.json'), saved)
        design['resumed_after_interruption'] = True
        design['resume_reused_logs'] = []
        design['resume_preserved_partial_logs'] = []
        design['limits'] += ' Interrupted batch resumed with identical executable hashes and DB bytes; earlier/later pairs can have different machine conditions.'
    else:
        before = snapshot()
        write(report('before.json'), before)
    write(report('design.json'), design)
    api = ctypes.WinDLL('kernel32', use_last_error=True)
    api.SetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
    api.SetProcessAffinityMask.restype = ctypes.c_int
    api.GetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_size_t), ctypes.POINTER(ctypes.c_size_t)]
    api.GetProcessAffinityMask.restype = ctypes.c_int
    for pair in range(args.pairs):
        order = ['control', 'fdb'] if pair % 2 == 0 else ['fdb', 'control']
        design['orders'].append(order)
        write(report('design.json'), design)
        for variant in order:
            flags = {'matrix': ['--matrix', str(pair)],
                     'dense': ['--matrix', str(pair), '--pages-only'],
                     'stress': ['--matrix', str(pair), '--stress-only'],
                     'profile': ['--profile-matrix'],
                     'regressions': ['--profile-regressions'],
                     'open': ['--deferred-probe'], 'memory': ['--memory-probe']}[args.phase]
            if args.resume and report(f'{variant}-{pair}.jsonl').exists():
                log_path = report(f'{variant}-{pair}.jsonl')
                rows = [json.loads(line) for line in log_path.read_text(encoding='utf-8').splitlines()]
                matrix = [row for row in rows if row['phase'] == 'matrix_reads']
                if matrix:
                    assert len(matrix) == 1 and rows[-1] == matrix[0]
                    assert matrix[0]['seed'] == pair and matrix[0]['repeats'] == 20 and matrix[0]['full_document_checks']
                    assert len(matrix[0]['queries']) == 38 and not report(f'{variant}-{pair}.stderr').read_text().strip()
                    design['resume_reused_logs'].append(log_path.name)
                    write(report('design.json'), design)
                    print(f'{args.phase} pair={pair} {variant} reused complete validated log', flush=True)
                    continue
                attempt = 1
                while report(f'{variant}-{pair}.interrupted-{attempt}.jsonl').exists():
                    attempt += 1
                for suffix in ('jsonl', 'stderr'):
                    partial = report(f'{variant}-{pair}.{suffix}')
                    if partial.exists():
                        preserved = report(f'{variant}-{pair}.interrupted-{attempt}.{suffix}')
                        partial.rename(preserved)
                        design['resume_preserved_partial_logs'].append(preserved.name)
                write(report('design.json'), design)
            with report(f'{variant}-{pair}.jsonl').open('w', encoding='utf-8') as out, report(f'{variant}-{pair}.stderr').open('w', encoding='utf-8') as err:
                process = subprocess.Popen([str(programs[variant]), *flags], cwd=ROOT, stdout=out, stderr=err, creationflags=subprocess.CREATE_NO_WINDOW)
                try:
                    if not api.SetProcessAffinityMask(int(process._handle), args.affinity):
                        raise ctypes.WinError(ctypes.get_last_error())
                    actual, system = ctypes.c_size_t(), ctypes.c_size_t()
                    if not api.GetProcessAffinityMask(int(process._handle), ctypes.byref(actual), ctypes.byref(system)) or actual.value != args.affinity:
                        raise RuntimeError('Requested CPU affinity was not applied')
                    if process.wait() != 0:
                        raise RuntimeError(f'{variant} pair{pair} failed; inspect stderr')
                    if args.phase == 'memory':
                        write(report(f'{variant}-{pair}.memory.json'), memory_peaks(process))
                finally:
                    if process.poll() is None:
                        process.kill()
                        process.wait()
            rows = [json.loads(line) for line in report(f'{variant}-{pair}.jsonl').read_text(encoding='utf-8').splitlines()]
            assert rows[0]['phase'] == 'opened'
            if args.sqlite_control and variant == 'control':
                assert rows[0]['engine'] == 'sqlite' and not rows[0]['full_integrity_on_open']
            else:
                assert rows[0]['engine'] == 'fdb' and rows[0]['pending_source_checks'] == 0
            if args.phase == 'profile':
                samples = [row for row in rows if row['phase'] == 'selectivity_profile']
                assert len(samples) == 38 * 3 and all(row['instrumented'] for row in samples)
            elif args.phase == 'memory':
                probe = next(row for row in rows if row['phase'] == 'query_memory_verified')
                assert probe['searches'] == 24 and probe['gets'] == 771 and not probe['oracle_loaded']
            print(f'{args.phase} pair={pair} {variant} complete', flush=True)
    after = snapshot()
    write(report('after.json'), after)
    assert before == after, 'Read changed source/index/SQLite/fixture files'
    if args.phase in ('matrix', 'dense', 'stress'):
        summarize(prefix, design)
    elif args.phase == 'profile':
        profile_summary(prefix, design)
    elif args.phase in ('open', 'memory'):
        process_summary(prefix, design)
    print('All database and fixture hashes unchanged', flush=True)


if __name__ == '__main__':
    main()
