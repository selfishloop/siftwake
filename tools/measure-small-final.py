"""Final helper, idle catch-up and threshold coverage, existing fixed FDB slot."""
import importlib,json,os
from pathlib import Path
b=importlib.import_module('measure-churn');P=b.PREFIX='small-commit-final-10k-20261026'
b.PROGRAMS['fdb']=b.ROOT/'output/small-commit-fdb-20261026-v3.exe'
def main():
    target=b.LOGS/f'{P}-design.json';assert not target.exists()
    for key in ['FDB_BENCH_SNAPSHOTS','FDB_COST_LEGACY','FDB_QUERY_VISIBILITY_PRUNING']:os.environ.pop(key,None)
    pointer=json.loads((b.LOGS/'small-commit-v2-10k-20261026-slot-fdb.json').read_text(encoding='utf-8'));state=Path(pointer['state'])
    design=dict(protected_before=b.protected(),program=str(b.PROGRAMS['fdb'].relative_to(b.ROOT)),sha256=b.sha(b.PROGRAMS['fdb']),state=str(state),completed=[])
    for mode in ['small-boundary','small-chunks','small-catchup','verify']:
        design['completed'].append(b.run('fdb',mode,'streamed',state,'current-slot'));b.write(target,design)
    design['protected_after']=b.protected();assert design['protected_after']==design['protected_before'];design['complete']=True;b.write(target,design)
if __name__=='__main__':main()
