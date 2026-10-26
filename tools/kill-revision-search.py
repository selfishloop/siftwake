"""Reuse the observed recovery probes with a separate evidence prefix."""
import importlib,os
cost=importlib.import_module('kill-revision-cost');b=cost.b;k=cost.k
def main():
    cost.P=b.PREFIX=k.PREFIX='revision-search-kill-10k-20261026'
    b.PROGRAMS['fdb']=b.ROOT/'output/revision-search-after-20261026.exe'
    os.environ['FDB_QUERY_VISIBILITY_PRUNING']='1';os.environ.pop('FDB_COST_LEGACY',None)
    state=b.LOGS/'revision-search-10k-20261026-candidate-state.json'
    protected=b.protected();results=[]
    for mode in ['kill-retire-before','kill-retire-after','kill-cancel']:results.append(cost.barrier(mode,state))
    for stage in ['iterator','flush','merge','cache']:
        r=k.kill_stage(stage,state);assert r['coverage_observed'];results.append(r)
    old=b.PROGRAMS['fdb'];b.PROGRAMS['fdb']=b.ROOT/'output/churn-fdb-20261024.exe'
    compat=b.run('fdb','verify','streamed',state,'old-reader');b.PROGRAMS['fdb']=old
    assert b.protected()==protected
    b.write(b.LOGS/f'{cost.P}-summary.json',dict(results=results,old_reader=compat,protected_unchanged=True,power_loss_claim=False,program=str(old.relative_to(b.ROOT)),program_sha256=b.sha(old)))
if __name__=='__main__':main()
