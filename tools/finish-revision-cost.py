"""One-shot continuation; no recurring automation or database copies."""
import argparse,importlib,json,subprocess,sys,time,shutil
from pathlib import Path
b=importlib.import_module('measure-churn')
ROOT,LOGS=b.ROOT,b.LOGS
P='revisions-cost-10k-20261025'

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--resume',action='store_true');args=parser.parse_args()
    proof=LOGS/f'{P}-finish.json';assert not proof.exists() or args.resume
    design=LOGS/f'{P}-compare-fixed-v2-design.json'
    while True:
        try:
            if json.loads(design.read_text(encoding='utf-8')).get('complete'):break
        except (FileNotFoundError,json.JSONDecodeError):pass
        time.sleep(5)
    evidence=dict(status='running',steps=[],protected_before=b.protected())
    if args.resume:
        evidence=json.loads(proof.read_text(encoding='utf-8'));assert evidence['protected_before']==b.protected()
    def step(label,args):
        if any(s['label']==label and s['exit_code']==0 for s in evidence['steps']):return
        logical=label
        if any(s['label']==label for s in evidence['steps']):label=label+'-retry1'
        path=LOGS/f'{P}-{label}.log';assert not path.exists()
        start=time.monotonic()
        with path.open('w',encoding='utf-8') as out:
            r=subprocess.run(args,cwd=ROOT,stdout=out,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW)
        entry=dict(label=label,args=list(map(str,args)),exit_code=r.returncode,wall_s=time.monotonic()-start,log=path.name,sha256=b.sha(path));evidence['steps'].append(entry);b.write(proof,evidence);print(json.dumps(entry),flush=True)
        assert r.returncode==0,entry
        if label!=logical:
            for old in evidence['steps']:
                if old['label']==logical and old['exit_code']!=0:old['superseded_by']=label
            b.write(proof,evidence)
    b.write(proof,evidence)
    step('long',[sys.executable,'-X','utf8','tools/measure-revision-cost.py','--phase','long'])
    state=json.loads((LOGS/f'{P}-slot-fdb.json').read_text())['state']
    step('old-reader',[str(ROOT/'output/churn-fdb-20261024.exe'),'verify','streamed',state])
    step('fmt-apply',['cargo','fmt'])
    step('kill-build-v2',['cargo','build','--release','--example','measure_churn','--bin','fdb'])
    probe=ROOT/'output/revision-cost-kill-20261025-v2.exe';assert not probe.exists();shutil.copyfile(ROOT/'target/release/examples/measure_churn.exe',probe)
    step('kills',[sys.executable,'-X','utf8','tools/kill-revision-cost.py'])
    step('profile-build',['cargo','build','--release','--example','measure_churn','--features','diagnostic-timing'])
    profile=ROOT/'output/revision-cost-after-profile-20261025.exe';assert not profile.exists();shutil.copyfile(ROOT/'target/release/examples/measure_churn.exe',profile)
    step('after-profile',[sys.executable,'-X','utf8','tools/measure-revision-cost.py','--phase','after-profile'])
    for label,args in [
        ('fmt',['cargo','fmt','--check']),('clippy',['cargo','clippy','--all-targets','--','-D','warnings']),
        ('profile-clippy',['cargo','clippy','--all-targets','--features','diagnostic-timing','--','-D','warnings']),
        ('tests',['cargo','test']),('reference-tests',['cargo','test','--no-default-features','--lib']),
        ('harness-tests',['cargo','test','--example','measure_churn'])]:step(label,args)
    evidence['protected_after']=b.protected();assert evidence['protected_before']==evidence['protected_after']
    evidence['status']='complete';b.write(proof,evidence)

if __name__=='__main__':main()
