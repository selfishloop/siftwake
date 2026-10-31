"""Final opt-in threshold/helper/idle and existing process-kill windows."""
import importlib,json,os
r=importlib.import_module('measure-small-final')
r.P=r.b.PREFIX='small-commit-final-v6-10k-20261026'
r.b.PROGRAMS['fdb']=r.b.ROOT/'output/small-commit-fdb-20261026-v6.exe'
if __name__=='__main__':
    slot=json.loads((r.b.LOGS/'small-commit-v6-10k-20261026-slot-fdb.json').read_text(encoding='utf-8'))
    r.b.write(r.b.LOGS/'small-commit-v2-10k-20261026-slot-fdb.json',slot);r.main()
    os.environ['FDB_STREAMED_WAL_LAYER']='1'
    k=importlib.import_module('kill-snapshots');k.P='small-commit-kill-v6-10k-20261026';k.PROGRAM=r.b.PROGRAMS['fdb']
    k.b.write(k.b.LOGS/'snapshot-10k-20261026-slot-fdb.json',slot);k.main()
