"""Final version threshold/helper/idle catch-up and process-kill recovery."""
import importlib,json,os
from pathlib import Path
r=importlib.import_module('measure-small-final')
r.P=r.b.PREFIX='small-commit-final-v4-10k-20261026'
r.b.PROGRAMS['fdb']=r.b.ROOT/'output/small-commit-fdb-20261026-v4.exe'
if __name__=='__main__':
    # Existing runner reads this descriptor; it is not a DB copy.
    state=json.loads((r.b.LOGS/'small-commit-v4-10k-20261026-slot-fdb.json').read_text(encoding='utf-8'))
    r.b.write(r.b.LOGS/'small-commit-v2-10k-20261026-slot-fdb.json',state)
    r.main()
    k=importlib.import_module('kill-snapshots');k.P='small-commit-kill-v4-10k-20261026';k.PROGRAM=r.b.PROGRAMS['fdb']
    k.b.write(k.b.LOGS/'snapshot-10k-20261026-slot-fdb.json',state);k.main()
