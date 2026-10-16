"""Build a matched query control, restoring the current dirty source exactly."""
import shutil
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parent.parent
path = root / 'src/index.rs'
original = path.read_bytes()
source = original.decode('utf-8').replace('\r\n', '\n')
source = source.replace(
    '    // Scope is owned once per dictionary, not once per gram or update probe.\n'
    '    pub grams: BTreeMap<String, BTreeMap<String, Posting>>,',
    '    pub grams: BTreeMap<(String, String), Posting>,')
source = source.replace(
    '        for (scope, grams) in &self.grams {\n'
    '            for (gram, posting) in grams {\n'
    '                emit(Term::Gram(scope.clone(), gram.clone()), posting)?;\n'
    '            }\n'
    '        }',
    '        for ((scope, gram), posting) in &self.grams {\n'
    '            emit(Term::Gram(scope.clone(), gram.clone()), posting)?;\n'
    '        }')
start = source.index('        let scope_grams = if add {')
end = source.index('        if add {\n            #[cfg(feature = "experimental-shared-delta-text")]', start)
source = source[:start] + '''        for gram in title_grams.union(&body.grams) {
            change(
                &mut self.grams,
                (key.scope.clone(), gram.clone()),
                &posting_key,
                add,
            );
        }
''' + source[end:]
source = source.replace('.map(|g| self.grams.get(scope).and_then(|grams| grams.get(&g)))',
                        '.map(|g| self.grams.get(&(scope.to_owned(), g)))')
source = source.replace('.chain(index.grams.values().flat_map(BTreeMap::values))',
                        '.chain(index.grams.values())')
assert 'pub grams: BTreeMap<(String, String), Posting>' in source
target = root / 'output/delta-dictionary-query-control-20261013.exe'
assert not target.exists(), 'Preserve existing control artifact'
try:
    path.write_text(source, encoding='utf-8', newline='\n')
    with (root / 'docs/benchmarks/delta-dictionary-query-control-build.log').open('w') as log:
        subprocess.run(['cargo', 'build', '--release', '--example', 'measure_delta_queries',
                        '--features', 'experimental-merge-postings,experimental-delta-bitmaps'],
                       cwd=root, stdout=log, stderr=subprocess.STDOUT, check=True)
    shutil.copyfile(root / 'target/release/examples/measure_delta_queries.exe', target)
finally:
    path.write_bytes(original)
assert path.read_bytes() == original
print('Control saved; current dirty source restored byte-for-byte.')
