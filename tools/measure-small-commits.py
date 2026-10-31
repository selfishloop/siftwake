"""Same first10k; balanced FDB/SQLite, bounded fixed slots, no corpus preparation."""
import importlib,json,os
b=importlib.import_module('measure-churn')
P=b.PREFIX='small-commit-10k-20261026'
b.PROGRAMS={'fdb':b.ROOT/'output/small-commit-fdb-20261026-v1.exe',
            'sqlite':b.ROOT/'output/small-commit-sqlite-20261026-v1.exe'}
def main():
    target=b.LOGS/f'{P}-design.json';assert not target.exists()
    for name in ['FDB_BENCH_SNAPSHOTS','FDB_COST_LEGACY','FDB_RECLAMATION_CONTROL','FDB_QUERY_VISIBILITY_PRUNING']:os.environ.pop(name,None)
    design=dict(protected_before=b.protected(),programs={k:dict(path=str(v.relative_to(b.ROOT)),sha256=b.sha(v)) for k,v in b.PROGRAMS.items()},completed=[],cache='warm OS,strict FDB,SQLite FULLWAL/FTS5trigram same config',scope='existing first10k,balanced engine order,96 mixed single changes;dirty500 in20-op chunks;no DB copies')
    b.write(target,design)
    for pair,order in enumerate([['fdb','sqlite'],['sqlite','fdb']]):
        for engine in order:
            label=f'r{pair}';state=b.LOGS/f'{P}-{label}-{engine}-state.json'
            b.reset(engine);b.write(b.LOGS/f'{P}-slot-{engine}.json',dict(state=str(state),label=label))
            for mode in ['seed','small-open','small-latency','verify','small-dirty','small-open','verify']:
                mode_label=label+('-dirty' if mode=='small-open' and any(x['engine']==engine and x['mode']=='small-dirty' and x['log'].startswith(f'{P}-{label}') for x in design['completed']) else '-final' if mode=='verify' and any(x['engine']==engine and x['mode']=='small-dirty' and x['log'].startswith(f'{P}-{label}') for x in design['completed']) else '')
                design['completed'].append(b.run(engine,mode,'streamed',state,mode_label));b.write(target,design)
    design['protected_after']=b.protected();assert design['protected_before']==design['protected_after'];design['complete']=True;b.write(target,design)
if __name__=='__main__':main()
