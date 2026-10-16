"""Fresh benchmark source from canonical OFF1 export, not measured ingest.
Never edits existing DBs or fixtures. Same single-checkpoint layout as JSON corpus.
"""
from pathlib import Path
import json
import os
import shutil
import struct
import zlib

root = Path('data/rkyv-corpus-20261009')
target = Path('data/native-general-long-binary-20261011')
target.mkdir()
source = target/'generation-0000000000000001.fdb'
with (root/'binary.payload-bench').open('rb') as payload, source.open('xb') as output:
    header = payload.read(20)
    assert header[:8] == b'PLOADB01' and struct.unpack_from('<II', header, 8) == (3, 100000)
    assert zlib.crc32(header[:16]) == struct.unpack_from('<I', header, 16)[0]
    output.write(b'FDBLOG04')
    for first in range(0, 100000, 500):
        body = bytearray(b'\x01'+struct.pack('<I',500))
        for ordinal in range(first, first+500):
            record = payload.read(16)
            length, checksum, id, header_crc = struct.unpack('<IIII',record)
            assert id == ordinal and zlib.crc32(record[:12]) == header_crc
            data = payload.read(length)
            assert len(data) == length and zlib.crc32(data) == checksum and data[:4] == b'OFF1'
            body.extend(b'\x00'+struct.pack('<I', length)+data)
        assert len(body) <= 64*1024*1024
        frame = b'TXN2'+struct.pack('<IQI',len(body),200,zlib.crc32(body))
        output.write(frame+struct.pack('<I',zlib.crc32(frame))+body+b'FDBDONE1')
    assert not payload.read(1)
    output.flush()
    os.fsync(output.fileno())
marker = b'FDBREADY'+struct.pack('<QQ',1,source.stat().st_size)
(target/'generation-0000000000000001.ready').write_bytes(marker+struct.pack('<I',zlib.crc32(marker)))
shutil.copy2('data/engine-fold-v2-20261011/oracle.json', target/'oracle.json')
print(json.dumps({'source':str(source),'bytes':source.stat().st_size,'documents':100000,
                  'note':'unmeasured source preparation; native builder/open validates source and produces SEG04'}))
