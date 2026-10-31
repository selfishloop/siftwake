"""Same balanced4R workload on the volatile active-query summary candidate."""
import importlib,sys
r=importlib.import_module('measure-snapshots')
r.P=r.b.PREFIX='small-commit-snapshot-v4-10k-20261026'
r.b.PROGRAMS['fdb']=r.b.ROOT/'output/small-commit-fdb-20261026-v4.exe'
if __name__=='__main__':
    for batch,pair in [('one',0),('100',0),('100',1),('one',1)]:
        sys.argv=['measure-small-snapshot-v4.py','--engine','fdb','--batch',batch,'--pair',str(pair)];r.main()
