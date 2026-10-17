import json, math, statistics, sys
from pathlib import Path
p=Path(sys.argv[1])
def read(suffix): return json.loads(Path(f'{p}-{suffix}.json').read_text(encoding='utf-8-sig'))
def phases(suffix): return {r['phase']: r for r in map(json.loads,Path(f'{p}-{suffix}.jsonl').read_text(encoding='utf-8-sig').splitlines())}
d=read('design'); n=d['pairs']
rows=[]; matrices={e:[] for e in ('control','fdb')}; sources=[]
for i in range(n):
 for e in ('control','fdb'):
  stem=f'{e}-{i}'; w=phases(f'{stem}-fdb-ingest-10000-0')['result']; m=read(f'{stem}-fdb-ingest-10000-0.memory')
  v=phases(f'{stem}-verify')['verified']; assert v['full_document_checks']==10000 and v['input_crc_verified'] and v['first50_full_pages_verified']
  assert w['text_bytes']==264842858 and m['exit_code']==0 and not m['low_disk_stop']
  o=phases(f'{stem}-open'); assert o['opened']['pending_source_checks']==0
  q=read(f'{stem}-query-memory.memory')['os_lifetime_peaks']; live=read(f'{stem}-live-indexes')
  if isinstance(live,dict):live=[live]
  assert all(r['magic']==('FDBSEG05' if e=='fdb' else 'FDBSEG04') for r in live)
  if e=='fdb':assert all(r['references_flag']==1 for r in live)
  sources.append(read(f'{stem}-sources'))
  rows.append(dict(variant=e,pair=i,ingest_s=w['ingest_ms']/1000,maintenance_s=w['maintenance_ms']/1000,combined_s=(w['ingest_ms']+w['maintenance_ms'])/1000,cpu_s=m['cpu_ms']/1000,write_private_bytes=m['os_lifetime_peaks']['peak_private_commit_bytes'],write_ws_bytes=m['os_lifetime_peaks']['peak_working_set_bytes'],query_private_bytes=q['peak_private_commit_bytes'],query_ws_bytes=q['peak_working_set_bytes'],retained_disk_bytes=m['retained_disk_bytes'],live_index_bytes=sum(r['bytes']+r['metadata_bytes'] for r in live),open_ms=o['opened']['open_ms'],first_use_ms=o['opened']['open_ms']+o['deferred_first_use']['first_get_ms']+o['deferred_first_use']['first_search_ms']))
  matrices[e].append(phases(f'{stem}-matrix')['matrix_reads'])
assert all(x==sources[0] for x in sources)
assert read('protected-before')==read('protected-after')
fields=[k for k in rows[0] if k not in ('variant','pair')]
med={e:{k:statistics.median(r[k] for r in rows if r['variant']==e) for k in fields} for e in matrices}
comparisons={}
for k in fields:
 a=[r[k] for r in rows if r['variant']=='control']; b=[r[k] for r in rows if r['variant']=='fdb']
 comparisons[k]=dict(change_percent=100*(med['fdb'][k]/med['control'][k]-1),lower_pairs=sum(y<x for x,y in zip(a,b)),paired_change_percent=[100*(y/x-1) for x,y in zip(a,b)])
def pooled(rs):
 for r in rs:assert len(r['samples'])==r['n'] and sorted(r['samples'])[math.ceil(r['n']*.95)-1]==r['p95_ms']
 s=sorted(x for r in rs for x in r['samples']);return dict(n=len(s),p95_ms=s[math.ceil(len(s)*.95)-1],p50_ms=s[math.ceil(len(s)*.5)-1],run_p95=[r['p95_ms'] for r in rs])
qs=[]
assert all(m['full_document_checks'] for ms in matrices.values() for m in ms)
for i,q in enumerate(matrices['control'][0]['queries']):
 identity={k:q[k] for k in ('text','kind','offset','limit','total','late_matches')}; groups={}
 for e,ms in matrices.items():
  rs=[m['queries'][i] for m in ms]; assert all(all(r[k]==v for k,v in identity.items()) for r in rs)
  groups[e]=pooled([r['timing'] for r in rs])
 a,b=groups['control'],groups['fdb'];qs.append(dict(**identity,**groups,change_percent=100*(b['p95_ms']/a['p95_ms']-1),lower_pairs=sum(y<x for x,y in zip(a['run_p95'],b['run_p95']))))
gets=[]
for i in range(len(matrices['control'][0]['gets'])):
 groups={e:pooled([m['gets'][i]['timing'] for m in ms]) for e,ms in matrices.items()}
 gets.append(dict(bin=i,**groups,change_percent=100*(groups['fdb']['p95_ms']/groups['control']['p95_ms']-1)))
result=dict(design=d,rows=rows,medians=med,comparisons=comparisons,queries=qs,gets=gets,lower_p95=sum(q['change_percent']<0 for q in qs),higher_p95=sum(q['change_percent']>0 for q in qs),source_ready_hashes_equal=True,source_ready_files=len(sources[0]),protected_files_unchanged=True,limits='Warm OS cache, no eviction/concurrent/power-cut test. Normal writer plus reader change together; not an isolated read algorithm comparison. Private commit and WS are process lifetime counters, not total system RAM. No fresh SQLite performance comparison.')
Path(f'{p}-summary.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps({k:result[k] for k in ('medians','comparisons','lower_p95','higher_p95')},indent=2))
