"""Recovery/compatibility for the final single100-write pair, same fixed slot."""
import importlib

k=importlib.import_module('kill-write-hardening')
k.P='hundred-pair-kill-10k-20261027'
k.PROGRAM=k.b.ROOT/'output/wal-profile-fdb-20261027-v1.exe'
k.POINTER=k.b.LOGS/'hundred-pair-10k-20261027-slot-fdb.json'
k.THIRTY_SCOPE='previous30round gate accepted; this step measured one matched128x100control/load pair perbackend'

if __name__=='__main__':k.main()
