"""One-shot continuation of this task after its existing measurement process exits."""
import argparse
import ctypes
import json
import subprocess
import sys
import time
from pathlib import Path

ROOT=Path(__file__).resolve().parent.parent
LOGS=ROOT/'docs/benchmarks'
STATE=LOGS/'churn-finalizer-20261025.json'


def write(value):
    STATE.write_text(json.dumps(value,indent=2),encoding='utf-8')


def step(name,command,result):
    out=LOGS/f'churn-finalizer-20261025-{name}.log'
    if out.exists():
        raise RuntimeError(f'Preserve existing step {name}')
    start=time.monotonic()
    with out.open('w',encoding='utf-8') as stream:
        child=subprocess.run(command,cwd=ROOT,stdout=stream,stderr=subprocess.STDOUT,
                             creationflags=subprocess.CREATE_NO_WINDOW)
    row=dict(name=name,command=command,exit_code=child.returncode,wall_s=time.monotonic()-start,log=out.name)
    result['steps'].append(row)
    write(result)
    return child.returncode==0


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--main-pid',type=int,required=True)
    args=parser.parse_args()
    if STATE.exists():raise RuntimeError('Existing finalizer state; inspect instead of starting duplicate')
    kernel=ctypes.WinDLL('kernel32',use_last_error=True)
    kernel.OpenProcess.argtypes=[ctypes.c_uint32,ctypes.c_int,ctypes.c_uint32]
    kernel.OpenProcess.restype=ctypes.c_void_p
    kernel.GetExitCodeProcess.argtypes=[ctypes.c_void_p,ctypes.POINTER(ctypes.c_uint32)]
    kernel.CloseHandle.argtypes=[ctypes.c_void_p]
    handle=kernel.OpenProcess(0x1000,False,args.main_pid)
    if not handle:raise ctypes.WinError(ctypes.get_last_error())
    result=dict(main_pid=args.main_pid,status='waiting_for_existing_measurement',steps=[],new_benchmark_runs=False)
    write(result)
    try:
        while True:
            code=ctypes.c_uint32()
            assert kernel.GetExitCodeProcess(handle,ctypes.byref(code))
            if code.value!=259:break
            time.sleep(5)
    finally:kernel.CloseHandle(handle)
    result['main_exit_code']=code.value
    result['status']='finalizing'
    write(result)
    design_path=LOGS/'churn-concurrent-10k-20261024-all-design.json'
    design=json.loads(design_path.read_text(encoding='utf-8'))
    complete=code.value==0 and design.get('protected_unchanged') is True
    result['ordinary_measurements_complete']=complete
    if complete:
        owner=ROOT/'data/benchmark-work/.fdb-benchmark-owned'
        assert owner.read_text(encoding='utf-8-sig').strip()=='FDB benchmark workspace v1'
        step('process-kill',[sys.executable,'-X','utf8','tools/measure-churn-kill.py'],result)
    step('summary',[sys.executable,'-X','utf8','tools/summarize-churn.py'],result)
    step('report',[sys.executable,'-X','utf8','tools/report-churn.py'],result)
    for name,command in [
        ('root-fmt',['cargo','fmt','--check']),
        ('sqlite-fmt',['cargo','fmt','--manifest-path','tools/native_sqlite/Cargo.toml','--check']),
        ('root-clippy',['cargo','clippy','--all-targets','--','-D','warnings']),
        ('sqlite-clippy',['cargo','clippy','--manifest-path','tools/native_sqlite/Cargo.toml','--all-targets','--','-D','warnings']),
        ('fdb-harness-test',['cargo','test','--example','measure_churn']),
        ('sqlite-harness-test',['cargo','test','--manifest-path','tools/native_sqlite/Cargo.toml','--bin','content_churn']),
    ]:step(name,command,result)
    result['status']='complete' if complete and all(s['exit_code']==0 for s in result['steps']) else 'needs_review'
    result['no_engine_optimization_or_format_change']=True
    write(result)


if __name__=='__main__':
    try:main()
    except Exception as error:
        if STATE.exists():
            result=json.loads(STATE.read_text());result['status']='failed';result['error']=repr(error);write(result)
        raise
