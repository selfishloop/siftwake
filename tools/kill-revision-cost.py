"""Observed old recovery windows plus deterministic retirement/cancel barriers."""
import importlib,json,subprocess,time
from pathlib import Path
k=importlib.import_module('measure-churn-kill');b=k.bench
ROOT,LOGS=b.ROOT,b.LOGS
P='revisions-cost-kill-10k-20261025'
DB=ROOT/'data/benchmark-work/fdb'

def barrier(mode,state):
    original=LOGS/f'{P}-{mode}-attempt.json'
    if original.exists():
        previous=json.loads(original.read_text(encoding='utf-8'))
        assert previous['oracle'] and previous['verify_exit_code']==0 and previous['marker_oracle']['all_passed']
        return previous
    label=mode
    if (LOGS/f'{P}-{label}.jsonl').exists():label=mode+'-retry1'
    log=LOGS/f'{P}-{label}.jsonl';assert not log.exists()
    before=k.snapshot();baseline=k.published_sources()
    phase=None;began=time.monotonic()
    with log.open('w',encoding='utf-8') as out,log.with_suffix('.stderr').open('w',encoding='utf-8') as err:
        child=subprocess.Popen([str(b.PROGRAMS['fdb']),mode,'streamed',str(state)],cwd=ROOT,stdout=subprocess.PIPE,stderr=err,text=True,encoding='utf-8',creationflags=subprocess.CREATE_NO_WINDOW)
        try:
            for line in child.stdout:
                out.write(line);out.flush();row=json.loads(line)
                if row['phase'] in ['retire_barrier','cancel_iterator_barrier']:
                    phase=row;break
            assert phase is not None,(mode,child.poll())
            publication=k.published_sources();observed=k.snapshot()
        finally:
            if child.poll() is None:child.kill();child.wait(timeout=10)
    process=dict(mode=mode,exit_code=child.returncode,barrier=phase,elapsed_s=time.monotonic()-began,before=before,files_at_kill=observed,publication_before=baseline,publication_at_kill=publication,coverage='explicit pre/post-install or post-cancellation iterator50 boundary; not power loss')
    b.write(LOGS/f'{P}-{label}-attempt.json',process)
    verified=b.run('fdb','verify','streamed',state,label)
    process['verify_exit_code']=verified['exit_code']
    process['marker_oracle']=k.pending_marker_oracle(state,label)
    process['oracle']=True
    b.write(LOGS/f'{P}-{label}-attempt.json',process)
    return process

def main():
    b.PREFIX=k.PREFIX=P
    b.PROGRAMS['fdb']=ROOT/'output/revision-cost-kill-20261025-v2.exe'
    state=Path(json.loads((LOGS/'revisions-cost-10k-20261025-slot-fdb.json').read_text())['state'])
    protected=b.protected();results=[]
    # These small pure-update runs ensure a genuinely fully-dead source.
    for mode in ['kill-retire-before','kill-retire-after','kill-cancel']:results.append(barrier(mode,state))
    for stage in ['iterator','flush','merge','cache']:
        result=k.kill_stage(stage,state);assert result['coverage_observed'],result;results.append(result)
    assert b.protected()==protected
    programs=[ROOT/'output/revision-cost-kill-20261025.exe',b.PROGRAMS['fdb']]
    b.write(LOGS/f'{P}-summary.json',dict(results=results,power_loss_claim=False,protected_unchanged=True,programs=[dict(path=str(p.relative_to(ROOT)),sha256=b.sha(p)) for p in programs],setup_failure=dict(mode='kill-retire-after',stderr=f'{P}-kill-retire-after.stderr',reason='automatic normal merge consumed setup source; target barrier absent; retry drains ordinary merges first')))

if __name__=='__main__':main()
