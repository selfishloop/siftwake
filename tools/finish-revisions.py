"""One-shot continuation of the current measurement, never a scheduled job."""
import importlib
import json
import subprocess
import time
import sys
from pathlib import Path

bench=importlib.import_module('measure-churn')
ROOT,LOGS=bench.ROOT,bench.LOGS
PYTHON=Path(__import__('sys').executable)
DESIGN=LOGS/'revisions-10k-20261025-v2-design.json'
EVIDENCE=LOGS/'revisions-verification-20261025.json'


def main():
    global EVIDENCE
    resume='--checks-only' in sys.argv
    if resume:EVIDENCE=LOGS/'revisions-verification-resume-20261025.json'
    assert not EVIDENCE.exists(),'Never overwrite a verification run'
    while not json.loads(DESIGN.read_text(encoding='utf-8')).get('complete'):
        time.sleep(5)
    evidence=dict(status='running',steps=[],protected_before=bench.protected(),
                  continuation_of='revisions-verification-20261025.json' if resume else None)
    bench.write(EVIDENCE,evidence)

    def step(label,args):
        log=LOGS/f'revisions-verification-{"resume-" if resume else ""}20261025-{label}.log'
        assert not log.exists()
        at=time.monotonic()
        with log.open('w',encoding='utf-8') as out:
            result=subprocess.run(args,cwd=ROOT,stdout=out,stderr=subprocess.STDOUT,
                                  creationflags=subprocess.CREATE_NO_WINDOW)
        evidence['steps'].append(dict(label=label,args=list(map(str,args)),exit_code=result.returncode,
                                      wall_s=time.monotonic()-at,log=log.name,sha256=bench.sha(log)))
        bench.write(EVIDENCE,evidence)
        print(json.dumps(evidence['steps'][-1]),flush=True)
        if result.returncode:raise RuntimeError(f'Failed {label}: preserve evidence')

    step('fmt-apply',['cargo','fmt'])
    if not resume:
        step('probe-build',['cargo','build','--release','--example','measure_churn','--bin','fdb'])
        probe=ROOT/'output/revisions-probe-20261025.exe'
        assert not probe.exists()
        __import__('shutil').copyfile(ROOT/'target/release/examples/measure_churn.exe',probe)
        state=json.loads((LOGS/'revisions-10k-20261025-v2-slot-fdb.json').read_text())['state']
        bench.PREFIX='revisions-probes-10k-20261025'
        bench.PROGRAMS['fdb']=probe
        evidence['probe_program']=dict(path=str(probe.relative_to(ROOT)),sha256=bench.sha(probe))
        for mode in ['revision-reads','revision-write','verify']:
            evidence['steps'].append(bench.run('fdb',mode,'streamed',state,'background'))
            bench.write(EVIDENCE,evidence)
        step('kills',[str(PYTHON),'-X','utf8','tools/kill-revisions.py'])
    for label,args in [
        ('fmt',['cargo','fmt','--check']),
        ('clippy',['cargo','clippy','--all-targets','--','-D','warnings']),
        ('tests',['cargo','test']),
        ('reference-lib',['cargo','test','--no-default-features','--lib']),
        ('harness',['cargo','test','--example','measure_churn']),
        ('sqlite-fmt',['cargo','fmt','--manifest-path','tools/native_sqlite/Cargo.toml','--check']),
        ('sqlite-clippy',['cargo','clippy','--manifest-path','tools/native_sqlite/Cargo.toml','--all-targets','--','-D','warnings']),
        ('sqlite-harness',['cargo','test','--manifest-path','tools/native_sqlite/Cargo.toml','--bin','content_churn'])]:
        step(label,args)
    evidence['protected_after']=bench.protected()
    assert evidence['protected_after']==evidence['protected_before']
    step('report',[str(PYTHON),'-X','utf8','tools/report-revisions.py'])
    evidence['status']='complete'
    bench.write(EVIDENCE,evidence)


if __name__=='__main__':main()
