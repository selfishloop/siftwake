"""Repeat existing witnessed kill windows with final small-WAL snapshot code."""
import importlib,json
k=importlib.import_module('kill-snapshots')
k.P='small-commit-kill-10k-20261026'
k.PROGRAM=k.b.ROOT/'output/small-commit-fdb-20261026-v3.exe'
if __name__=='__main__':
    # Alias only the model descriptor; never copy a database/corpus.
    slot=json.loads((k.b.LOGS/'small-commit-v2-10k-20261026-slot-fdb.json').read_text(encoding='utf-8'))
    k.b.write(k.b.LOGS/'snapshot-10k-20261026-slot-fdb.json',slot)
    k.main()
