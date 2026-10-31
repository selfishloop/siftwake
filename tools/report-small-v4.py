"""Final FDB tables and clearly labelled unchanged earlier SQLite reference."""
import importlib
r=importlib.import_module('report-small-commits')
r.P='small-commit-v4-10k-20261026'
r.SQLITE_PREFIX='small-commit-v2-10k-20261026'
if __name__=='__main__':r.main()
