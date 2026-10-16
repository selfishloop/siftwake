"""Phase-separated Windows counters; never substitute process peaks for idle."""
import json
import statistics
import sys
import struct
import zlib
from pathlib import Path

prefix=sys.argv[1] if len(sys.argv)>1 else 'docs/benchmarks/content-memory-10k-20261011'
fdb_features=sys.argv[2] if len(sys.argv)>2 else 'default only'
def load(path): return json.loads(Path(path).read_text(encoding='utf-8-sig'))
def live_disk(mem,engine):
    files={f['name']:f['bytes'] for f in mem['files']}
    if engine=='sqlite': return sum(files.values())
    ready=max(name for name in files if name.endswith('.ready'))
    data=(Path(mem['database'])/ready).read_bytes()
    assert data[:8]==b'FDBMAN03' and len(data)==struct.unpack_from('<I',data,16)[0]+24
    assert zlib.crc32(data[:-4])==struct.unpack_from('<I',data,len(data)-4)[0]
    manifest=json.loads(data[20:-4])
    names=[ready,f"generation-{manifest['active']['id']:016x}.fdb"]
    assert files[names[-1]]==manifest['active']['end']
    for source in manifest['runs']:
        stem='run' if source['kind']=='Run' else 'generation'
        name=f"{stem}-{source['id']:016x}.fdb"
        assert files[name]==source['end']
        names.extend([name,name[:-4]+'.idx'])
    assert len(set(names))==len(names)
    return sum(files[name] for name in names)
def pair(engine,mode,count,run=0):
    base=f'{prefix}-{engine}-{mode}-{count}-{run}'
    mem=load(base+'.memory.json')
    rows=[json.loads(s) for s in Path(base+'.jsonl').read_text(encoding='utf-8-sig').splitlines() if s.strip()]
    assert mem['exit_code']==0 and not mem['low_disk_stop'] and mem['samples']>0
    assert mem['retained_disk_bytes']==sum(f['bytes'] for f in mem['files'])
    assert mem['features']==(fdb_features if engine=='fdb' else 'default only') and mem['includes_os_file_cache'] is False
    opening=next(r for r in rows if r['phase']=='opened')
    if mode=='idle':
        assert mem['idle']['search_calls']==0 and mem['idle']['after_open_wait_s']==5
        assert opening.get('documents',opening.get('stats',{}).get('documents'))==count
    else:
        result=next(r for r in rows if r['phase']=='result')
        assert result['from']==0 and result['to']==count==10000
        assert result['all_input_crc_verified'] and result['sampled_full_document_checks']==40
        assert result.get('documents',result.get('stats',{}).get('documents'))==count
        assert 'indexing' in mem['phases'] and 'maintenance' in mem['phases']
        if engine=='fdb': assert result['stats']['resident_documents']==0 and result['stats']['sequence']==count//500
    return {'memory':mem,'opening':opening,'result':result if mode=='ingest' else None}

out={'processes':8,'search_calls':0,'sampled_full_document_checks':80,
     'scope':'10k current engine comparison, no derived profile; same streamed long corpus; fresh DB; idle fresh process 5s after opened marker; thresholds are preliminary10k checks, not50k acceptance',
     'fdb_features':fdb_features,
     'indexing_repeats':1,'idle_repeats':3,'cache':'OS cache, no forced eviction; no searches/books/oracle resident',
     'unmeasured':['30-query rare/AND/accent p95 set','update visibility','concurrent writer/readers','cold cache','DLL','derived profile','compression','single-copy payload','temporary disk peak'],
     'engines':{},'all_processes_passed':True}
for engine in ['fdb','sqlite']:
    out['engines'][engine]={}
    previous=None
    for count in [10000]:
        ingest=pair(engine,'ingest',count)
        idle=[pair(engine,'idle',count,n) for n in range(3)]
        phases=ingest['memory']['phases']
        peak_private=max(phases[k]['sampled_peak_private_bytes'] for k in ['indexing','maintenance'])
        peak_ws=max(phases[k]['sampled_peak_working_set_bytes'] for k in ['indexing','maintenance'])
        r=ingest['result']
        input_bytes=r['text_bytes']+(previous['text_bytes'] if previous else 0)
        total_ingest=r['ingest_ms']+(previous['total_ingest_ms'] if previous else 0)
        total_maintenance=r['maintenance_ms']+(previous['total_maintenance_ms'] if previous else 0)
        combined_peak=max(peak_private,previous['indexing_peak_private_bytes'] if previous else 0)
        item={'idle_private_bytes':statistics.median(p['memory']['idle']['private_bytes'] for p in idle),
              'idle_working_set_bytes':statistics.median(p['memory']['idle']['working_set_bytes'] for p in idle),
              'idle_private_samples':[p['memory']['idle']['private_bytes'] for p in idle],
              'open_ms':statistics.median(p['opening']['open_ms'] for p in idle),
              'indexing_peak_private_bytes':combined_peak,'increment_indexing_peak_private_bytes':peak_private,
              'increment_indexing_peak_working_set_bytes':peak_ws,
              'indexing_peak_private_bytes_without_final_maintenance':phases['indexing']['sampled_peak_private_bytes'],
              'maintenance_peak_private_bytes':phases['maintenance']['sampled_peak_private_bytes'],
              'increment_ingest_ms':r['ingest_ms'],'increment_maintenance_ms':r['maintenance_ms'],
              'total_ingest_ms':total_ingest,'total_maintenance_ms':total_maintenance,
              'write_plus_maintenance_ms':total_ingest+total_maintenance,
              'text_bytes':input_bytes,'retained_disk_bytes':ingest['memory']['retained_disk_bytes'],
              'live_disk_bytes':live_disk(ingest['memory'],engine),
              'disk_over_text_ratio':ingest['memory']['retained_disk_bytes']/input_bytes,
              'gates':{'idle_private_15MiB':all(p['memory']['idle']['private_bytes']<=15*1048576 for p in idle),
                       'indexing_private_60MiB':combined_peak<=60*1048576,
                       'open_500ms':all(p['opening']['open_ms']<=500 for p in idle),
                       'disk_text40percent':live_disk(ingest['memory'],engine)<=0.4*input_bytes},
              'ingest_run':ingest,'idle_runs':idle}
        out['engines'][engine][str(count)]=item
        previous=item
for count in ['10000']:
    f,s=[out['engines'][e][count] for e in ['fdb','sqlite']]
    assert f['text_bytes']==s['text_bytes']
    f['gates']['disk_sqlite120percent']=f['live_disk_bytes']<=1.2*s['live_disk_bytes']
    f['gates']['retained_disk_sqlite120percent']=f['retained_disk_bytes']<=1.2*s['retained_disk_bytes']
    f['gates']['maintenance_sqlite2x']=f['increment_maintenance_ms']<=2*s['increment_maintenance_ms']
    f['gates']['rare_query_p95']='unmeasured'
    f['gates']['update_visible20ms']='unmeasured'
Path(prefix+'-summary.json').write_text(json.dumps(out,indent=2),encoding='utf-8')
print(json.dumps({e:{c:{k:v for k,v in a.items() if k not in ['ingest_run','idle_runs']} for c,a in b.items()} for e,b in out['engines'].items()},indent=2))
