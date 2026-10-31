"""Repeat all seven witnessed windows on the release-counter fix executable."""
import importlib

k = importlib.import_module('kill-snapshots')
k.P = 'snapshot-kill-v3-10k-20261026'
k.PROGRAM = k.b.ROOT/'output/snapshot-fdb-20261026-v3.exe'

if __name__ == '__main__':
    k.main()
