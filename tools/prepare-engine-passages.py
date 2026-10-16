"""New isolated benchmark source in the existing FDBLOG01 format, not a migration.
Never touches repository fixtures or existing databases. Engine validates it on open.
"""
import json
import os
import struct
import sys
import zlib
from pathlib import Path

corpus = Path('data/rkyv-corpus-20261009')
target = Path(sys.argv[1] if len(sys.argv) > 1 else 'data/engine-fold-v2-20261011')
target.mkdir()  # Refuse any existing directory.
source = target / 'generation-0000000000000001.fdb'
with (corpus/'json.payload-bench').open('rb') as payload, source.open('xb') as output:
    header = payload.read(20)
    assert header[:8] == b'PLOADB01' and struct.unpack_from('<II', header, 8) == (1, 100_000)
    assert zlib.crc32(header[:16]) == struct.unpack_from('<I', header, 16)[0]
    output.write(b'FDBLOG01')
    for first in range(0, 100_000, 500):
        changes = []
        for id in range(first, first+500):
            record = payload.read(16)
            length, checksum, ordinal, header_crc = struct.unpack('<IIII', record)
            assert ordinal == id and zlib.crc32(record[:12]) == header_crc
            data = payload.read(length)
            assert len(data) == length and zlib.crc32(data) == checksum
            changes.append(b'{"kind":"put","stored":'+data+b'}')
        body = b'{"checkpoint":true,"changes":['+b','.join(changes)+b']}'
        assert len(body) <= 64*1024*1024
        # A multi-frame checkpoint has one shared sequence, not WAL sequences.
        frame = b'TXN1'+struct.pack('<IQI', len(body), 200, zlib.crc32(body))
        output.write(frame+struct.pack('<I', zlib.crc32(frame))+body+b'FDBDONE1')
    assert not payload.read(1)
    output.flush()
    os.fsync(output.fileno())
marker = b'FDBREADY'+struct.pack('<QQ', 1, source.stat().st_size)
(target/'generation-0000000000000001.ready').write_bytes(marker+struct.pack('<I', zlib.crc32(marker)))
if '--source-only' in sys.argv:
    print(json.dumps({'source':str(source),'count':100000,'bytes':source.stat().st_size}))
    sys.exit(0)

manifest = json.loads((corpus/'corpus.json').read_text(encoding='utf-8'))
books = [(corpus/f"body{s['book_id']}.txt").read_bytes() for s in manifest['sources']]
translation = str.maketrans({'I':'i','İ':'i','ı':'i','Ç':'c','ç':'c','Ğ':'g','ğ':'g','Ö':'o','ö':'o','Ş':'s','ş':'s','Ü':'u','ü':'u'})
queries = [{'name':q['name'], 'text':q['text'], 'ids':[]} for q in manifest['queries']]
for row in manifest['rows']:
    text = books[row['book']][row['start']:row['start']+row['len']].decode('utf-8').translate(translation).lower()
    title = f"passage {row['id']:010}"
    for query in queries:
        if all(term in title or term in text for term in query['text'].split()):
            query['ids'].append(row['id'])
for query, expected in zip(queries, manifest['queries']):
    assert len(query['ids']) == expected['total']
    assert query['ids'][:50] == [r['id'] for r in expected['page']]
(target/'oracle.json').write_text(json.dumps(queries, separators=(',', ':')), encoding='utf-8')
print(json.dumps({'source':str(source), 'count':100000, 'bytes':source.stat().st_size,
                  'format':'existing FDBLOG01; canonical JSON payload retained',
                  'queries':[{k:v for k,v in q.items() if k!='ids'} | {'total':len(q['ids'])} for q in queries]}))
