"""Rebuild final derived tables and audit only; never rerun DB workloads."""
import importlib
if __name__=='__main__':
    r=importlib.import_module('report-small-commits');r.P='small-commit-v6-10k-20261026';r.SQLITE_PREFIX='small-commit-v2-10k-20261026';r.main()
    r=importlib.import_module('report-small-concurrent');r.P='small-commit-snapshot-v6-10k-20261026';r.main()
    importlib.import_module('audit-small-commits').main()
