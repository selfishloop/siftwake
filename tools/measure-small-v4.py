"""Final FDB-only gate repeats; unchanged SQLite v2 reference retained separately."""
import importlib,json,os
b=importlib.import_module('measure-churn');P=b.PREFIX='small-commit-v4-10k-20261026'
b.PROGRAMS['fdb']=b.ROOT/'output/small-commit-fdb-20261026-v4.exe'
def main():
    target=b.LOGS/f'{P}-design.json';assert not target.exists()
    for name in ['FDB_BENCH_SNAPSHOTS','FDB_COST_LEGACY','FDB_RECLAMATION_CONTROL','FDB_QUERY_VISIBILITY_PRUNING']:os.environ.pop(name,None)
    design=dict(protected_before=b.protected(),program=str(b.PROGRAMS['fdb'].relative_to(b.ROOT)),sha256=b.sha(b.PROGRAMS['fdb']),completed=[],sqlite_reference='small-commit-v2-10k-20261026 two unchanged native SQLite runs;not a fresh same-order FDB-v4/SQLite A/B',cache='warm OS,strict FDB;no CPU/power/background isolation')
    for pair in [0,1]:
        label=f'r{pair}';state=b.LOGS/f'{P}-{label}-fdb-state.json';b.reset('fdb');b.write(b.LOGS/f'{P}-slot-fdb.json',dict(state=str(state),label=label))
        for mode,tag in [('seed',''),('small-open',''),('small-latency',''),('verify',''),('small-dirty',''),('small-open','-dirty'),('verify','-final')]:
            design['completed'].append(b.run('fdb',mode,'streamed',state,label+tag));b.write(target,design)
    design['protected_after']=b.protected();assert design['protected_before']==design['protected_after'];design['complete']=True;b.write(target,design)
if __name__=='__main__':main()
