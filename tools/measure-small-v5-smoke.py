"""Validate final opt-in API wrapping without repeating completed gate matrices."""
import importlib,json,os
from pathlib import Path
b=importlib.import_module('measure-churn');P=b.PREFIX='small-commit-v5-10k-20261026'
b.PROGRAMS['fdb']=b.ROOT/'output/small-commit-fdb-20261026-v5.exe'
def main():
    target=b.LOGS/f'{P}-design.json';assert not target.exists()
    slot=json.loads((b.LOGS/'small-commit-snapshot-v4-10k-20261026-slot-fdb.json').read_text(encoding='utf-8'));state=Path(slot['state'])
    design=dict(protected_before=b.protected(),program=str(b.PROGRAMS['fdb'].relative_to(b.ROOT)),sha256=b.sha(b.PROGRAMS['fdb']),state=str(state),scope='existing final slot;opt-in flag wrapper+remove bounded fallback Vec;not a new balanced gate A/B',completed=[])
    for mode in ['small-latency','verify']:
        design['completed'].append(b.run('fdb',mode,'streamed',state,'current-slot'));b.write(target,design)
    b.write(b.LOGS/'snapshot-10k-20261026-slot-fdb.json',slot)
    os.environ['FDB_STREAMED_WAL_LAYER']='1'
    k=importlib.import_module('kill-snapshots');k.P='small-commit-kill-v5-10k-20261026';k.PROGRAM=b.PROGRAMS['fdb'];k.main()
    design['protected_after']=b.protected();assert design['protected_before']==design['protected_after'];design['complete']=True;b.write(target,design)
if __name__=='__main__':main()
