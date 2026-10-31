"""Final opt-in gate runs, fixed same first10k; unchanged SQLite v2 reference."""
import importlib
r=importlib.import_module('measure-small-v4')
r.P=r.b.PREFIX='small-commit-v6-10k-20261026'
r.b.PROGRAMS['fdb']=r.b.ROOT/'output/small-commit-fdb-20261026-v6.exe'
if __name__=='__main__':r.main()
