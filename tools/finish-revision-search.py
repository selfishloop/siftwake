"""Sequential measurement then verification; never starts snapshot work."""
import importlib,json,subprocess,sys,os
b=importlib.import_module('measure-churn');ROOT,LOGS=b.ROOT,b.LOGS
P='revision-search-10k-20261026'
def main():
    file=LOGS/f'{P}-checks.json';assert not file.exists()
    proof=dict(status='running',steps=[],protected_before=b.protected())
    def step(label,args):
        log=LOGS/f'{P}-{label}.log';assert not log.exists()
        with log.open('w',encoding='utf-8') as out:r=subprocess.run(args,cwd=ROOT,stdout=out,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW)
        entry=dict(label=label,args=args,exit_code=r.returncode,log=log.name,sha256=b.sha(log));proof['steps'].append(entry);b.write(file,proof);print(json.dumps(entry),flush=True);assert r.returncode==0,entry
    step('measure',[sys.executable,'-X','utf8','tools/measure-revision-search.py'])
    os.environ['FDB_QUERY_VISIBILITY_PRUNING']='1'
    step('kill',[sys.executable,'-X','utf8','tools/kill-revision-search.py'])
    for label,args in [('fmt',['cargo','fmt','--check']),('clippy',['cargo','clippy','--all-targets','--','-D','warnings']),('profile-clippy',['cargo','clippy','--all-targets','--features','diagnostic-timing','--','-D','warnings']),('tests',['cargo','test']),('no-default',['cargo','test','--no-default-features','--lib']),('harness-tests',['cargo','test','--example','measure_churn'])]:step(label,args)
    proof['protected_after']=b.protected();assert proof['protected_before']==proof['protected_after'];proof['status']='complete';b.write(file,proof)
if __name__=='__main__':main()
