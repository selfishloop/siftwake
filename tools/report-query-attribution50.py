import json,math,statistics,csv
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='query-attribution50-20261030'
def pct(x):return sorted(x)[math.ceil(len(x)*.95)-1]
def main():
    d=json.loads((L/f'{P}-design.json').read_text(encoding='utf-8'));assert d['stage']=='completed';out=[]
    for run in d['completed']:
        rows=list(map(json.loads,(L/run['log']).read_text(encoding='utf-8').splitlines()))
        for phase in rows:
            if phase['phase'] not in ['snapshot_idle_probe','snapshot_load']:continue
            for reader in phase['results']:
                for q in reader['records']:
                    if q['case_id'] not in [16,25] or (phase['phase']=='snapshot_load' and not q['writer_active']):continue
                    assert q['oracle'] and q['internal_wait_measured'];profile=q['query_profile'];cpu=[t['cpu_100ns'] for t in profile['threads']];out.append(dict(label=run['label'],**q,cpu_ms=sum(t for t in cpu if t is not None)/10000,query_thread_count=len(cpu)))
    (L/f'{P}-requests.json').write_text(json.dumps(out,ensure_ascii=False,indent=2),encoding='utf-8')
    stages=['first.delta_filter','first.posting_setup','first.posting_intersection','first.candidate_verify','query.posting_page_read','query.posting_cache_read','query.payload_read','query.payload_crc','query.payload_validation','payload.decode_crc_identity','first.permit_acquire','first.permit_mutex_wait','first.permit_condvar_wait','query.term_cache_mutex_wait','query.metadata_cache_mutex_wait','lock.cache_wait']
    counts=['query.term_lookup_hits','query.term_lookup_misses','query.term_page_hits','query.term_page_misses','query.posting_page_hits','query.posting_page_read','first.hybrid_handoffs','first.permits_granted']
    measured={k for q in out for k,v in q['query_profile']['counters'].items() if v['elapsed_ns']}
    stages+=sorted(measured-set(stages))
    counts+=sorted({k for q in out for k in q['query_profile']['counters'] if k.endswith(('_hits','_misses','_evictions'))}-set(counts))
    with (L/f'{P}-requests.tsv').open('w',encoding='utf-8',newline='') as f:
        header=['label','case_id','sequence','thread','round','writer_active','search_ms','engine_ms','cpu_ms']+[s+'.ms' for s in stages]+[s+'.calls' for s in counts]
        writer=csv.DictWriter(f,fieldnames=header,delimiter='\t');writer.writeheader()
        for q in out:
            counter=q['query_profile']['counters'];row={k:q[k] for k in header[:9]};row.update({s+'.ms':counter.get(s,{}).get('elapsed_ns',0)/1e6 for s in stages});row.update({s+'.calls':counter.get(s,{}).get('calls',0) for s in counts});writer.writerow(row)
    for q in out:
        q['group']=('loaded-' if q['label'].startswith('loaded') else 'idle-')+'-'.join(q['label'].split('-')[1:])
    groups=sorted(set((q['group'],q['case_id']) for q in out));summary=[]
    lines=['# Timmy / üreticisi aşama profili —30 Ekim2026','','Yalnız ölçüm instrumentation: term/metadata cache mutex acquisition span, permit mutex/condvar span, mevcut posting/payload/CRC/verification/cache hit-miss ve request-local CPU. Child worker threadleri aynı request handle ile katılır; test parent/child iki gerçek mutex ediniminin o requestte toplanmasını denetler. Normal algoritma/cache politikası/biçim değiştirilmedi. Frozen kabul ikilisi yeniden derlenmedi; diagnostics ayrı ikili, kabul p95 yerine kullanılamaz.','','Idle E matrisi aynı live source/WAL/manifest SHA üzerinde1R41→4R41→4Rfew→4Rfew→4R41→1R41; few cases16,25,31,33.1R41 20round;4R41 5round;4Rfew50round. Bu, hedef sorgu örnek sayılarını açık verir. Sonra4R+16384 tekli writer41 ve few. Yüklü iki varyant aynı DBde ilerleyen sequence/normal bakım kullanır; bit-aynı fiziksel yerleşim diye sunulmaz. Hepsinde oracle.','','Mutex sayacı actual lock acquisition wall içerir (uncontended overhead dahil); permit condvar ayrı ölçülür. Nested/parallel spanlar üst üste biner: sütunlar toplanıp yüzde veya CPU süresi yapılmaz. CPU GetThreadTimes toplam parent+child,coarse15,625ms nicemlenmesi olabilir. Wall−CPU scheduler/antivirüs iddiası değildir. Cache miss sayacı posting_page_read.calls ile fill sayısıdır; query-local posting pages mutex paylaşmaz, global term/metadata cache mutexleri ayrı. Sayaç0 bir aşamanın çalışmadığını gösterebilir; missing=0 durumunu raw requestten ayırt edin.','','## Query wall/CPU','','|Grup/case|N|Wall p50 /p95 ms|CPU p50 /p95 ms|Mean mutex wait ms (term /metadata)|Mean permit condvar ms|','|---|---:|---|---|---|---:|']
    for group,i in groups:
        q=[x for x in out if (x['group'],x['case_id'])==(group,i)]
        def vals(name):return [x['query_profile']['counters'].get(name,{}).get('elapsed_ns',0)/1e6 for x in q]
        row=dict(group=group,case_id=i,n=len(q),wall_p50=statistics.median(x['search_ms'] for x in q),wall_p95=pct([x['search_ms'] for x in q]),cpu_p50=statistics.median(x['cpu_ms'] for x in q),cpu_p95=pct([x['cpu_ms'] for x in q]),stages={s:dict(mean=statistics.mean(vals(s)),p95=pct(vals(s))) for s in stages},counts={s:sum(x['query_profile']['counters'].get(s,{}).get('calls',0) for x in q)/len(q) for s in counts});summary.append(row)
        lines.append(f"|{group}/{i}|{len(q)}|{row['wall_p50']:.3f} /{row['wall_p95']:.3f}|{row['cpu_p50']:.3f} /{row['cpu_p95']:.3f}|{row['stages']['query.term_cache_mutex_wait']['mean']:.3f} /{row['stages']['query.metadata_cache_mutex_wait']['mean']:.3f}|{row['stages']['first.permit_condvar_wait']['mean']:.3f}|")
    lines+=['','## Bütün aşamalar: request başına mean /p95 wall ms','','|Grup/case|Aşama|Mean ms|p95 ms|','|---|---|---:|---:|']
    for row in summary:
        for name,x in row['stages'].items():lines.append(f"|{row['group']}/{row['case_id']}|{name}|{x['mean']:.3f}|{x['p95']:.3f}|")
    lines+=['','## Hit/miss ve işçi sayıları (request başına ortalama)','','|Grup/case|Sayaç|Ortalama|','|---|---|---:|']
    for row in summary:
        for name,v in row['counts'].items():lines.append(f"|{row['group']}/{row['case_id']}|{name}|{v:.2f}|")
    lines+=['','Her sorgu sequence,thread,round,writer-active,wall/CPU ve bütün sayaçlarıyla [ham request tablosunda](benchmarks/'+P+'-requests.json). [Kaynak SHA eşitliği/program ve loglar](benchmarks/'+P+'-design.json). CPU→aşama özel ayrıştırma yapılmadı; CPU request toplamıdır. Kilidi hangi requestin tuttuğu ayrıca izlenmedi. Sebep atfı ve öneri son kapanış raporunda ölçüyle sınırlanır; bilinmeyene bilinmiyor denir. D ayrı engine prosesinde ölçülür. Optimizasyon uygulanmadı; WAL handle/format/256KiB/ön filtre/Blok3/4/C ABI/soğuk/uzun snapshot/100k yok.']
    (L/f'{P}-summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2),encoding='utf-8');(R/'docs/QUERY-ATTRIBUTION-50K-20261030.md').write_text('\n'.join(lines)+'\n',encoding='utf-8');print(json.dumps([dict(group=x['group'],case_id=x['case_id'],wall_p95=x['wall_p95'],cpu_p95=x['cpu_p95'],term_wait=x['stages']['query.term_cache_mutex_wait'],permit_wait=x['stages']['first.permit_condvar_wait']) for x in summary],ensure_ascii=False))
if __name__=='__main__':main()
