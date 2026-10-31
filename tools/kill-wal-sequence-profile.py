"""Recovery/compatibility for the per-sequence profiler step, trace off by default."""
import importlib

k=importlib.import_module('kill-write-hardening')
k.P='wal-sequence-kill-10k-20261027'
k.PROGRAM=k.b.ROOT/'output/wal-profile-fdb-20261027-v1.exe'
k.POINTER=k.b.LOGS/'wal-qualified-10k-20261027-slot-fdb.json'
k.THIRTY_SCOPE='previous30round gate accepted; this step added only opt-in volatile trace, no handle optimization/format changes'

if __name__=='__main__':k.main()
