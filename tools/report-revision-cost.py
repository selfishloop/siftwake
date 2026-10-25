"""Summarize retained raw observations. Never opens a database for mutation."""
import csv,importlib,json,math,statistics
from pathlib import Path
b=importlib.import_module('measure-churn');ROOT,LOGS=b.ROOT,b.LOGS
P='revisions-cost-10k-20261025'
def load(path):return json.loads(path.read_text(encoding='utf-8-sig'))
def rows(path):return [json.loads(l) for l in path.read_text(encoding='utf-8-sig').splitlines()]
def quantile(values,p):
    values=sorted(values);return values[math.ceil(len(values)*p)-1]
def stats(values):return dict(n=len(values),p50_ms=quantile(values,.5),p95_ms=quantile(values,.95),max_ms=max(values))
def export(suffix,data):
    with (LOGS/f'{P}-{suffix}.csv').open('w',encoding='utf-8',newline='') as f:
        writer=csv.DictWriter(f,fieldnames=list(data[0]));writer.writeheader();writer.writerows(data)
def main():
    profile=[];rounds=[];query_samples={};query_cases={};loads=[];writers=[];counters={}
    for variant in ['before','after']:
        path=LOGS/f'{P}-{variant}-profile-r0-{variant}-profile-streamed-churn.jsonl'
        for row in rows(path):
            if row.get('phase')!='churn_round':continue
            trace=row['maintenance']['cost_profile'];assert sum(trace['critical_ns'].values())==trace['window_ns']
            item=dict(variant=variant,round=row['round'],outer_maintenance_ms=row['maintenance_ms'],window_ms=trace['window_ns']/1e6,trace_analysis_ms=trace['analysis_ns']/1e6)
            item.update({name+'_ms':trace['critical_ns'].get(name,0)/1e6 for name in ['source_copy_encode','pacing_wait','source_crc_verify','source_validate_replay','index_crc_verify','index_rewrite_open','id_map_build','posting_remap_write','planning_metadata','fsync_publish','publication_reclaim','worker_other','other']})
            item.update({name[:-3]+'_percent':trace['critical_ns'].get(name[:-3],0)/trace['window_ns']*100 for name in list(item) if name.endswith('_ms') and name not in ['outer_maintenance_ms','window_ms','trace_analysis_ms']})
            profile.append(item)
            for name,value in row['maintenance'].get('cost_timing_counters',{}).items():
                entry=counters.setdefault(name,dict(name=name,calls=0,elapsed_ns=0))
                entry['calls']+=value['calls'];entry['elapsed_ns']+=value['elapsed_ns']
    for pair in [0,1]:
        for label in ['reference','candidate-fixed','sqlite']:
            phase='compare-v2' if label=='sqlite' else 'compare-fixed-v2'
            mode='churn' if label=='sqlite' else 'revisions-10'
            path=LOGS/f'{P}-{phase}-r{pair}-{label}-streamed-{mode}.jsonl'
            raw=rows(path);baseline=next(r for r in raw if r['phase']=='churn_baseline')['disk']['retained_bytes']
            process=load(path.with_suffix('.process.json'));assert process['exit_code']==0
            for row in raw:
                if row.get('phase')=='churn_round':
                    rounds.append(dict(label=label,pair=pair,round=row['round'],baseline_bytes=baseline,retained_bytes=row['after']['retained_bytes'],ratio=row['after']['retained_bytes']/baseline,commit_ms=sum(a['commit_ms'] for a in row['attempts']),maintenance_ms=row['maintenance_ms'],source_bytes=row['stats']['source_bytes'],index_bytes=row['stats'].get('disk_index_bytes',0),segments=row['stats'].get('segments',0),os_private_peak=row['memory']['os_lifetime_peak_private_commit_bytes'],os_ws_peak=row['memory']['os_lifetime_peak_working_set_bytes'],sampled_disk_peak=process['per_round_peak_retained_bytes'].get(str(row['round'])),oracle=row['full_oracle']))
                    live=row.get('liveness_after')
                    rounds[-1].update(payload_bytes=sum(x['payload_bytes'] for x in live) if live else None,dead_payload_bytes=sum(x['dead_payload_bytes'] for x in live) if live else None,dead_records=sum(x['dead_records'] for x in live) if live else None)
                if row.get('phase')=='matrix' and label!='sqlite':
                    for case_id,record in enumerate(row['records']):
                        query_cases[case_id]=record['case']
                        query_samples.setdefault((label,case_id),[]).extend(record['samples_ms'])
            if label=='candidate-fixed':
                data=rows(LOGS/f'{P}-{phase}-r{pair}-{label}-streamed-cost-load.jsonl')
                for row in data:
                    if row['phase']=='cost_write_load':
                        assert row['active_samples']==32 and row['oracle']
                        for x in row['samples']:
                            if x['observation']['active'] and x['observation']['source_copy_witness']:
                                writers.append(dict(pair=pair,attempt=x['attempt'],commit_ms=x['result']['commit_ms'],visibility_ms=x['result']['visible_end_to_end_ms'],oracle=x['result']['ok']))
                    if row['phase']=='cost_read_load':
                        assert row['oracle']
                        loads.extend(dict(pair=pair,**x) for x in row['samples'] if x['active'])
    long_path=LOGS/f'{P}-long-r0-long-streamed-revisions-30.jsonl'
    longraw=rows(long_path);longbase=next(x for x in longraw if x['phase']=='churn_baseline')['disk']['retained_bytes']
    longprocess=load(long_path.with_suffix('.process.json'));assert longprocess['exit_code']==0
    longrounds=[dict(round=x['round'],retained_bytes=x['after']['retained_bytes'],ratio=x['after']['retained_bytes']/longbase,commit_ms=sum(r['commit_ms'] for r in x['attempts']),maintenance_ms=x['maintenance_ms'],oracle=x['full_oracle']) for x in longraw if x['phase']=='churn_round']
    queries=[];readload=[]
    for case in range(38):
        control=stats(query_samples['reference',case]);candidate=stats(query_samples['candidate-fixed',case])
        assert control['n']==candidate['n']==60
        queries.append(dict(case_id=case,text=query_cases[case]['text'],offset=query_cases[case]['offset'],limit=query_cases[case]['limit'],reference_p95_ms=control['p95_ms'],candidate_p95_ms=candidate['p95_ms'],change_percent=(candidate['p95_ms']/control['p95_ms']-1)*100,candidate_p50_ms=candidate['p50_ms'],candidate_max_ms=candidate['max_ms'],tracked=case==34))
        subset=[r for r in loads if r['case_id']==case];assert len(subset)==20
        idle=stats([r['idle_ms'] for r in subset]);loaded=stats([r['loaded_ms'] for r in subset])
        readload.append(dict(case_id=case,n=20,idle_p95_ms=idle['p95_ms'],loaded_p95_ms=loaded['p95_ms'],ratio=loaded['p95_ms']/idle['p95_ms'],loaded_max_ms=loaded['max_ms']))
    totals={label:[sum(r['maintenance_ms'] for r in rounds if r['label']==label and r['pair']==pair) for pair in [0,1]] for label in ['reference','candidate-fixed','sqlite']}
    write_stats=stats([r['commit_ms'] for r in writers]);assert write_stats['n']==64
    visible_stats=stats([r['visibility_ms'] for r in writers])
    gates=dict(maintenance_12s=all(v<=12000 for v in totals['candidate-fixed']),disk_1_3=all(r['ratio']<=1.3 for r in rounds if r['label']=='candidate-fixed') and all(r['ratio']<=1.3 for r in longrounds),write_p95_100=write_stats['p95_ms']<=100,write_max_250=write_stats['max_ms']<=250,reads_twice_idle=all(r['ratio']<=2 for r in readload),search_5percent=all(r['change_percent']<=5 for r in queries if not r['tracked']),oracle=all(r['oracle'] for r in rounds+longrounds+writers))
    kills=load(LOGS/'revisions-cost-kill-10k-20261025-summary.json')
    gates['kill']=all(r.get('oracle',r.get('reopen_full_payload_matrix_oracle_exit_code')==0) for r in kills['results'])
    finish=load(LOGS/f'{P}-finish.json')
    gates['old_reader']=any(r['label']=='old-reader' and r['exit_code']==0 for r in finish['steps'])
    trials=[]
    for path in sorted(LOGS.glob(P+'-*.jsonl')):
        if not any(x in path.name for x in ['-streamed-churn.','-streamed-revisions-10.']):continue
        if any(x in path.name for x in ['-compare-fixed-v2-','-before-profile-','-after-profile-','-sqlite-']):continue
        raw=[r for r in rows(path) if r['phase']=='churn_round'];proc=path.with_suffix('.process.json')
        reason='Tek pilot; dengeli tekrar ve yeni kapı kapsamı yok'
        if '-compare-r0-' in path.name:reason='Warm-idle/load eşleme düzeltmesi için durduruldu; eksik koşu'
        elif '-compare-fixed-' in path.name:reason='v5 flag düzeltmesi hatalı; final gate dışında'
        elif '-candidate-' in path.name:reason='v4 index_terms=true; final gate dışında'
        elif '-reference-' in path.name:reason='Ön kalibrasyon referansı; final dengeli çift yerine kullanılmadı'
        trials.append(dict(log=path.name,rounds=len(raw),maintenance_ms=sum(r['maintenance_ms'] for r in raw),last_retained_bytes=raw[-1]['after']['retained_bytes'] if raw else None,exit_code=load(proc)['exit_code'] if proc.exists() else None,reason=reason))
    peaks=dict(candidate_10=max(r['sampled_disk_peak'] for r in rounds if r['label']=='candidate-fixed'),candidate_30=max(longprocess['per_round_peak_retained_bytes'].values()))
    summary=dict(gates=gates,maintenance_totals_ms=totals,writer=write_stats,visibility=visible_stats,queries=queries,readload=readload,rounds=rounds,long=longrounds,profiles=profile,after_timing_counters=counters,trials=trials,sampled_disk_peaks=peaks,case34_status='tracked by explicit user acceptance; all values reported',all_passed=all(gates.values()))
    for suffix,data in [('stages',profile),('rounds',rounds),('queries',queries),('reads',readload),('writes',writers),('long',longrounds)]:export(suffix,data)
    export('after-counters',list(counters.values()))
    export('trials',trials)
    b.write(LOGS/f'{P}-summary.json',summary)
    render(summary)
    print(json.dumps(gates,indent=2))

