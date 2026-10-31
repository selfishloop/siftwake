"""Balanced4R matrix with explicit final opt-in flag; no corpus preparation."""
import importlib,sys,os
r=importlib.import_module('measure-snapshots')
r.P=r.b.PREFIX='small-commit-snapshot-v6-10k-20261026'
r.b.PROGRAMS['fdb']=r.b.ROOT/'output/small-commit-fdb-20261026-v6.exe'
if __name__=='__main__':
    # The parent runner clears unrelated experiment flags, not this API opt-in.
    os.environ['FDB_STREAMED_WAL_LAYER']='1'
    for batch,pair in [('one',0),('100',0),('100',1),('one',1)]:
        sys.argv=['measure-small-snapshot-v6.py','--engine','fdb','--batch',batch,'--pair',str(pair)];r.main()
