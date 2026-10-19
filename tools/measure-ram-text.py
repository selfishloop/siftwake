"""Read-only paired text materialization probe; never opens or resets a database."""
import argparse
import ctypes
import importlib
import json
import math
import os
import statistics
import subprocess
from pathlib import Path

bench = importlib.import_module('measure-write-stages')
reads = importlib.import_module('measure-interleaved-reads')
ROOT = bench.ROOT


def percentile(values, p):
    return sorted(values)[math.ceil(len(values) * p / 100) - 1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prefix', required=True)
    args = parser.parse_args()
    prefix = ROOT / args.prefix
    report = lambda suffix: Path(f'{prefix}-{suffix}')
    if prefix.parent != ROOT / 'docs/benchmarks' or report('design.json').exists():
        raise RuntimeError('Unused docs/benchmarks prefix required')
    program = ROOT / 'target/ram-text-probe/release/fdb-ram-text-probe.exe'
    design = dict(
        executable=str(program.relative_to(ROOT)), sha256=bench.sha(program),
        codec='lz4_flex0.14.0;safe-encode,safe-decode,checked-decode,std;block per document;no trained/inter-document dictionary',
        corpus='existing first10000;264842858 text bytes;200 source texts maximum;no generation/download',
        modes='raw owned copy versus LZ4 decode+UTF8,optional decoded CRC;fresh/reused output separately',
        retained='sum of200-text groups;String/encoded capacities+actual vector metadata;not simultaneous/process/physical memory',
        timing='normal System allocator;input CRC/JSON+input clone+output vector reserve+oracle+drop outside materialization timer;UTF8+optional CRC inside',
        encode='compression+decoded CRC+compact box+raw/packed selection;owned input cloning excluded',
        cache='resident group;two warmups per mode/page;OS/allocator warm;no eviction/power/background isolation',
        affinity=4, runs=4, rounds_per_page=8,
        queries='materialize1,50,200 texts only;no contains/normalization/postings/API/DB/SQLite query speed claim',
        exclusions='fields,title,links,normalized search text,WAL,maintenance;both raw oracle and compressed data coexist in probe',
        engine='unchanged;new compression dependency confined to standalone probe;no new engine feature',
        rustflags=os.environ.get('RUSTFLAGS'), cargo_flags='release;thin LTO;one codegen unit',
        sources={str(p.relative_to(ROOT)): bench.sha(p) for p in [
            ROOT / 'tools/ram_text_probe/Cargo.toml', ROOT / 'tools/ram_text_probe/Cargo.lock',
            ROOT / 'tools/ram_text_probe/src/main.rs', ROOT / 'tools/ram_text_probe/src/text.rs',
            ROOT / 'tools/measure-ram-text.py']})
    bench.write(report('design.json'), design)
    before = reads.snapshot()
    protected = bench.protected()
    bench.write(report('files-before.json'), before)
    bench.write(report('protected-before.json'), protected)
    records = []
    library = ctypes.WinDLL('kernel32', use_last_error=True)
    library.SetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
    library.SetProcessAffinityMask.restype = ctypes.c_int
    library.GetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_size_t), ctypes.POINTER(ctypes.c_size_t)]
    library.GetProcessAffinityMask.restype = ctypes.c_int
    for run in range(4):
        with report(f'run{run}.jsonl').open('w', encoding='utf-8') as log, report(f'run{run}.stderr').open('w', encoding='utf-8') as errors:
            process = subprocess.Popen([str(program), str(run)], cwd=ROOT, stdout=log,
                                       stderr=errors, creationflags=subprocess.CREATE_NO_WINDOW)
            try:
                if not library.SetProcessAffinityMask(int(process._handle), 4):
                    raise ctypes.WinError(ctypes.get_last_error())
                actual, system = ctypes.c_size_t(), ctypes.c_size_t()
                if not library.GetProcessAffinityMask(int(process._handle), ctypes.byref(actual), ctypes.byref(system)) or actual.value != 4:
                    raise RuntimeError('Affinity not applied')
                code = process.wait()
            finally:
                if process.poll() is None:
                    process.kill()
                    process.wait()
            if code:
                raise RuntimeError(f'Probe failed: {code};see stderr')
        rows = [json.loads(s) for s in report(f'run{run}.jsonl').read_text(encoding='utf-8').splitlines()]
        result = next(r for r in rows if r['phase'] == 'result')
        assert result['documents'] == 10000 and result['text_bytes'] == 264842858 and result['all_payloads_byte_equal']
        groups = [r for r in rows if r['phase'] == 'group']
        pages = [r for r in rows if r['phase'] == 'page']
        assert len(groups) == 50 and len(pages) == 300 and all(r['all_payloads_byte_equal'] for r in groups)
        metrics = {}
        for count in (1, 50, 200):
            selected = [r for r in pages if r['count'] == count]
            modes = {entry['mode']: [] for entry in selected[0]['modes']}
            sums = {mode: 0 for mode in modes}
            for row in selected:
                for entry in row['modes']:
                    assert len(entry['samples_ns']) == 8
                    modes[entry['mode']].extend(entry['samples_ns'])
                    sums[entry['mode']] += sum(entry['samples_ns'])
            metrics[count] = {mode: dict(p50_ns=statistics.median(samples), p95_ns=percentile(samples, 95),
                                        samples=len(samples), sum_ns=sums[mode]) for mode, samples in modes.items()}
        records.append(dict(run=run, result=result, metrics=metrics))
        bench.write(report('records.json'), records)
        print(f'run={run};all10000 texts byte-equal;compressed={result["compressed_records"]};retained ratio={result["packed_retained_sum_bytes"]/result["raw_retained_sum_bytes"]:.4f}', flush=True)
    after, protected_after = reads.snapshot(), bench.protected()
    assert before == after and protected == protected_after, 'DB/corpus/fixture bytes changed'
    bench.write(report('files-after.json'), after)
    bench.write(report('protected-after.json'), protected_after)
    summary = dict(design=design, all_db_corpus_fixture_bytes_unchanged=True,
                   retained_sums={key: records[0]['result'][key] for key in ['raw_retained_sum_bytes', 'packed_retained_sum_bytes', 'compressed_records']},
                   encode_crc_compact_ms=[r['result']['encode_crc_compact_ns']/1e6 for r in records], materialization={})
    for count in (1, 50, 200):
        result = {}
        for candidate, control in [('lz4_utf8_fresh', 'raw_fresh'), ('lz4_crc_utf8_fresh', 'raw_fresh'),
                                   ('lz4_utf8_reused', 'raw_reused'), ('lz4_crc_utf8_reused', 'raw_reused')]:
            samples = [(r['metrics'][count][control], r['metrics'][count][candidate]) for r in records]
            result[candidate] = dict(
                control=control, p95_ms={name: statistics.median(r['metrics'][count][mode]['p95_ns'] for r in records)/1e6
                                        for name, mode in [('control', control), ('candidate', candidate)]},
                total_ratio_per_run=[a['sum_ns']/b['sum_ns'] for b, a in samples],
                p95_lower_runs=sum(a['p95_ns'] < b['p95_ns'] for b, a in samples),
                total_lower_runs=sum(a['sum_ns'] < b['sum_ns'] for b, a in samples),
                samples_per_mode_run=samples[0][0]['samples'])
        summary['materialization'][count] = result
    bench.write(report('summary.json'), summary)
    print(json.dumps(summary, indent=2), flush=True)


if __name__ == '__main__':
    main()
