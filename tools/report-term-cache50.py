"""Retain every arm/sample and unchanged gate. No selective reruns."""
import csv,json,math
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='term-cache50-20261102'
def read(p):return json.loads(p.read_text(encoding='utf-8-sig'))
def stat(xs):
    x=sorted(xs);n=len(x)
    return dict(n=n,mean=sum(x)/n,p50=x[math.ceil(.5*n)-1],p95=x[math.ceil(.95*n)-1],p99=x[math.ceil(.99*n)-1],p999=x[math.ceil(.999*n)-1],max=x[-1])
def rows(p):return [json.loads(x) for x in p.read_text(encoding='utf-8').splitlines() if x]
def main():
    d=read(L/f'{P}-design.json');normal=[];arms=[];samples=[];pools=[];pool_samples=[];ram=[];cache=[]
    catalog=read(L/'trwiki-cap200k-20261029-v3-catalog.json')
    for run in d['completed']:
        log=rows(L/run['log']);base=dict(pair=run['pair'],arm=run['arm'])
        diag=next((x['counters'] for x in log if x['phase']=='term_cache_diagnostics'),None)
        if diag:cache.append({**base,'phase':run['phase'],**diag})
        if run['phase']=='ram':
            x=next(x for x in log if x['phase']==run['mode'])
            ram.append(dict(**base,mode=run['mode'],**x['memory']));continue
        if run['phase'] in ['control','normal']:
            load=next(x for x in log if x['phase']=='snapshot_load');assert load['oracle'] and load['failure'] is None
            writes=stat([x['commit_ms'] for x in load['writes']])
            arm=dict(**base,phase=run['phase'],ops_s=load['mutations']*1000/load['writer_elapsed_ms'],write=writes,
                     mutations=load['mutations'],writer_elapsed_ms=load['writer_elapsed_ms'],
                     cpu_ms_all_process_threads=run['cpu_ms_all_process_threads'],memory=load.get('memory'),
                     over100=sum(x['commit_ms']>100 for x in load['writes']),thread_peak=run['thread_peak'])
            if run['phase']=='normal':
                phases={}
                for name in ['snapshot_idle_single','snapshot_idle_four','snapshot_load']:
                    phase=next(x for x in log if x['phase']==name);assert phase['oracle']
                    bycase=[[] for _ in range(41)]
                    for reader in phase['results']:
                        assert reader['oracle']
                        for q in reader['records']:
                            assert q['oracle']
                            if name!='snapshot_load' or q['writer_active']:bycase[q['case_id']].append(q['engine_ms'])
                    assert min(map(len,bycase))>=60
                    phases[name]=[stat(v) for v in bycase]
                    for i,v in enumerate(bycase):samples.append(dict(**base,phase=name,case_id=i,values=v,**stat(v)))
                for i in range(41):
                    idle=phases['snapshot_idle_four'][i];loaded=phases['snapshot_load'][i]
                    limit=30 if idle['p95']<=30 else 2*idle['p95']
                    normal.append(dict(**base,case_id=i,text=catalog['cases'][i]['text'],one=phases['snapshot_idle_single'][i],idle=idle,loaded=loaded,limit=limit,passed=loaded['p95']<=limit))
                own=[x for x in normal if all(x[k]==base[k] for k in base)]
                arm.update(passed=sum(x['passed'] for x in own),failed=[x['case_id'] for x in own if not x['passed']])
            arms.append(arm)
        elif run['phase']=='pool':
            writes=next(x for x in log if x['phase']=='writes');assert writes['oracle']
            rr=[];profiled=[]
            for ordinal,x in enumerate(writes['records']):
                assert x['oracle'];p=x['timeline_ns'];assert list(p.values()) # serde map order is not chronological
                values=[p[k] for k in ['commit_return','history_done','acquire_done','probe_prepared','diagnostic_begin_done','query_done','release_done']]
                assert values==sorted(values)
                r=dict(**base,ordinal=ordinal,sequence=x['sequence'],kind=x['kind'],body_band=x['body_band'],path_bytes=x['path_bytes'],commit=x['commit_ms'],visibility=x['visibility_ms'],gap=(values[-1]-values[0])/1e6,acquire=(values[2]-values[1])/1e6,first_query=(values[5]-values[4])/1e6,profiled=bool(x['diagnostic']))
                if r['profiled']:
                    r['profile']=x['diagnostic'];profiled.append(r)
                pool_samples.append(r)
                if not x['kind'].startswith('edge') and not r['profiled']:rr.append(r)
            assert len(rr)>=4000
            summary={k:stat([r[k] for r in rr]) for k in ['commit','visibility','gap','acquire','first_query']}
            summary.update(**base,main_samples=len(rr),profiled_samples=len(profiled),cpu_ms_all_process_threads=run['cpu_ms_all_process_threads'],memory=writes['memory'])
            summary['by_kind']={}
            for kind in ['update','delete','insert','mtime_delete_insert','edge1000_mtime']:
                cohort=[r for r in pool_samples if r['pair']==run['pair'] and r['arm']==run['arm'] and r['kind']==kind and not r['profiled']]
                # A profiled sole edge stays informational in its profile row.
                if not cohort:continue
                st={k:stat([r[k] for r in cohort]) for k in ['commit','visibility','gap','first_query']}
                st['passed']=all(v['max']<=100 and (kind.startswith('edge') or v['p95']<=20 and v['p99']<=50) for k,v in st.items() if k in ['commit','visibility'])
                summary['by_kind'][kind]=st
            summary['short_gate']=all(x['passed'] for x in summary['by_kind'].values())
            pools.append(summary)
    for arm in arms:
        if arm['phase']=='normal':
            control=next(x for x in arms if x['phase']=='control' and x['arm']==arm['arm'])
            arm['retention']=arm['ops_s']/control['ops_s']
    out=dict(stage=d['stage'],default='K unchanged',normal=normal,arms=arms,pools=pools,ram=ram,cache=cache,error=d.get('error'))
    (L/f'{P}-summary.json').write_text(json.dumps(out,ensure_ascii=False,indent=2),encoding='utf-8')
    (L/f'{P}-normal-samples.json').write_text(json.dumps(samples,ensure_ascii=False),encoding='utf-8')
    (L/f'{P}-pool-samples.json').write_text(json.dumps(pool_samples,ensure_ascii=False),encoding='utf-8')
    lines=['# Terim cache son deney — 2 Kasım 2026','',f"Durum: **{d['stage']}**. Varsayılan **K** korunur. Eşikler aynı; A/D sonuç kararı kullanıcıya ait.",'',
           'K=960KiB/kilit içinde; A=aynı toplam byte sınırında tek decoder/kilit dışında; D=yalnız sayfa4MiB, descriptor64KiB. B/C yok.',
           '','Sıra K/A/S/D, A/D/K/S, D/S/A/K, S/K/D/A. Aynı ilk50k/881756DA…; sıcak/kontrolsüz OS cache, aynı sabit slotlar ilerler, reset/ingest/DB kopyası yok.',
           '','## Normal41 N60 — ham kapı kararları','', '|Blok|Kol|Geçti/41|Kalan case|Yazma ops/s|Korunum %|Commit p95/p99/p99,9/max ms|CPU ms (bütün proses threadleri)|','|---|---|---:|---|---:|---:|---|---:|']
    for x in arms:
        if x['phase']=='normal':lines.append(f"|{x['pair']}|{x['arm']}|{x['passed']}|{x['failed']}|{x['ops_s']:.2f}|{100*x['retention']:.2f}|"+'/'.join(f"{x['write'][k]:.3f}" for k in ['p95','p99','p999','max'])+f"|{x['cpu_ms_all_process_threads']:.2f}|")
    lines+=['','Her sorgunun kuralı: idle p95<=30 ise loaded<=30ms, aksi<=2*idle. N>=60, nearest rank; bütün örnekler saklandı. Oracle bütün idle/loaded sorgularda, zaman sınırı dışında.',
            '','## Frunner görünürlük — profilsiz işlemler','', '|Blok|Kol|N|Gap p50/p95/max ms|İlk query p95 ms|Commit p95 ms|Visibility p95/p99/max ms|Tekli kapı|','|---|---|---:|---|---:|---:|---|---|']
    for x in pools:lines.append(f"|{x['pair']}|{x['arm']}|{x['main_samples']}|"+'/'.join(f"{x['gap'][k]:.3f}" for k in ['p50','p95','max'])+f"|{x['first_query']['p95']:.3f}|{x['commit']['p95']:.3f}|"+'/'.join(f"{x['visibility'][k]:.3f}" for k in ['p95','p99','max'])+f"|{'REFERANS' if x['arm']=='S' else ('GEÇTİ' if x['short_gate'] else 'KALDI')}|")
    lines+=['','Tablodaki p95ler pooled bilgi; tekli kapı her işlem tipine ayrı uygulanır (summary by_kind), uçta yalnız max100ms. Profil stride64, işaretli işlemler ana wall dağılımından çıkarılıp ayrı saklandı. Sequence tabanlı stride örnekleri işlem tipine dengeli olmak zorunda değildir; profilli alt-küme saf eşleşmiş tip/CPU kazancı sayılmaz. Gap aynı işlem visibility−commit farkıdır. Parent+child CPU süreçteki bütün threadlerin toplamıdır; açılış/oracle/yazma dahildir, saf sorgu CPU iddiası değildir.',
            '','## Motor-only RAM','', '|Blok|Kol|Durum|Idle private MiB|Proses peak private MiB|15MiB karşılığı|','|---|---|---|---:|---:|---|']
    for x in ram:lines.append(f"|{x['pair']}|{x['arm']}|{x['mode']}|{x['private_bytes']/2**20:.3f}|{x['peak_private_commit_bytes']/2**20:.3f}|{'GEÇTİ' if x['private_bytes']<=15*2**20 else 'KALDI'}|")
    lines+=['','RAM ayrı engine+runtime prosesidir; model/oracle/history/katalog içermez. Fresh idle ve41-query-sonrası warm idle ayrıdır. Yeni ingest ölçülmedi.',
            '','## 4R+W ana proses RAM — model/oracle/history dahil','',
            '|Blok|Kol|Örneklenmiş phase peak private MiB|OS lifetime peak private commit MiB|','|---|---|---:|---:|']
    for x in arms:
        if x['phase']=='normal' and x['memory']:
            v=x['memory'];lines.append(f"|{x['pair']}|{x['arm']}|{v['sampled_phase_peak_private_bytes']/2**20:.3f}|{v['os_lifetime_peak_private_commit_bytes']/2**20:.3f}|")
    lines+=['','Bu tablo motor-only değildir; modelleri çıkararak tahmin yapılmadı. D yalnız capacity sabitini büyütür; WORKING_POOL\'un2MiB baseline\'ı büyütülmedi. D\'nin4MiB term cache\'i bu baseline içinde sayılmış gibi toplam16MiB hard-cap iddiası kurulmaz. Gerçek proses RAM yukarıda ölçülür.',
            '','## Bounded cache sayaçları','', '|Faz/blok/kol|Page hit/miss|Capacity/oversized/drop page|Lookup eviction|In-flight/combined peak bytes (A)|','|---|---|---|---:|---|']
    for x in cache:lines.append(f"|{x['phase']}/{x['pair']}/{x['arm']}|{x['page_hits']}/{x['page_misses']}|{x['capacity_pages']}/{x['oversized_pages']}/{x['drop_pages']}|{x['lookup_evictions']}|{x['inflight_peak_bytes']}/{x['combined_peak_bytes']}|")
    lines+=['','K/D in-flight0, ölçülmüş sıfır scratch anlamına gelmez; A rezervasyon sayacı yalnız A yolundadır. A page-hit sayacı recheck hitlerini de kapsar; başlangıç miss+sonra recheck-hit aynı istekte birlikte bulunabilir. Gerçek decode miktarı profilli decode çağrısıyla veya recheck farkıyla ayrıca değerlendirilir. Legacy fallback oversized\'dan ayrı sayaçtır. CRC/read/decode ve slot beklemesi profilli örneklerde nested wall; üst toplamla tekrar toplanmaz.',
            '','## Karar ve sınırlar','']
    if d['stage']=='completed':
        ng={a:all(x['passed'] for x in normal if x['arm']==a) for a in ['K','A','D','S']}
        paired={a:[next(x['gap']['p95'] for x in pools if x['pair']==p and x['arm']==a)-next(x['gap']['p95'] for x in pools if x['pair']==p and x['arm']=='K') for p in range(4)] for a in ['A','D']}
        out.update(normal_gates=ng,paired_gap_p95_difference_ms=paired)
        lines += [f"Normal41 tüm çiftler: {ng}. Gap p95 eşleşmiş aday−K ms: {paired}.",
                  'A−K kilit kapsamı+zorunlu rezervasyon düzenlemesinin, D−K kapasitenin toplam etkisidir. Saf tek kilit CPU payı diye yorumlanmaz. Yeni owner/tahliye ayrımı bounded sayaçlarla gösterilir; OS cache/scheduler/antivirüs atfı yapılmaz.',
                  '**Bu son bloktan sonra duruldu.** Bir kapı kalırsa veya net gap/ilk-query kazanımı yoksa aday varsayılan yapılmaz ve Blok2 kapanır. Yeni aday/ayar/tekrar yok.']
        (L/f'{P}-summary.json').write_text(json.dumps(out,ensure_ascii=False,indent=2),encoding='utf-8')
    else:lines.append('Henüz tamamlanmamış ölçüm: GEÇTİ/kazanç veya kök neden kapanışı ilan edilmez. '+str(d.get('error','')))
    lines+=['','[Tasarım/SHA/süreç](benchmarks/'+P+'-design.json) · [Bütün özetler](benchmarks/'+P+'-summary.json) · [Normal ham örnekler](benchmarks/'+P+'-normal-samples.json) · [Gap ve ayrı profilli örnekler](benchmarks/'+P+'-pool-samples.json).',
            '','100lü,C500drain,2,6sn,WAL handle/File::open,zx düzeltmesi,reclaim varsayılanı,C ABI yapılmadı. Diğer açık başlıklar OPEN-WORKte korunur.']
    close=R/'docs/benchmarks'/f'{P}-closing.md'
    if close.exists():lines[2:2]=[close.read_text(encoding='utf-8'),'']
    (R/'docs/TERM-CACHE-50K-20261102.md').write_text('\n'.join(lines)+'\n',encoding='utf-8')
if __name__=='__main__':main()