def render(s):
    status=lambda value:'GEÇTİ (gözlenen)' if value else 'KALDI'
    g=s['gates'];cost=s['maintenance_totals_ms']['candidate-fixed'];disk=[r for r in s['rounds'] if r['label']=='candidate-fixed' and r['round']==10]
    out=['# Revizyon geri kazanma maliyeti — 10k / 26 Ekim 2026','',f'**Bakım ek kapısı: {status(s["all_passed"])}.** Politika varsayılan kapalıdır. Kullanıcı önceki disk/oracle/kill sonucunu kabul etti; case34 izleniyor. Bu rapor yeni bakım/yük kapısını ayrı ölçer.','',
         '| Kapı | Gözlem | Karar |','|---|---|---|',
         f'| 10 tur bakım ≤12s | {cost[0]/1000:.3f}/{cost[1]/1000:.3f}s | {status(g["maintenance_12s"])} |',
         f'| Bakım sonrası disk ≤1,3× | 10 tur {max(r["ratio"] for r in disk):.4f}×; tur30 {s["long"][-1]["ratio"]:.4f}× | {status(g["disk_1_3"])} |',
         f'| Rewrite sırasında tek commit p95≤100 / max≤250ms | n64, {s["writer"]["p95_ms"]:.3f}/{s["writer"]["max_ms"]:.3f}ms | {status(g["write_p95_100"] and g["write_max_250"])} |',
         f'| Yazma başlangıcı → oracle ile görünürlük | p95 {s["visibility"]["p95_ms"]:.3f}, max {s["visibility"]["max_ms"]:.3f}ms | Önceki 1s hedefinin altında |',
         f'| Rewrite read p95≤2×idle | En yüksek {max(r["ratio"] for r in s["readload"]):.3f}×, 38 durum/n20 | {status(g["reads_twice_idle"])} |',
         f'| Sıcak38-case p95 kontrol +%5 | case34 hariç aşan {sum(r["change_percent"]>5 for r in s["queries"] if not r["tracked"])}/37 | {status(g["search_5percent"])}; case34 ayrıca |',
         f'| Oracle / kill / eski okuyucu | {status(g["oracle"])} / {status(g["kill"])} / {status(g["old_reader"])} | Yalnız gözlenen koşullar |','',
         '## Yöntem ve sınırlar','',
         'Mevcut aynı ilk10000, aynı donör/UTF-8 kesme/revizyon marker modeli; yeni corpus yok. Her tur1000 güncelle/500sil/500ekle,100\'lük akışlı commit;10k canlı sabit. İki FDB sırası reference→candidate-fixed / candidate-fixed→reference. Reference, kabul edilmiş önceki %20/sabit32MiB/s politikasını aynı binary içinde çalıştırır. Düzeltme yalnız candidate dalındadır. Corpus/0.1 SHA koruması ve yalnız sabit FDB/SQLite slotları kullanılır.',
         'SQLite referansı aynı oturumdaki compare-v2 iki10tur churn/verify koşusudur; frozen executable değişmedi: FTS5trigram/detailnone,FULLWAL,cache2MiB,mmap0,tempFILE. Bu Rust düzeltmesi için SQLite yeniden üretilmedi. Normal38-case SQLite karşılaştırması önceki rapordadır; bu rapor SQLite için yeni60örnekli sorgu p95 iddia etmez. Eski5,6s kontrol, geri kazanma kapalı önceki ölçümdür; yeni reference geri kazanma açık eski32,4s yoludur.',
         'Normal kabul ölçümleri diagnostic feature kapalıdır. Aşama profilleri ayrı diagnostic-timing derlemesidir; yüzdeler CPU değil exclusive wall/critical timeline payıdır. Foreground join süresi o sırada çalışan worker aşamasına atanır. Ham interval kayıtları tutulur; iç içe span\'ler iki kez toplanmaz. Trace analiz süresi ayrıca CSV\'dedir. Profilin mutlak toplamı normal derlemeyle eşit kabul edilmez.',
         'Posting_remap_write, ID çevirimi+sıra/bounds kontrolü+posting akış yazımını birlikte ölçer; tek scalar map lookup\'ı izole edilmiş gibi sunulmaz. Source_validate_replay CRC+şema/native kayıt doğrulaması ve metadata kurulumunu içerir; salt CRC taraması ayrı sütundur.',
         'Yük probunda gerçek source-copy dosyası≥256KiB gözlenir ve worker hâlâ çalışırken çağrı başlar. Her koşuda32aktif yazı, her query için10eşlenmiş warm idle/load örneği; iki koşuda64ve20. Bir gerçek güncelleme her okuma çifti arasında worker\'ı preempt edip terk edilen çıktıyı sonraki publication/reclaim ile toplar. Bu yazı ve schedule/poll süreleri query timer\'ında yoktur. Bağımsız foreground writer+reader thread/snapshot API yapılmadı; Adım2 kapısı sayılmaz.',
         'Yazı/yük örneklerinin varış noktası kaynak kopyasıdır; tüm CRC, posting, fsync ve install evrelerine rastgele varış dağılımı ölçülmedi. Gösterilen p95/max bu gözlenen örneklerindir, her evre için üst sınır garantisi değildir. Özellikle OS I/O ve install fsync kuyruğunda en kötü gecikme açık kalır. Ham logların 20261025 prefix\'i görev başlangıcıdır; rapor tarihi yerel gün değişiminden sonra 26 Ekimdir.',
         'Nearest-rank quantile; kısa query\'lerde oran yanında mutlak ms verilir. OS cache sıcak; background/power/CPU izolasyonu yok. Private/WS OS sayaçları ayrı; örnekleme kısa sıçramayı kaçırabilir. Güç kesintisi iddiası yok. Owned/Vec yoluna yeni hız iddiası yok; bu turun kabul yolu streaming\'dir.','',
         '## Ölçülen teşhis ve seçilen düzeltme','',
         'İlk10tur eski profilinde pacing14,023s (%37,84), posting stream6,959s (%18,78), indeks yazma/açma3,791s (%10,23); toplam37,063s. Bu nedenle sabit pacing, düşük kazançlı tekrar yazım ve fazladan doğrulama/açma işi hedeflendi. ID-map tablosunu kurmak <0,001s; asıl posting maliyeti map aritmetiğiyle sınırlı değildir.',
         'Yeni seçim ≥%50 ölü payload\'ı önceler; toplam ölü payload görünür payload\'ın %20\'sini aşarsa ≥%20 segmentleri de kazanç oranıyla seçer. Bir metadata görünürlük maskesi plan ve kopya seçimi için tekrar kullanılır. Tamamen gölgelenmiş, tombstone0 segmentlerden bitişik ≤256MiB grup sadece doğrulanıp manifestten emekli edilir: kaynak/indeks çıktısı yok. En yeni tombstone bariyerleri tutulur.',
         'Boşta kopya pacing beklemez; get/search/search_into aktifken32MiB/s pacing uygulanır. Writer gelince yalnız unpublished revision worker\'ına cancellation verilir. CRC64KiB, native entry, posting refill/term sınırlarında kontrol edilir; pacing Condvar ile uyanır. Writer cancellation join\'i bitmeden input/builder lease almaz. Tamamlanmış worker install edilebilir. Fsync/OS I/O zorla kesilmez; süre sınırı formal garanti değildir. Sürekli writer bütün revision işlerini iptal ederse bakım ilerlemesi boşta yeniden planlamayı bekler; bu koşuda sürekli yük altında disk üst sınırı kanıtlanmadı.',
         'Aynı yeni kaynağın tekrar açılması kaldırıldı: son tam CRC/şema doğrulaması öncesi immutable SourceLease alınır, doğrulanmış builder/index ve aynı handle korunur. Sequence/length/mtime/binding kontrolü devam eder. WAL/source/manifest/payload/SEG sürümü veya fsync politikası değişmedi; 0.1 fixture korunur.',
         'İlk checked-object uygulamasında index_terms=true yanlışlıkla trigramları yeniden üretip sonra posting reuse yapıyordu: candidate-v4 sonucu başarısız deneme olarak korundu. İlk bool düzeltmesi SourceLease çağrısına yanlış uygulandı; candidate-v5 de gate sayılmadı. Son v6 iki isimlendirilmiş immutable/records-only helper kullanır ve yayımlanan çıktının yazma erişimini reddettiği testlidir; CRC/şema kontrolleri aynı. İlk soğuk-idle/sıcak-load smoke sonucu da gate sayılmadı; untimed warmup iki tarafta eşlendi. Referans yük probunun32yazısı tutuldu, gerekmeyen eski read kalibrasyonu saved model/manifest sequence615 commit sınırında durduruldu ve full oracle yeniden geçti. Bütün kesilen/bad trial logları korunur.','',
         '## Aşama tablosu','',
         'Hücreler ms (% toplam). Saf kaynak CRC ve native CRC/şema doğrulaması aşağıda birlikte gösterilir; ham CSV her birini ayırır. ID map kurulumu ve kalan worker/other süreleri CSV içinde tutulur; aşağıdaki baskın sütunlar toplamın tamamı değildir. [Bütün aşamalar ve yüzde payları](benchmarks/'+P+'-stages.csv).','',
         '| Sürüm/tur | Toplam ms | Kopya | Pacing | Kaynak CRC+verify | Posting remap/write | İndeks+CRC | Plan | Fsync+yayım |','|---|---:|---:|---:|---:|---:|---:|---:|---:|']
    for r in s['profiles']:
        cell=lambda *names: f'{sum(r[n+"_ms"] for n in names):.1f} (%{sum(r[n+"_percent"] for n in names):.1f})'
        out.append(f'| {r["variant"]}/{r["round"]} | {r["window_ms"]:.1f} | {cell("source_copy_encode")} | {cell("pacing_wait")} | {cell("source_crc_verify","source_validate_replay")} | {cell("posting_remap_write")} | {cell("index_rewrite_open","index_crc_verify")} | {cell("planning_metadata")} | {cell("fsync_publish","publication_reclaim")} |')
    out+=['','## Toplam maliyet ve kalan engel','', '| Motor/run | Commit toplamı s | Bakım toplamı s | İkisi s | Tepe private / WS MiB |','|---|---:|---:|---:|---|']
    for label in ['reference','candidate-fixed','sqlite']:
        for pair in [0,1]:
            subset=[r for r in s['rounds'] if r['label']==label and r['pair']==pair]
            commit=sum(r['commit_ms'] for r in subset)/1000;maint=sum(r['maintenance_ms'] for r in subset)/1000
            private=max(r['os_private_peak'] for r in subset)/2**20;ws=max(r['os_ws_peak'] for r in subset)/2**20
            out.append(f'| {label}/{pair} | {commit:.3f} | {maint:.3f} | {commit+maint:.3f} | {private:.3f} / {ws:.3f} |')
    after=[r for r in s['profiles'] if r['variant']=='after'];total=sum(r['window_ms'] for r in after)
    names=['source_copy_encode','pacing_wait','source_crc_verify','source_validate_replay','index_crc_verify','index_rewrite_open','id_map_build','posting_remap_write','planning_metadata','fsync_publish','publication_reclaim','worker_other','other']
    dominant=sorted([(n,sum(r[n+'_ms'] for r in after)) for n in names],key=lambda x:x[1],reverse=True)
    out+=['', 'Son diagnostic profilin en pahalı aşamaları: '+', '.join(f'`{n}` {v/1000:.3f}s (%{v/total*100:.2f})' for n,v in dominant[:4])+'. Bu mutlak süreler diagnostic derlemenin gözlemidir; normal derlemenin 12s sınırına birebir eklenmez.',
          'Bakım sınırı geçmediği için çalışma burada durur; varsayılan kapalı kalır. Sonraki dar çalışma, bu baskın aşamaların içindeki tekrarları azaltmak olmalıdır. Snapshot API ve küçük commit/dirty open tasarımı bu tur uygulanmadı.']
    counters=s['after_timing_counters']
    out+=['',f'Son bakım pencerelerindeki sayaçlar: {counters["merge.posting_streams"]["calls"]} posting stream, {counters["merge.posting_input_entries"]["calls"]:,} giriş ID\'si, {counters["merge.posting_output_entries"]["calls"]:,} çıktı ID\'si. Bunlar normal merge ve revision bakımının birlikte sayımıdır; ID\'ler kaynak kopyası byte\'ı değildir. Sayaçların nested süreleri aşama toplamına eklenmez. [Ayrı sayaç CSV](benchmarks/{P}-after-counters.csv).']
    ref=next(r for r in s['rounds'] if r['label']=='reference' and r['pair']==0 and r['round']==10)
    cand=next(r for r in s['rounds'] if r['label']=='candidate-fixed' and r['pair']==0 and r['round']==10)
    out+=['',f'Arama kapısı da kaldı. Onuncu tur sonunda kontrol {ref["segments"]} segment / {ref["dead_payload_bytes"]/1e6:.3f}MB ölü payload / {ref["dead_records"]} ölü kayıt; aday {cand["segments"]} segment / {cand["dead_payload_bytes"]/1e6:.3f}MB / {cand["dead_records"]} ölü kayıt tutuyor (iki tekrarda aynı byte sonuçları). Daha seyrek rewrite\'ın veri yerleşimi bedeli ölçüldü. Sorgu sürelerinin hangi iç aşamada arttığı bu tur profillenmedi; bütün artışı bu sayılara veya ölçüm dalgalanmasına kesin olarak atfetmiyoruz. Sonraki maliyet çalışmasında arama kapısı korunarak posting/indeks işini azaltmak, yalnız eşiği gevşetmekten daha uygun hedeftir.']
    out+=['','## Normal tur sonuçları','', '| Motor/run/tur | Commit ms | Bakım ms | Retained MB / oran |','|---|---:|---:|---|']
    for r in s['rounds']:out.append(f'| {r["label"]}/{r["pair"]}/{r["round"]} | {r["commit_ms"]:.1f} | {r["maintenance_ms"]:.1f} | {r["retained_bytes"]/1e6:.3f} / {r["ratio"]:.4f}× |')
    out+=['','## Otuz tur','', '| Tur | Bakım ms | Retained MB / oran |','|---|---:|---|']
    for r in s['long']:out.append(f'| {r["round"]} | {r["maintenance_ms"]:.1f} | {r["retained_bytes"]/1e6:.3f} / {r["ratio"]:.4f}× |')
    out+=['',f'5ms örneklenmiş geçici retained tepe: iki normal 10 turda {s["sampled_disk_peaks"]["candidate_10"]/1e6:.3f}MB, 30 turda {s["sampled_disk_peaks"]["candidate_30"]/1e6:.3f}MB. Bunlar örneklenen en yüksek dosya toplamıdır; kısa sıçrama veya sürekli yazıcı altında sınırsız iptal birikimi için kesin üst sınır değildir.']
    out+=['','## Bütün 38 sorgu / yük','', '| Case / sorgu / offset / limit | Ref p95 ms | Aday p95 ms | Fark | Yük / idle p95 ms | Yük p95/idle | Yük max ms |','|---|---:|---:|---:|---:|---:|---:|']
    for q,r in zip(s['queries'],s['readload']):
        text=q['text'] if len(q['text'])<60 else f'zx × {len(q["text"])//2}'
        out.append(f'| {q["case_id"]}{" (izlenen)" if q["tracked"] else ""} / {text} / {q["offset"]} / {q["limit"]} | {q["reference_p95_ms"]:.6f} | {q["candidate_p95_ms"]:.6f} | %{q["change_percent"]:+.2f} | {r["loaded_p95_ms"]:.3f} / {r["idle_p95_ms"]:.3f} | {r["ratio"]:.3f}× | {r["loaded_max_ms"]:.3f} |')
    out+=['','## Kalibrasyon ve başarısız denemeler','',
          'Aşağıdakiler final gate örneklerine katılmadı. Eksik koşunun toplamı tamamlanmış 10 tur süresi değildir. İlk load smoke yazı p95/max66,784/72,717ms idi; soğuk-idle/sıcak-load okuma kıyası geçersiz sayıldı. Ön kalibrasyon sürelerindeki farklar ortam izolasyonu olmadan bütün oturumlara aynı mutlak performans sözü verilemeyeceğini de gösterir.','',
          '| Ham deneme | Tur | Bakım s | Son disk MB | Exit | Gate dışında kalma nedeni |','|---|---:|---:|---:|---|---|']
    for r in s['trials']:
        name=r['log'].removeprefix(P+'-').removesuffix('.jsonl')
        out.append(f'| [{name}](benchmarks/{r["log"]}) | {r["rounds"]} | {r["maintenance_ms"]/1000:.3f} | {r["last_retained_bytes"]/1e6:.3f} | {r["exit_code"] if r["exit_code"] is not None else "yarıda durdu"} | {r["reason"]} |')
    out+=['','## Kanıt ve açık işler','',
          f'- [Ham özet](benchmarks/{P}-summary.json), [tur CSV](benchmarks/{P}-rounds.csv), [yazı örnekleri](benchmarks/{P}-writes.csv), [38-query CSV](benchmarks/{P}-queries.csv), [yük CSV](benchmarks/{P}-reads.csv).',
          f'- [Eski kaynak, retirement/cancel/flush/merge/cache process-kill](benchmarks/revisions-cost-kill-10k-20261025-summary.json).',
          f'- [Komut/test kayıtları](benchmarks/{P}-finish.json).',
          f'- `cargo fmt --check`, default ve diagnostic `cargo clippy --all-targets -- -D warnings`, `cargo test`, no-default lib ve workload testleri geçti. [Bağımsız kanıt kontrolü](benchmarks/{P}-proof.json) corpus/fixture SHA, frozen program ve örnek sayılarını doğrular.',
          '- Yeni emeklilik/iptal senaryoları akışlı API\'yi kullanır. Önceden var olan dört genel kill modu için CLI etiketi owned\'dır: iterator modu zaten streaming iterator\'ünü, flush/merge modları owned delta\'nın açık flush/yayım yolunu keser. Bunlar owned performans sonucu veya bütün responsive rewrite evrelerinin kill kapsamı değildir.',
          f'- İlk `kill-retire-after` hazırlığı otomatik merge nedeniyle tam ölü hedef bırakmadı ve barrier öncesi assertion ile durdu; [stderr](benchmarks/revisions-cost-kill-10k-20261025-kill-retire-after.stderr) ve başarısız komut kaydı korundu. Hazırlık normal merge\'leri önce boşaltacak şekilde düzeltildi; `retry1` ayrı logla doğrulandı. Bu ilk deneme kill kapsamı olarak sayılmadı.',
          '- [OPEN-WORK](OPEN-WORK.md).','',
          'Case34 kullanıcı kararıyla izlenir. Maliyet kapısı geçmeden snapshot/çok okuyucu veya küçük commit/dirty open adımına geçilmez. C ABI yapılmadı. Genel RAM, gerçek-soğuk, uzun vadeli tombstone ve kill sonrası input/build artıklarının temizliği önceki açık işler olarak kalır.']
    separated=[]
    for i,line in enumerate(out):
        separated.append(line)
        if line and not line.startswith(('#','|','- ')) and i+1<len(out) and out[i+1]:separated.append('')
    (ROOT/'docs/REVISION-RECLAIM-COST-10K-20261026.md').write_text('\n'.join(separated)+'\n',encoding='utf-8')
if __name__=='__main__':main()
