"""Repeat balanced small-write gates after the measured WAL-reader-open fix."""
import importlib
r=importlib.import_module('measure-small-commits')
r.P=r.b.PREFIX='small-commit-v2-10k-20261026'
r.b.PROGRAMS['fdb']=r.b.ROOT/'output/small-commit-fdb-20261026-v2.exe'
if __name__=='__main__':r.main()
