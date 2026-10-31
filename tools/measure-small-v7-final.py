"""Final bounded-cache correctness fix: same-slot small writes and recovery."""
import importlib,json,os
from pathlib import Path
b=importlib.import_module('measure-churn');P=b.PREFIX='small-commit-v7-10k-20261026'
b.PROGRAMS['fdb']=b.ROOT/'output/small-commit-fdb-20261026-v7.exe'
def main():
    target=b.LOGS/f'{P}-design.json';assert not target.exists()
    slot=json.loads((b.LOGS/'small-commit-snapshot-v6-10k-20261026-slot-fdb.json').read_text(encoding='utf-8'));state=Path(slot['state'])
    d=dict(protected_before=b.protected(),program=str(b.PROGRAMS['fdb'].relative_to(b.ROOT)),sha256=b.sha(b.PROGRAMS['fdb']),completed=[],scope='same existing final10k;final per-entry shadow-cache coverage guard;small96 verification,not a new balanced throughput comparison')
    for mode in ['small-latency','verify']:
        d['completed'].append(b.run('fdb',mode,'streamed',state,'current-slot'));b.write(target,d)
    b.write(b.LOGS/'snapshot-10k-20261026-slot-fdb.json',slot)
    os.environ['FDB_STREAMED_WAL_LAYER']='1'
    k=importlib.import_module('kill-snapshots');k.P='small-commit-kill-v7-10k-20261026';k.PROGRAM=b.PROGRAMS['fdb'];k.main()
    d['protected_after']=b.protected();assert d['protected_before']==d['protected_after'];d['complete']=True;b.write(target,d)
if __name__=='__main__':main()
