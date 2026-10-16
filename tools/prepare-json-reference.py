"""Isolated default-build JSON control, keeping current fold/query/maintenance code.
No checkout reset and no data migration. Refuse to overwrite the snapshot.
"""
from pathlib import Path
import difflib
import hashlib
import json
import shutil

root = Path('output/native-general-json-reference-20261011')
root.mkdir()
for name in ['Cargo.toml', 'Cargo.lock']:
    shutil.copy2(name, root/name)
for name in ['src', 'examples', 'tools/bench_support']:
    shutil.copytree(name, root/name)
patches = []

def edit(name, transform):
    path = root/name
    original = path.read_text(encoding='utf-8')
    changed = transform(original)
    assert original != changed, name
    path.write_text(changed, encoding='utf-8')
    patches.extend(difflib.unified_diff(original.splitlines(True), changed.splitlines(True),
                                      fromfile='native/'+name, tofile='json-control/'+name))

def frame(s):
    start = s.index('    #[cfg(feature = "diagnostic-memory")]', s.index('pub(crate) fn encode('))
    end = s.index('\n}\n', start)
    return s[:start]+'''    let mut bytes = vec![0; HEADER];
    let mut sink = crate::bounded::Sink::new(&mut bytes, MAX_FRAME);
    let encoded = serde_json::to_writer(&mut sink, batch);
    if sink.exceeded { return Err(Error::Invalid("transaction exceeds 64 MiB".into())); }
    if let Some(error) = sink.failure.take() { return Err(Error::Io(error)); }
    encoded?;
    let len = sink.written;
    bytes[..4].copy_from_slice(b"TXN1");
    bytes[4..8].copy_from_slice(&(len as u32).to_le_bytes());
    bytes[8..16].copy_from_slice(&sequence.to_le_bytes());
    let checksum = crc(&bytes[HEADER..]);
    bytes[16..20].copy_from_slice(&checksum.to_le_bytes());
    let checksum = crc(&bytes[..20]);
    bytes[20..24].copy_from_slice(&checksum.to_le_bytes());
    bytes.extend_from_slice(FOOTER);
    Ok(bytes)'''+s[end:]

edit('src/format.rs', frame)
edit('src/manifest.rs', lambda s: s.replace('MAGIC_V4', 'MAGIC_V3'))
edit('src/engine.rs', lambda s: s.replace('MAGIC_V4','MAGIC_V3')
     .replace('crate::binary_batch::entry_len(&change)?','serde_json::to_vec(&change)?.len()')
     .replace('crate::payload::encoded_len(&stored.document)?','serde_json::to_vec(&stored)?.len()')
     .replace('crate::payload::encoded_len(&old.document)?','serde_json::to_vec(old)?.len()'))
edit('src/lib.rs', lambda s: s.replace('payload::encoded_len(doc)?','serde_json::to_vec(doc)?.len()'))
edit('src/segment.rs', lambda s: s.replace('b"FDBSEG04"','b"FDBSEG03"').replace('b"FDBEND04"','b"FDBEND03"')
     .replace('let binary_payload = &header[..8] == MAGIC;', 'let binary_payload = false;')
     .replace('let bytes = crate::payload::encode(stored)?;', 'let bytes = serde_json::to_vec(stored)?;'))
(root/'reference.patch').write_text(''.join(patches), encoding='utf-8')
manifest = {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in root.rglob('*') if p.is_file()}
(root/'snapshot-sha256.json').write_text(json.dumps(manifest, indent=2), encoding='utf-8')
print(json.dumps({'snapshot':str(root), 'configuration':'default features only; reconstructed JSON control, not an old release'}))
