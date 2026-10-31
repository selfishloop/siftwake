"""Read-only boundary/metadata I/O probe, NOT a functional FDB fast-open API."""
import ctypes
import json
import struct
import time
import zlib
from pathlib import Path

ROOT=Path(__file__).resolve().parent.parent
DB=ROOT/'data/benchmark-work/fdb'
OUT=ROOT/'docs/benchmarks/block2-query-open-50k-20261028-boundary-probe-v1.json'


def crc(data):
    return zlib.crc32(data)


def probe():
    started=time.perf_counter()
    kernel=ctypes.WinDLL('kernel32',use_last_error=True)
    create=kernel.CreateFileW
    create.argtypes=[ctypes.c_wchar_p,ctypes.c_uint32,ctypes.c_uint32,ctypes.c_void_p,ctypes.c_uint32,ctypes.c_uint32,ctypes.c_void_p]
    create.restype=ctypes.c_void_p
    close=kernel.CloseHandle
    close.argtypes=[ctypes.c_void_p]
    # Existing LOCK, no sharing: prevents a writer/GC owner during this probe.
    handle=create(str(DB/'LOCK'),0x80000000,0,None,3,0x80,None)
    if handle==ctypes.c_void_p(-1).value:
        raise ctypes.WinError(ctypes.get_last_error())
    read_bytes=0
    opened=1
    try:
        candidates=sorted(DB.glob('generation-????????????????.ready'))
        ready=candidates[-1]
        data=ready.read_bytes()
        read_bytes+=len(data)
        opened+=1
        assert data[:8]==b'FDBMAN03'
        length=int.from_bytes(data[16:20],'little')
        assert length+24==len(data) and crc(data[:-4])==int.from_bytes(data[-4:],'little')
        manifest=json.loads(data[20:-4])
        assert manifest['version']==3
        assert (DB/f"generation-{manifest['active']['id']:016x}.fdb").stat().st_size==manifest['active']['end'],'clean WAL required'
        after_manifest=time.perf_counter()
        for is_run,source in [*((True,s) for s in manifest['runs']),(False,manifest['active'])]:
            if is_run:
                assert source['kind']=='Run'
            path=DB/f"{'run' if is_run else 'generation'}-{source['id']:016x}.fdb"
            assert path.parent==DB and not path.is_symlink()
            assert path.stat().st_size==source['end']
            with path.open('rb',buffering=0) as stream:
                head=stream.read(8)
                stream.seek(-8,2)
                tail=stream.read(8)
            opened+=1
            read_bytes+=16
            assert head==b'FDBLOG04' and tail==b'FDBDONE1'
            if not is_run:
                continue
            index=path.with_suffix('.idx')
            size=index.stat().st_size
            with index.open('rb',buffering=0) as stream:
                head=stream.read(28)
                stream.seek(-32,2)
                foot=stream.read(32)
            opened+=1
            read_bytes+=60
            assert head[:8]==b'FDBSEG05' and foot[:8]==b'FDBEND05'
            assert crc(foot[:28])==int.from_bytes(foot[28:],'little')
            seq,end,checksum=struct.unpack('<QQI',head[8:])
            assert (seq,end,checksum)==(source['sequence'],source['end'],source['checksum'])
            offset,length=struct.unpack('<QQ',foot[8:24])
            assert offset>=28 and offset+length==size-32
            # Existing MET04 full checksum is cheap; no SEG05 record comparison.
            met=index.with_suffix('.records04.idx').read_bytes()
            opened+=1
            read_bytes+=len(met)
            assert met[:8]==b'FDBMET04' and met[-32:-24]==b'FDBMEND4'
            assert crc(met[-32:-4])==int.from_bytes(met[-4:],'little')
            assert crc(met[:-32])==int.from_bytes(met[-8:-4],'little')
            assert struct.unpack('<QIQ',met[8:28])==(size,crc(foot),seq)
            root_offset,root_len=struct.unpack('<QQ',met[-24:-8])
            assert root_offset>=32 and root_offset+root_len==len(met)-32
            roots=json.loads(met[root_offset:root_offset+root_len])
            count=int.from_bytes(met[28:32],'little')
            assert len(roots)==(count+64*32-1)//(64*32)
        return dict(elapsed_ms=(time.perf_counter()-started)*1000,
                    lock_inventory_manifest_ms=(after_manifest-started)*1000,
                    logical_read_bytes=read_bytes,file_opens=opened,runs=len(manifest['runs']),
                    manifest=ready.name,source_full_crc_validated=False,
                    index_full_crc_validated=False,segment_record_metadata_compared=False,
                    database_instance_constructed=False,query_executed=False,
                    cache='No OS eviction; repeated boundary reads only',
                    meaning='Boundary I/O feasibility only, not usable DB open time or a passed100ms gate')
    finally:
        assert close(handle)


def main():
    assert not OUT.exists(),'Keep prior evidence'
    samples=[probe() for _ in range(5)]
    OUT.write_text(json.dumps(dict(samples=samples,fast_open_implemented=False),indent=2),encoding='utf-8')
    print(json.dumps(samples))


if __name__=='__main__':
    main()
