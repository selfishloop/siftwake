"""Repeat recovery/compatibility after the new30round step, same fixed slot."""
import importlib

k=importlib.import_module('kill-write-hardening')
k.P='reclaim-idle30-kill-10k-20261027'
k.PROGRAM=k.b.ROOT/'output/reclaim-idle30-fdb-20261027-v1.exe'
k.POINTER=k.b.LOGS/'reclaim-idle30-10k-20261027-slot-fdb.json'
k.THIRTY_SCOPE='30round logs measured before these additional recovery probes; oracle ledger is the current fixed slot, not an archived DB copy'

if __name__=='__main__':k.main()
