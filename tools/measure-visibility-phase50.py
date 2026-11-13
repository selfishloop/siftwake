"""One diagnostic block only; reuse the owned pool lifecycle and oracle."""
import importlib.util
from pathlib import Path
R=Path(__file__).resolve().parent.parent
spec=importlib.util.spec_from_file_location('pool',R/'tools/measure-frunner-key50.py')
pool=importlib.util.module_from_spec(spec);spec.loader.exec_module(pool)
pool.P='visibility-phase50-20261102'
if __name__=='__main__':pool.main(diagnostic_once=True)
