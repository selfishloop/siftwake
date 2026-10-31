"""Inspect complete snapshot JSONL evidence; never modifies DBs or samples."""
import glob
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
P = 'snapshot-10k-20261026'


def percentile(values, q=.95):
    values = sorted(values)
    return values[max(0, math.ceil(len(values)*q)-1)]


def records(row):
    return [r for thread in row['results'] for r in thread['records']]


def main():
    for filename in sorted(glob.glob(str(ROOT/f'docs/benchmarks/{P}-*-load-*-streamed-snapshot-*.jsonl'))):
        rows = [json.loads(line) for line in Path(filename).read_text(encoding='utf-8').splitlines()]
        phases = {row['phase']: row for row in rows}
        if 'snapshot_load' not in phases:
            continue
        load = phases['snapshot_load']
        out = dict(log=Path(filename).name, commits=load['commits'], ops_s=load['ops_s'],
                   maintenance=load['maintenance'], memory=load['memory'])
        read = records(load)
        out['reader_waits'] = dict(queries=len(read), writer_waited=sum(r['writer_waited'] for r in read),
                                  max_writer_ms=max(r['max_writer_wait_ns'] for r in read)/1e6,
                                  max_admission_ms=max(r['admission']['wait_ns'] for r in read)/1e6,
                                  max_release_ms=max(r['release_ms'] for r in read))
        for idle in ['snapshot_idle_single', 'snapshot_idle_four']:
            baseline = records(phases[idle])
            bad = []
            values = []
            for case in range(38):
                a = percentile([r['engine_ms'] for r in baseline if r['case_id'] == case])
                b = percentile([r['engine_ms'] for r in read if r['case_id'] == case])
                passed = b-a <= 2 if a < 1 else b <= a*2
                values.append(dict(case=case, idle_ms=a, load_ms=b, ratio=b/a, delta_ms=b-a, passed=passed))
                if not passed:
                    bad.append(values[-1])
            out[idle] = dict(failed=bad, cases=values)
        print(json.dumps(out, ensure_ascii=False))


if __name__ == '__main__':
    main()
