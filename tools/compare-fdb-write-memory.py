"""Windows CLI comparison on new synthetic databases. No application integration.

The external Python client's allocations are excluded from engine memory.
Times include JSON Lines IPC but exclude corpus construction. Private-byte peaks
are sampled (20 ms); working-set peaks are the OS process counter.
"""
import argparse
import ctypes
from ctypes import wintypes
import hashlib
import json
from pathlib import Path
import subprocess
import threading
import time


class Counters(ctypes.Structure):
    _fields_ = [('cb', wintypes.DWORD), ('PageFaultCount', wintypes.DWORD)] + [
        (name, ctypes.c_size_t) for name in ['PeakWorkingSetSize', 'WorkingSetSize',
        'QuotaPeakPagedPoolUsage', 'QuotaPagedPoolUsage', 'QuotaPeakNonPagedPoolUsage',
        'QuotaNonPagedPoolUsage', 'PagefileUsage', 'PeakPagefileUsage', 'PrivateUsage']]


def document(number, body):
    return dict(key=dict(scope='shared', collection='contents', id=f'{number:010}'),
                title=f'rapor-{number:010}', text=body,
                fields=dict(day=number % 30, source='synthetic'), links=[])


def request(proc, payload):
    proc.stdin.write(payload if isinstance(payload, str) else json.dumps(payload) + '\n')
    proc.stdin.flush()
    line = proc.stdout.readline()
    if not line:
        raise RuntimeError('FDB closed its output unexpectedly')
    reply = json.loads(line)
    if not reply['ok']:
        raise RuntimeError(reply)
    return reply['result']


def launch(exe, database):
    return subprocess.Popen([str(exe), str(database), '--stdio'], stdin=subprocess.PIPE,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            encoding='utf-8', bufsize=1)


def close(proc):
    proc.stdin.close()
    proc.wait(timeout=60)
    error = proc.stderr.read()
    proc.stdout.close()
    proc.stderr.close()
    if proc.returncode:
        raise RuntimeError(error)


def measure(exe, database, batches, count, body):
    if database.exists():
        raise RuntimeError(f'New directory required: {database}')
    memory_info = ctypes.WinDLL('psapi', use_last_error=True).GetProcessMemoryInfo
    memory_info.argtypes = [wintypes.HANDLE, ctypes.POINTER(Counters), wintypes.DWORD]
    memory_info.restype = wintypes.BOOL
    proc = launch(exe, database)
    stop = threading.Event()
    memory = dict(peak_working_set_bytes=0, sampled_peak_private_bytes=0, samples=0)
    failures = []

    def sample():
        try:
            while not stop.is_set():
                counters = Counters()
                counters.cb = ctypes.sizeof(counters)
                if not memory_info(wintypes.HANDLE(int(proc._handle)), ctypes.byref(counters), counters.cb):
                    raise ctypes.WinError(ctypes.get_last_error())
                memory['peak_working_set_bytes'] = max(memory['peak_working_set_bytes'], counters.PeakWorkingSetSize)
                memory['sampled_peak_private_bytes'] = max(memory['sampled_peak_private_bytes'], counters.PrivateUsage)
                memory['samples'] += 1
                stop.wait(.020)
        except Exception as error:
            failures.append(str(error))

    sampler = threading.Thread(target=sample)
    sampler.start()
    try:
        request(proc, dict(op='stats'))
        started = time.perf_counter()
        for sequence, batch in enumerate(batches, 1):
            assert request(proc, batch)['sequence'] == sequence
        ingest_s = time.perf_counter() - started
        started = time.perf_counter()
        request(proc, dict(op='flush'))
        request(proc, dict(op='wait_compaction'))
        while request(proc, dict(op='compact'))['compaction_running']:
            request(proc, dict(op='wait_compaction'))
        maintenance_s = time.perf_counter() - started
        stats = request(proc, dict(op='stats'))
        assert stats['documents'] == count and stats['resident_documents'] == 0
    finally:
        stop.set()
        sampler.join()
        close(proc)
    if failures:
        raise RuntimeError(failures)
    proc = launch(exe, database)
    try:
        reopened = request(proc, dict(op='stats'))
        assert reopened['documents'] == count
        for number in [0, count // 2, count - 1]:
            doc = document(number, body)
            assert request(proc, dict(op='get', key=doc['key'])) == dict(
                document=doc, version=number // 500 + 1)
    finally:
        close(proc)
    return dict(ingest_s=ingest_s, maintenance_s=maintenance_s, stats=stats,
                verified_reopened_documents=3, **memory)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('baseline')
    parser.add_argument('candidate')
    parser.add_argument('directory_prefix')
    parser.add_argument('--count', type=int, default=100000)
    parser.add_argument('--runs', type=int, default=3)
    args = parser.parse_args()
    assert args.count >= 1000 and args.runs > 0
    body = 'Şirketin teknoloji yatırımları ve bilanço raporu. ' * 20
    while len(body.encode('utf-8')) > 256:
        body = body[:-1]
    batches = [json.dumps(dict(op='commit', mutations=[dict(op='put',
        document=document(number, body), guard=dict(kind='missing'))
        for number in range(first, min(first + 500, args.count))]), ensure_ascii=False) + '\n'
        for first in range(0, args.count, 500)]
    engines = [('baseline', Path(args.baseline).resolve()), ('candidate', Path(args.candidate).resolve())]
    for run in range(args.runs):
        for label, exe in engines if run % 2 == 0 else list(reversed(engines)):
            database = Path(f'{args.directory_prefix}-{label}-{run}').resolve()
            result = measure(exe, database, batches, args.count, body)
            print(json.dumps(dict(run=run, engine=label, executable=str(exe),
                binary_sha256=hashlib.sha256(exe.read_bytes()).hexdigest(), database=str(database),
                count=args.count, text_bytes=len(body.encode('utf-8')), batch_size=500,
                includes_stdio=True, sample_interval_ms=20, memory_scope='FDB process only; excludes Python client and system file cache',
                corpus='synthetic shared/contents, ten-digit IDs, repeated Turkish text, day and source fields',
                **result)), flush=True)


if __name__ == '__main__':
    main()
