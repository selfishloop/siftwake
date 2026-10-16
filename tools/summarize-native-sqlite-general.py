"""Verify fresh paired native binary FDB/SQLite runs before reporting medians."""
import json
import statistics
import sys
from pathlib import Path

prefix = sys.argv[1] if len(sys.argv) > 1 else 'docs/benchmarks/native-sqlite-general-20261011'
def compare(a, b):
    ratios = [y/x for x,y in zip(a,b)]
    return {'fdb_median':statistics.median(a),'sqlite_median':statistics.median(b),
            'paired_sqlite_over_fdb':ratios,'ratio_range':[min(ratios),max(ratios)],
            'fdb_faster_every_pair':all(x>1 for x in ratios),
            'sqlite_faster_every_pair':all(x<1 for x in ratios)}
data = {}
expected = {'common:0':100000,'common:50':100000,'common:99950':100000,
            'rare:0':264,'rare:50':264,'short:0':100000,'short:50':100000,
            'and:0':16537,'and:50':16537}
for phase in ['short','long']:
    for engine in ['fdb','sqlite']:
        runs=[]
        for run in range(3):
            base=f'{prefix}-{phase}-{engine}-{run}'
            rows=[json.loads(s) for s in Path(base+'.jsonl').read_text(encoding='utf-8-sig').splitlines() if s.strip()]
            mem=json.loads(Path(base+'.memory.json').read_text(encoding='utf-8-sig'))
            assert mem['exit_code']==0 and mem['samples']>0 and mem['engine']==engine and mem['phase']==phase
            assert mem['features']=='default only' and mem['wal_sync']=='on'
            assert mem['retained_engine_disk_bytes']==sum(f['bytes'] for f in mem['files'])
            if phase=='short':
                result=next(r for r in rows if r['phase']=='result')
                assert result['count']==100000 and result['text_bytes']==256
                assert result['verified_updates']==result['verified_deletes']==50
                assert result['query_verification'].startswith('full StoredDocument')
                if engine=='fdb':
                    assert result['stats']['documents']==99950 and result['stats']['sequence']==201
                    assert result['stats']['resident_documents']==0
                else:
                    assert result['documents']==99950 and result['integrity_ms'] is None
                    cfg=next(r for r in rows if r['phase']=='ingest')
                    assert cfg['cache_kib']==2048 and cfg['synchronous']=='FULL' and cfg['journal_mode']=='WAL'
            else:
                opening=next(r for r in rows if r['phase']=='open')
                get=next(r for r in rows if r['phase']=='get')
                assert get['requests']==100 and get['full_document_oracle_verified'] and len(get['samples_ms'])==3
                qs={f"{r['query']}:{r['offset']}":r for r in rows if r['phase']=='query'}
                assert set(qs)==set(expected)
                for key,q in qs.items():
                    assert q['total']==expected[key] and q['full_page_oracle_verified'] and not q['profiled']
                    assert len(q['samples_ms'])==(10 if key.startswith('rare:') else 1)
                    if engine=='sqlite': assert any('VIRTUAL TABLE' in s for s in q['plan'])
                if engine=='fdb':
                    assert opening['overlay']=='native' and opening['overlay_bytes']==opening['attach_ms']==0
                    assert opening['stats']['documents']==100000 and opening['stats']['sequence']==200
                    assert opening['stats']['segments']==1 and opening['stats']['resident_documents']==0
                else: assert opening['full_integrity_on_open'] is False
                result={'open_ms':opening['open_ms'],'get_100':get,'queries':qs}
            runs.append({'result':result,'memory':mem})
        data[phase,engine]=runs
out={'processes':12,'total_search_calls':978,'long_get_calls':2400,
     'all_processes_and_oracles_passed':True,'open_contract_equal':False,
     'cache':'OS cache, no forced eviction; per-query warmup; alternating paired serial processes',
     'scope':'default native OFF1 FDB vs isolated SQLite WAL/FULL/2MiB cache/mmap0/tempFILE; exact total + sorted full pages',
     'unmeasured':['long ingestion','cold cache','concurrency','source-offset layout','power loss','temporary disk peak'],
     'short':{},'long':{}}
for phase in ['short','long']:
    a,b=data[phase,'fdb'],data[phase,'sqlite']
    for key in ['peak_working_set_bytes','sampled_peak_private_bytes','retained_engine_disk_bytes','elapsed_s','cpu_ms']:
        out[phase][key]=compare([r['memory'][key] for r in a],[r['memory'][key] for r in b])
    metrics=['ingest_ms','maintenance_ms','reopen_ms','changed_reopen_ms'] if phase=='short' else ['open_ms']
    for key in metrics:
        out[phase][key]=compare([r['result'][key] for r in a],[r['result'][key] for r in b])
    keys=['narrow','broad','range','broad_text','last_page','second_page','get_100'] if phase=='short' else ['get_100']
    for key in keys:
        out[phase][key+'_p50_ms']=compare([r['result'][key]['p50_ms'] for r in a],[r['result'][key]['p50_ms'] for r in b])
    if phase=='short':
        out[phase]['write_plus_maintenance_ms']=compare([r['result']['ingest_ms']+r['result']['maintenance_ms'] for r in a],[r['result']['ingest_ms']+r['result']['maintenance_ms'] for r in b])
    else:
        for x,y in zip(a,b):
            for key in expected:
                assert all(x['result']['queries'][key][k]==y['result']['queries'][key][k] for k in ['text','offset','total'])
        out[phase]['queries']={key:compare([r['result']['queries'][key]['p50_ms'] for r in a],[r['result']['queries'][key]['p50_ms'] for r in b]) for key in expected}
out['runs']={f'{phase}-{engine}':runs for (phase,engine),runs in data.items()}
Path(prefix+'-summary.json').write_text(json.dumps(out,indent=2),encoding='utf-8')
print(json.dumps({k:v for k,v in out.items() if k!='runs'},indent=2))
