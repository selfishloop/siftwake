"""Freeze strict-open benchmark workers with/without the current search change.

Only the task-owned segment source is temporarily replaced; try/finally restores
its exact bytes. No database/corpus operations or worktrees are involved.
"""
import hashlib
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
source = ROOT / 'src/segment.rs'
current = source.read_bytes()
baseline = subprocess.run(['git', 'show', 'bf879b6:src/segment.rs'], cwd=ROOT,
                          check=True, capture_output=True).stdout
backup = ROOT / 'output/search-worker-segment-backup-20261017.rs'
backup.parent.mkdir(exist_ok=True)
if backup.exists():
    raise RuntimeError('Preserve the previous source backup')
backup.write_bytes(current)


def build(label):
    target = ROOT / f'output/search-worker-{label}-20261017.exe'
    if target.exists():
        raise RuntimeError(f'Preserve frozen executable: {target}')
    subprocess.run(['cargo', 'build', '--release', '--example', 'measure_interleaved_reads'],
                   cwd=ROOT, check=True, creationflags=subprocess.CREATE_NO_WINDOW)
    target.write_bytes((ROOT / 'target/release/examples/measure_interleaved_reads.exe').read_bytes())
    print(label, hashlib.sha256(target.read_bytes()).hexdigest(), flush=True)


try:
    source.write_bytes(baseline)
    build('control')
finally:
    if source.read_bytes() != baseline:
        raise RuntimeError('Segment source changed externally; backup preserved, no overwrite')
    source.write_bytes(current)
assert source.read_bytes() == backup.read_bytes()
build('isolated')
print('Candidate source restored exactly; no database files touched', flush=True)
