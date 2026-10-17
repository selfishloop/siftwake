"""Freeze same-harness readers around a segment-only change; never opens a DB."""
import argparse
import hashlib
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base', required=True)
    parser.add_argument('--name', required=True)
    args = parser.parse_args()
    if not args.name or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.name):
        parser.error('--name must be a simple lowercase artifact name')
    source = ROOT / 'src/segment.rs'
    changed = subprocess.run(['git', 'diff', '--name-only', args.base, '--', 'src',
                              'Cargo.toml', 'Cargo.lock', 'tests/fixtures'], cwd=ROOT,
                             capture_output=True, text=True, check=True).stdout.splitlines()
    allowed = {'src/segment.rs', 'src/segment/grouped_tests.rs', 'src/segment/window_tests.rs'}
    if set(changed) - allowed:
        raise RuntimeError(f'Runtime/format/fixture changes outside this comparison: {changed}')
    current = source.read_bytes()
    baseline = subprocess.run(['git', 'show', f'{args.base}:src/segment.rs'], cwd=ROOT,
                              capture_output=True, check=True).stdout
    backup = ROOT / f'output/{args.name}-segment-backup.rs'
    if backup.exists():
        raise RuntimeError('Preserve previous backup/artifacts; choose an unused name')
    backup.parent.mkdir(exist_ok=True)
    backup.write_bytes(current)

    def build(label):
        for diagnostic in (False, True):
            kind = 'profile' if diagnostic else 'normal'
            examples = ['measure_content_reads'] if diagnostic else ['measure_content_reads', 'measure_interleaved_reads']
            targets = {example: ROOT / f'output/{args.name}-{label}-{example}-{kind}.exe' for example in examples}
            if any(path.exists() for path in targets.values()):
                raise RuntimeError('Refusing to overwrite a frozen executable')
            command = ['cargo', 'build', '--release']
            for example in examples:
                command += ['--example', example]
            if diagnostic:
                command += ['--features', 'diagnostic-timing']
            with (ROOT / f'docs/benchmarks/{args.name}-{label}-{kind}-build.log').open('w', encoding='utf-8') as log:
                subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, check=True,
                               creationflags=subprocess.CREATE_NO_WINDOW)
            for example, target in targets.items():
                target.write_bytes((ROOT / f'target/release/examples/{example}.exe').read_bytes())
                print(target.name, hashlib.sha256(target.read_bytes()).hexdigest(), flush=True)

    try:
        source.write_bytes(baseline)
        build('control')
    finally:
        if source.read_bytes() != baseline:
            raise RuntimeError('External source change detected; original backup preserved, no overwrite')
        source.write_bytes(current)
    assert source.read_bytes() == backup.read_bytes()
    build('candidate')
    print('Candidate source restored exactly; all runtime changes must be confined to segment.rs', flush=True)


if __name__ == '__main__':
    main()
