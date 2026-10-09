"""Sequential warm-cache comparison of two FDB binaries on the same healthy DB.

Does not submit mutations or maintenance. FDB open retains its normal recovery
and cache validation behavior. Measures JSON Lines client wall time, not only
engine execution; neither interpreter memory nor startup time is a query metric.
"""
import argparse
import json
import subprocess
import time
from pathlib import Path


def samples(values):
    ordered = sorted(values)
    return dict(p50_ms=ordered[len(ordered)//2],
                p95_ms=ordered[min(len(ordered)-1,len(ordered)*95//100)],
                max_ms=ordered[-1])


def request(proc, value):
    proc.stdin.write(json.dumps(value, ensure_ascii=False)+'\n')
    proc.stdin.flush()
    line = proc.stdout.readline()
    if not line:
        raise RuntimeError(f'FDB exited: {proc.stderr.read()}')
    response = json.loads(line)
    if not response['ok']:
        raise RuntimeError(response)
    return response['result']


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('baseline')
    parser.add_argument('candidate')
    parser.add_argument('database')
    parser.add_argument('--runs', type=int, default=5)
    parser.add_argument('--start-run', type=int, default=0,
                        help='resume at this run number; earlier complete runs are retained separately')
    parser.add_argument('--skip-text', action='store_true', help='target metadata/offset regression checks')
    args = parser.parse_args()
    if not 0 <= args.start_run < args.runs:
        parser.error('start-run must be between 0 and runs-1')
    base = dict(scope='shared', collection='contents', limit=50, offset=0)
    queries = [
        ('narrow', dict(base, text='rapor-0000500001'), 30),
        ('exact', dict(base, exact=dict(source='synthetic')), 30),
        ('range', dict(base, exact=dict(source='synthetic'), ranges=[dict(field='day',min=2,max=5)]), 30),
        ('text', dict(base, text='teknoloji'), 3),
        ('second_page', dict(base, exact=dict(source='synthetic'), offset=50), 3),
        ('last_page', dict(base, exact=dict(source='synthetic'), offset=999900), 3),
    ]
    if args.skip_text:
        queries = [(name, query, 3 if name == 'last_page' else 30) for name, query, _ in queries if name != 'text']
    expected = {}
    for run in range(args.start_run, args.runs):
        order = [('baseline',args.baseline),('candidate',args.candidate)]
        if run % 2:
            order.reverse()
        for label, exe in order:
            start = time.perf_counter()
            proc = subprocess.Popen([str(Path(exe).resolve()),str(Path(args.database).resolve()),'--stdio'],
                stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,
                encoding='utf-8',bufsize=1)
            try:
                stats = request(proc,dict(op='stats'))
                print(json.dumps(dict(phase='open',run=run,engine=label,
                    open_ms=(time.perf_counter()-start)*1000,stats=stats)),flush=True)
                for name, query, repeats in queries:
                    # One untimed request warms this query for each process.
                    page = request(proc,dict(op='search',query=query))
                    if name in expected:
                        assert page == expected[name],(label,name,'result mismatch')
                    else:
                        expected[name] = page
                    times = []
                    for _ in range(repeats):
                        start = time.perf_counter()
                        page = request(proc,dict(op='search',query=query))
                        times.append((time.perf_counter()-start)*1000)
                        assert page == expected[name],(label,name,'snapshot changed')
                    print(json.dumps(dict(phase='query',run=run,engine=label,query=name,
                        total=page['total'],hits=len(page['hits']),timing=samples(times),
                        cache='warm local OS cache; per-query warmup',includes_stdio=True)),flush=True)
            finally:
                proc.stdin.close()
                proc.wait(timeout=30)
                if proc.returncode != 0:
                    raise RuntimeError(proc.stderr.read())
                proc.stdout.close()
                proc.stderr.close()


if __name__ == '__main__':
    main()
