"""Repeat the same frozen off/off/on executable; read-only fixed 10k slot.

CPU observations bracket whole blocks OUTSIDE query clocks. They include strict
open, warmup, query/get work and full oracle checks, not per-query CPU attribution.
No process priorities, power settings, engine source or database bytes change.
"""
import ctypes
from ctypes import wintypes
from datetime import datetime, timezone
import importlib.util
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
PREFIX = ROOT / 'docs/benchmarks/reuse-repeat-10k-20261018'
EXE = ROOT / 'output/reuse-controls-20261018-normal.exe'
EXPECTED_SHA = '0324F3CC03424CB8FB63DE010AB337B22091A1A60E92B3C401D7472B417725B2'


def report(suffix):
    return Path(f'{PREFIX}-{suffix}')


api = ctypes.WinDLL('kernel32', use_last_error=True)
FT = wintypes.FILETIME
api.GetProcessTimes.argtypes = [wintypes.HANDLE] + [ctypes.POINTER(FT)] * 4
api.GetProcessTimes.restype = wintypes.BOOL
api.GetSystemTimes.argtypes = [ctypes.POINTER(FT)] * 3
api.GetSystemTimes.restype = wintypes.BOOL
api.GetCurrentProcess.argtypes = []
api.GetCurrentProcess.restype = wintypes.HANDLE
api.GetPriorityClass.argtypes = [wintypes.HANDLE]
api.GetPriorityClass.restype = wintypes.DWORD


def ticks(value):
    return (value.dwHighDateTime << 32) | value.dwLowDateTime


def process_cpu(handle):
    created, exited, kernel, user = FT(), FT(), FT(), FT()
    if not api.GetProcessTimes(handle, ctypes.byref(created), ctypes.byref(exited), ctypes.byref(kernel), ctypes.byref(user)):
        raise ctypes.WinError(ctypes.get_last_error())
    return ticks(kernel) + ticks(user)


def system_cpu():
    idle, kernel, user = FT(), FT(), FT()
    if not api.GetSystemTimes(ctypes.byref(idle), ctypes.byref(kernel), ctypes.byref(user)):
        raise ctypes.WinError(ctypes.get_last_error())
    return ticks(kernel) + ticks(user) - ticks(idle)


def main():
    spec = importlib.util.spec_from_file_location('reuse', ROOT / 'tools/measure-reuse-controls.py')
    driver = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(driver)
    assert driver.blocks.sha(EXE) == EXPECTED_SHA
    previous = json.loads((ROOT / 'docs/benchmarks/reuse-controls-20261018-data-proof.json').read_text(encoding='utf-8'))
    assert driver.blocks.snapshot() == previous['files']
    assert abs(process_cpu(api.GetCurrentProcess()) / 1e7 - time.process_time()) < .1
    old_worker = driver.Worker
    rows = []
    with report('block-cpu.jsonl').open('x', encoding='utf-8') as log:
        class ObservedWorker(old_worker):
            def __init__(self, *args, **kwargs):
                super().__init__(*args, **kwargs)
                self.metadata['priority_class_verified'] = api.GetPriorityClass(int(self.process._handle))
                if self.metadata['priority_class_verified'] != 0x20:
                    self.close()
                    raise RuntimeError('Expected unchanged Normal process priority')

            def block(self, command):
                handle = int(self.process._handle)
                begin = time.perf_counter()
                system_before = system_cpu()
                child_before = process_cpu(handle)
                parent_before = process_cpu(api.GetCurrentProcess())
                measured = super().block(command)
                child_s = (process_cpu(handle) - child_before) / 1e7
                parent_s = (process_cpu(api.GetCurrentProcess()) - parent_before) / 1e7
                system_s = (system_cpu() - system_before) / 1e7
                elapsed = time.perf_counter() - begin
                row = dict(id=command['id'], label=command['label'], pid=self.process.pid,
                    utc=datetime.now(timezone.utc).isoformat(), block_wall_s=elapsed,
                    child_cpu_s=child_s, harness_cpu_s=parent_s, system_busy_cpu_s=system_s,
                    child_cpu_to_wall_ratio=child_s / elapsed,
                    other_cpu_logical_cores=(system_s - child_s - parent_s) / elapsed)
                rows.append(row)
                log.write(json.dumps(row) + '\n')
                log.flush()
                return measured

        driver.Worker = ObservedWorker
        sys.argv = ['measure-reuse-controls.py', '--prefix', str(PREFIX),
            '--executable', str(EXE), '--phase', 'normal', '--pattern', 'mixed',
            '--processes', '4', '--repeats', '5', '--affinity', '4', '--get-interleave']
        driver.main()
    assert len(rows) == 72
    power = subprocess.check_output(['powercfg', '/getactivescheme'], text=True).strip()
    values = [r['other_cpu_logical_cores'] for r in rows]
    ratio = [r['child_cpu_to_wall_ratio'] for r in rows]
    summary = dict(executable_sha256=EXPECTED_SHA, logical_processors=os.cpu_count(),
        power_scheme=power, sample_scope='whole block; open/warmup/get/query/full oracle; outside query clocks',
        background_estimate='system busy CPU minus child and harness CPU, divided by wall time; all logical CPUs; not target-core scheduling or I/O attribution; counters not atomic and quantized',
        blocks=len(rows), other_cpu_logical_cores=dict(median=statistics.median(values),
            p95=driver.blocks.percentile(values, 95), maximum=max(values), minimum=min(values)),
        child_cpu_to_wall_ratio=dict(median=statistics.median(ratio), minimum=min(ratio), maximum=max(ratio)))
    driver.blocks.write(report('cpu-summary.json'), summary)
    print('CPU observation', json.dumps(summary), flush=True)


if __name__ == '__main__':
    main()
