"""Validate all paired processes before reporting native binary vs JSON control."""
from pathlib import Path
import json
import statistics
import sys

prefix = Path(sys.argv[1] if len(sys.argv)>1 else 'docs/benchmarks/native-general-20261011')
def read(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))
def rows(path):
    return [json.loads(line) for line in path.read_text(encoding='utf-8-sig').splitlines() if line.strip()]
def comparison(j, b):
    assert len(j)==len(b)==3
    medj, medb = statistics.median(j), statistics.median(b)
    gains = [100*(x-y)/x for x,y in zip(j,b)]
    return {'json_median':medj,'binary_median':medb,'paired_gain_percent':gains,
            'gain_range_percent':[min(gains),max(gains)],'all_pairs_faster':all(y<x for x,y in zip(j,b))}

data = {}
for phase in ['short','long']:
    for variant in ['json','binary']:
        runs=[]
        for run in range(3):
            base=Path(f'{prefix}-{phase}-{variant}-{run}')
            output=rows(Path(str(base)+'.jsonl'))
            memory=read(Path(str(base)+'.memory.json'))
            assert memory['exit_code']==0 and memory['samples']>0
            assert memory['features']=='default only' and memory['wal_sync']=='on'
            assert memory['includes_os_file_cache'] is False
            assert memory['retained_engine_disk_bytes']==sum(f['bytes'] for f in memory['files'])
            if phase=='short':
                result=next(r for r in output if r['phase']=='result')
                assert result['count']==100000 and result['text_bytes']==256
                assert result['verified_updates']==result['verified_deletes']==50
                assert result['stats']['documents']==99950
                assert result['stats']['resident_documents']==0
                assert result['stats']['sequence']==201
                assert result['query_verification'].startswith('full StoredDocument')
            else:
                opened=next(r for r in output if r['phase']=='open')
                assert opened['overlay']=='native' and opened['overlay_bytes']==opened['attach_ms']==0
                assert opened['stats']['documents']==100000 and opened['stats']['sequence']==200
                assert opened['stats']['resident_documents']==0 and opened['stats']['segments']==1
                queries=[r for r in output if r['phase']=='query']
                assert len(queries)==9
                assert all(q['full_page_oracle_verified'] and q['api']=='Database::search' and not q['profiled'] for q in queries)
                assert next(r for r in output if r['phase']=='get')['full_document_oracle_verified']
                result={'open_ms':opened['open_ms'], 'get_100':next(r for r in output if r['phase']=='get'),
                        'queries':{f"{q['query']}:{q['offset']}":q for q in queries}}
            runs.append({'result':result,'memory':memory})
        data[phase,variant]=runs

out={'corpus_short':'100000 unique keys/titles, repeated 256 UTF-8-byte Turkish body, batch500, delta10000 keys/16MiB, sync on',
     'corpus_long':'100000 unique overlapping real book windows, 2-50KiB, 2655160287 body bytes, same fields/links/versions',
     'cache':'OS file cache; no forced eviction; query warmups; independent processes in JSON/binary, binary/JSON, JSON/binary order',
     'reference':'isolated current source snapshot with default JSON encode/accounting/SEG03 reinstated; same fold/query/maintenance code; not historical release',
     'placement':'both variants duplicate payload in source and index; source-offset layout unmeasured',
     'short':{},'long':{},'processes':12,'short_search_calls':792,'long_search_calls':186,'total_search_calls':978,'long_get_calls':2400,
     'unmeasured':['long-corpus ingest','cold cache','source-offset single-copy layout','concurrent throughput','update/delete latency','power loss'],
     'all_processes_and_oracles_passed':True}
for phase in ['short','long']:
    j,b=data[phase,'json'],data[phase,'binary']
    for key in ['peak_working_set_bytes','sampled_peak_private_bytes','retained_engine_disk_bytes','elapsed_s','cpu_ms']:
        out[phase][key]=comparison([r['memory'][key] for r in j],[r['memory'][key] for r in b])
    if phase=='short':
        for key in ['ingest_ms','maintenance_ms','reopen_ms','changed_reopen_ms']:
            out[phase][key]=comparison([r['result'][key] for r in j],[r['result'][key] for r in b])
        out[phase]['write_plus_maintenance_ms']=comparison([r['result']['ingest_ms']+r['result']['maintenance_ms'] for r in j],
                                                          [r['result']['ingest_ms']+r['result']['maintenance_ms'] for r in b])
        for key in ['narrow','broad','range','broad_text','last_page','second_page','get_100']:
            out[phase][key+'_p50_ms']=comparison([r['result'][key]['p50_ms'] for r in j],[r['result'][key]['p50_ms'] for r in b])
    else:
        out[phase]['open_ms']=comparison([r['result']['open_ms'] for r in j],[r['result']['open_ms'] for r in b])
        out[phase]['get_100_p50_ms']=comparison([r['result']['get_100']['p50_ms'] for r in j],[r['result']['get_100']['p50_ms'] for r in b])
        keys=set(j[0]['result']['queries'])
        assert all(set(r['result']['queries'])==keys for r in j+b)
        out[phase]['queries']={}
        for key in sorted(keys):
            for x,y in zip(j,b):
                jq,bq=x['result']['queries'][key],y['result']['queries'][key]
                assert (jq['text'],jq['total'],jq['offset'])==(bq['text'],bq['total'],bq['offset'])
            out[phase]['queries'][key]=comparison([r['result']['queries'][key]['p50_ms'] for r in j],[r['result']['queries'][key]['p50_ms'] for r in b])
Path(str(prefix)+'-summary.json').write_text(json.dumps(out,indent=2),encoding='utf-8')
print(json.dumps(out,indent=2))
