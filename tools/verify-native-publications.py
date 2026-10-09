"""Read-only byte identity check of matched native final publications."""
import argparse
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('database_prefix', help='prefix followed by -{label}-{run}')
parser.add_argument('--runs', type=int, default=3)
parser.add_argument('--engines', nargs=2, default=['baseline', 'candidate'])
args = parser.parse_args()
assert args.runs > 0 and args.engines[0] != args.engines[1]
comparisons = []
for number in range(args.runs):
    directories = [Path(f'{args.database_prefix}-{label}-{number}') for label in args.engines]
    markers = [max(directory.glob('*.ready')) for directory in directories]
    contents = [marker.read_bytes() for marker in markers]
    assert markers[0].name == markers[1].name and contents[0] == contents[1], 'ready differs'
    manifest = json.loads(contents[0][20:-4])
    hashes = {markers[0].name: hashlib.sha256(contents[0]).hexdigest()}
    for run in manifest['runs']:
        assert run['kind'] in ['Run', 'Legacy']
        assert isinstance(run['id'], int) and 0 <= run['id'] < 2 ** 64
        stem = 'run' if run['kind'] == 'Run' else 'generation'
        for extension in ['fdb', 'idx']:
            name = f'{stem}-{run["id"]:016x}.{extension}'
            values = [hashlib.sha256((directory / name).read_bytes()).hexdigest()
                      for directory in directories]
            assert values[0] == values[1], f'{number}: {name} differs'
            hashes[name] = values[0]
    comparisons.append(dict(pair=number, ready=markers[0].name, sha256=hashes))
print(json.dumps(comparisons, indent=2))
