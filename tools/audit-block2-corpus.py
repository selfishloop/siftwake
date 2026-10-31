"""Read-only inventory of the existing pool. Never creates document data or DBs."""
import hashlib
import json
import struct
import time
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
POOL = ROOT / 'data/rkyv-corpus-20261009'
OUT = ROOT / 'docs/benchmarks/block2-20261028-corpus-audit.json'


def main():
    if OUT.exists():
        raise RuntimeError('Immutable evidence already exists')
    started = time.monotonic()
    manifest = json.loads((POOL / 'corpus.json').read_text(encoding='utf-8'))
    bodies = []
    for source in manifest['sources']:
        body = (POOL / f"body{source['book_id']}.txt").read_bytes()
        assert hashlib.sha256(body).hexdigest() == source['body_sha256']
        bodies.append(body)
    seen = set()
    prefixes = []
    digest = hashlib.sha256()
    text_bytes = 0
    minimum, maximum = 1 << 32, 0
    with (POOL / 'json.payload-bench').open('rb') as stream:
        header = stream.read(20)
        digest.update(header)
        magic, version, count, crc = struct.unpack('<8sIII', header)
        assert magic == b'PLOADB01' and version == 1
        assert zlib.crc32(header[:16]) == crc
        assert count == manifest['count'] == len(manifest['rows'])
        for i in range(count):
            record = stream.read(16)
            length, checksum, ident, hcrc = struct.unpack('<IIII', record)
            assert i == ident and zlib.crc32(record[:12]) == hcrc
            assert 0 < length < 4 * 1024 * 1024
            payload = stream.read(length)
            assert len(payload) == length and zlib.crc32(payload) == checksum
            digest.update(record)
            digest.update(payload)
            doc = json.loads(payload)['document']
            text = doc['text'].encode('utf-8')
            row = manifest['rows'][i]
            expected = bodies[row['book']][row['start']:row['start'] + row['len']]
            assert text == expected and row['id'] == i
            assert doc['key']['id'] == f'{i:010}'
            fingerprint = hashlib.sha256(text).digest()
            assert fingerprint not in seen, f'Duplicate body at {i}'
            seen.add(fingerprint)
            text_bytes += len(text)
            minimum, maximum = min(minimum, len(text)), max(maximum, len(text))
            if i + 1 in (10_000, 50_000, 100_000):
                prefixes.append(dict(documents=i + 1, unique_bodies=len(seen), text_bytes=text_bytes,
                                     min_text_bytes=minimum, max_text_bytes=maximum))
                print(json.dumps(prefixes[-1]), flush=True)
        assert stream.read(1) == b''
    result = dict(pool=str((POOL / 'json.payload-bench').relative_to(ROOT)),
                  pool_bytes=(POOL / 'json.payload-bench').stat().st_size,
                  pool_sha256=digest.hexdigest().upper(), records=count, prefixes=prefixes,
                  all_record_crc_verified=True, all_bodies_match_existing_book_windows=True,
                  duplicate_bodies=0, independent_source_books=len(bodies),
                  limitation='Overlapping passages from eight books; not 100000 independent real files',
                  generated_or_downloaded_documents=0, elapsed_s=time.monotonic() - started)
    OUT.write_text(json.dumps(result, indent=2), encoding='utf-8')
    print(json.dumps(result), flush=True)


if __name__ == '__main__':
    main()
