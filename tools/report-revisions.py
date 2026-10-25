"""Read-only calculations over saved step-1 observations; no DB/corpus generation."""
import csv
import importlib
import json
import statistics
import zlib
from pathlib import Path

bench = importlib.import_module('measure-churn')
ROOT, LOGS = bench.ROOT, bench.LOGS
P = 'revisions-10k-20261025-v2'


def load(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))


def rows(label, pair, mode=None, api='streamed'):
    mode=mode or ('revisions-30' if pair==2 else 'revisions-10')
    return [json.loads(s) for s in (LOGS/f'{P}-r{pair}-{label}-{api}-{mode}.jsonl').read_text(encoding='utf-8').splitlines()]


def export(name, records):
    with (LOGS/name).open('w',newline='',encoding='utf-8') as out:
        writer=csv.DictWriter(out,fieldnames=list(records[0]))
        writer.writeheader();writer.writerows(records)


def payload_lengths():
    # OFF1 encoded-size formula, independently reconstructed from payload.rs.
    # Only numeric lengths persist; original corpus is opened read-only.
    def nstr(v): return len(v.encode('utf-8'))
    def node(v):
        if v is None or isinstance(v,bool): return 1
        if isinstance(v,(int,float)): return 9
        if isinstance(v,str): return 5+nstr(v)
        if isinstance(v,list): return 5+sum(map(node,v))
        return 5+sum(4+nstr(k)+node(x) for k,x in v.items())
    index=load(LOGS/'churn-10k-20261024-corpus-index.json')
    lengths=[]
    with (ROOT/'data/rkyv-corpus-20261009/json.payload-bench').open('rb') as source:
        for entry in index['entries']:
            source.seek(entry['offset']); raw=source.read(entry['size'])
            assert zlib.crc32(raw)==entry['crc']
            doc=json.loads(raw)['document']
            body=doc['text'].encode('utf-8')[:51140]
            # Ignore only an incomplete final UTF-8 scalar, as Rust trimmed().
            body=body.decode('utf-8',errors='ignore').encode('utf-8')
            length=44+6+5+10+nstr(doc['title'])+len(body)+1+len('zqv0000000000r0000000000zq')+8
            length+=sum(4+nstr(k)+node(v) for k,v in doc['fields'].items())
            length+=sum(12+sum(nstr(k[x]) for x in ['scope','collection','id']) for k in doc['links'])
            lengths.append(length)
    bench.write(LOGS/f'{P}-payload-lengths.json',dict(method='OFF1 formula; corpus read-only CRC checked; only 10000 numeric sizes',lengths=lengths))
    return lengths


def logical_payload_by_round(lengths):
    model={i:(i,0) for i in range(10000)}
    next_id=10000
    result=[]
    for turn in range(30):
        ids=sorted(model)
        total=sum(lengths[b] for b,_ in model.values()); minimum=total
        changes=[]
        for at in range(1500):
            doc_id=ids[(turn*683+at)%10000]
            old,rev=model[doc_id]
            changes.append((doc_id,((old+997+turn*31)%10000,rev+1) if at<1000 else None))
        for _ in range(500):
            changes.append((next_id,((next_id*997)%10000,1)));next_id+=1
        for offset in range(0,2000,100):
            for doc_id,new in changes[offset:offset+100]:
                if doc_id in model: total-=lengths[model[doc_id][0]]
                if new is None: del model[doc_id]
                else: model[doc_id]=new;total+=lengths[new[0]]
            minimum=min(minimum,total)
        assert len(model)==10000
        result.append(dict(round=turn+1,min_visible_payload_bytes=minimum,final_visible_payload_bytes=total))
    return result


def main():
    design=load(LOGS/f'{P}-design.json');assert design.get('complete')
    protection=bench.protected();assert protection==design['protected_before']==design['protected_after']
    logical=logical_payload_by_round(payload_lengths())
    disk=[];segments=[];queries=[];single=[];query_samples={}
    for label in ['control','candidate','sqlite']:
        for pair in ([0,1,2] if label=='candidate' else [0,1]):
            data=rows(label,pair)
            baseline=next(x for x in data if x['phase']=='churn_baseline')['disk']['retained_bytes']
            process=load(LOGS/f'{P}-r{pair}-{label}-streamed-{"revisions-30" if pair==2 else "revisions-10"}.process.json')
            for entry in data:
                if entry['phase']=='churn_round':
                    turn=entry['round'];min_live=logical[turn-1]['min_visible_payload_bytes']
                    peak=process['per_round_peak_retained_bytes'].get(str(turn),0)
                    live=entry['liveness_after']
                    if label!='sqlite':
                        assert sum(x['payload_bytes']-x['dead_payload_bytes'] for x in live)==logical[turn-1]['final_visible_payload_bytes']
                    disk.append(dict(label=label,pair=pair,round=turn,baseline_bytes=baseline,
                                     retained_bytes=entry['after']['retained_bytes'],ratio=entry['after']['retained_bytes']/baseline,
                                     commit_ms=sum(x['commit_ms'] for x in entry['attempts']),maintenance_ms=entry['maintenance_ms'],
                                     sampled_peak_retained_bytes=peak,min_visible_payload_bytes=min_live,
                                     peak_over_payload_lower_bound=peak/min_live,
                                     source_bytes=entry['stats']['source_bytes'],index_bytes=entry['stats'].get('disk_index_bytes',0),
                                     segments=entry['stats'].get('segments',0),
                                     phase_private_bytes=entry['memory']['sampled_phase_peak_private_bytes'],
                                     phase_ws_bytes=entry['memory']['sampled_phase_peak_working_set_bytes'],
                                     os_peak_private_bytes=entry['memory']['os_lifetime_peak_private_commit_bytes'],
                                     os_peak_ws_bytes=entry['memory']['os_lifetime_peak_working_set_bytes'],full_oracle=entry['full_oracle']))
                    for when in ['before','after']:
                        for seg in entry[f'liveness_{when}'] or []:
                            segments.append(dict(label=label,pair=pair,round=turn,when=when,**seg,
                                                 dead_payload_fraction=seg['dead_payload_bytes']/max(seg['payload_bytes'],1)))
                elif entry['phase']=='matrix':
                    for case_id,record in enumerate(entry['records']):
                        if pair<2:
                            query_samples.setdefault((label,case_id),[]).extend(record['samples_ms'])
                        queries.append(dict(label=label,pair=pair,round=int(entry['label'][5:]),case_id=case_id,
                                            text=record['case']['text'],offset=record['case']['offset'],limit=record['case']['limit'],
                                            **record['timing']))
            for api in ([] if pair==2 else ['streamed','owned']):
                for entry in rows(label,pair,'latency-single',api):
                    if entry['phase']=='latency':
                        assert all(x['ok'] for x in entry['samples'])
                        for at,sample in enumerate(entry['samples']):
                            single.append(dict(label=label,pair=pair,api=api,kind=entry['kind'],sample=at,
                                               commit_ms=sample['commit_ms'],visibility_ms=sample['visible_end_to_end_ms']))
    export(f'{P}-rounds.csv',disk);export(f'{P}-segments.csv',segments)
    export(f'{P}-queries.csv',queries);export(f'{P}-single.csv',single)
    def quantile(values,percent):
        values=sorted(values);return values[(len(values)*percent+99)//100-1]
    regressions=[]
    for q in queries:
        if q['label']=='candidate' and q['round']==10 and q['pair']<2:
            control=next(c for c in queries if c['label']=='control' and all(c[k]==q[k] for k in ['pair','round','case_id']))
            if q['p95_ms']>control['p95_ms']*1.05:
                regressions.append(dict(**q,control_p95_ms=control['p95_ms'],p95_change_percent=(q['p95_ms']/control['p95_ms']-1)*100))
    pooled=[]
    for label in ['control','candidate','sqlite']:
        for case_id in range(38):
            samples=query_samples[label,case_id]
            assert len(samples)==60
            pooled.append(dict(label=label,case_id=case_id,n=60,p50_ms=quantile(samples,50),p95_ms=quantile(samples,95),max_ms=max(samples)))
    pooled_regressions=[]
    for q in pooled:
        if q['label']=='candidate':
            control=next(x for x in pooled if x['label']=='control' and x['case_id']==q['case_id'])
            if q['p95_ms']>control['p95_ms']*1.05:
                pooled_regressions.append(dict(**q,control_p95_ms=control['p95_ms'],p95_change_percent=(q['p95_ms']/control['p95_ms']-1)*100))
    export(f'{P}-queries-pooled.csv',pooled)
    candidate=[r for r in disk if r['label']=='candidate']
    ten=[r for r in candidate if r['round']==10]
    kills=load(LOGS/'revisions-kill-10k-20261025-summary.json')
    gates=dict(disk_10=all(r['ratio']<=1.3 for r in ten),
               temporary_stronger_payload_bound=all(r['peak_over_payload_lower_bound']<=2 for r in candidate),
               search_unexplained_regressions=len(pooled_regressions),
               single_visibility_1s=max(x['visibility_ms'] for x in single if x['label']=='candidate' and x['api']=='streamed')<=1000,
               oracle=all(r['full_oracle'] for r in disk),
               kill=all(r['coverage_observed'] and r['reopen_full_payload_matrix_oracle_exit_code']==0 for r in kills['results']))
    probe_prefix='revisions-probes-10k-20261025-background-fdb-streamed-'
    read_probe=[json.loads(line) for line in (LOGS/f'{probe_prefix}revision-reads.jsonl').read_text(encoding='utf-8').splitlines()]
    write_probe=next(json.loads(line) for line in (LOGS/f'{probe_prefix}revision-write.jsonl').read_text(encoding='utf-8').splitlines() if json.loads(line)['phase']=='rewrite_write')
    active_reads=next(x for x in read_probe if x['phase']=='rewrite_reads')
    gates['background_read_oracle']=active_reads['oracle']
    gates['background_single_visibility_1s']=write_probe['under_worker']['visible_end_to_end_ms']<=1000
    summary=dict(gates=gates,regressions=regressions,pooled_regressions=pooled_regressions,pooled_queries=pooled,rounds=disk,logical_payload=logical,
                 query_rows=len(queries),segment_rows=len(segments),single_samples=len(single),
                 quantiles={f'{label}/{api}/{kind}':dict(commit_p50_ms=quantile([s['commit_ms'] for s in single if s['label']==label and s['api']==api and s['kind']==kind],50),
                         commit_p95_ms=quantile([s['commit_ms'] for s in single if s['label']==label and s['api']==api and s['kind']==kind],95),
                         visibility_p95_ms=quantile([s['visibility_ms'] for s in single if s['label']==label and s['api']==api and s['kind']==kind],95),
                         visibility_max_ms=max(s['visibility_ms'] for s in single if s['label']==label and s['api']==api and s['kind']==kind))
                    for label in ['control','candidate','sqlite'] for api in ['streamed','owned'] for kind in ['update','delete','insert','mixed']},
                 protected_unchanged=True,background_read=active_reads,background_write=write_probe,
                 background_case_coverage=sorted({x['case_id'] for x in active_reads['samples'] if x['job_pending_at_start']}))
    gsingle=summary['quantiles']
    gates['single_visibility_regressions']=[kind for kind in ['update','delete','insert','mixed']
        if gsingle[f'candidate/streamed/{kind}']['visibility_p95_ms']>gsingle[f'control/streamed/{kind}']['visibility_p95_ms']*1.05]
    bench.write(LOGS/f'{P}-summary.json',summary)
    render(summary,segments,queries)
    print(json.dumps(gates,indent=2))


def render(s,segments,queries):
    g=s['gates']; rounds=s['rounds']
    candidate=[r for r in rounds if r['label']=='candidate']
    ten=[r for r in candidate if r['round']==10]
    extra=[r for r in candidate if r['pair']==2]
    passed=(g['disk_10'] and g['temporary_stronger_payload_bound'] and not s['pooled_regressions'] and not g['single_visibility_regressions'] and
            g['single_visibility_1s'] and g['oracle'] and g['kill'] and g['background_read_oracle'] and g['background_single_visibility_1s'])
    status=lambda b:'GEÇTİ (gözlenen)' if b else 'KALDI'
    out=['# Ölü revizyon geri kazanımı — 10k / 25 Ekim 2026','',
         f'**Adım 1: {status(passed)}.** Sonraki adımların kapısı bu rapordaki bütün alt koşullara bağlıdır.',
         'Snapshot ve küçük commit yolu bu ölçümün kapsamına dahil edilmedi. C ABI uygulanmadı.','',
         '**Yayım kararı:** Geri kazanma uygulaması korunur; normal `Database::open` politikasında varsayılan kapalıdır. İsteyen çağıran `set_revision_reclamation(true)` ile açık bakımda etkinleştirir veya doğrudan `start_revision_reclamation()` çağırır. Benchmark adayları açık, kontroller kapalı ölçülmüştür. Arama kabulü geçmeden normal davranışa alınmadı.','',
         '| Adım | Durum |','|---|---|',
         '| 1 — Ölü revizyon temizliği | KALDI: ishmael offset130/limit1 birleşik p95 +%9,77; nedeni aşama bazında henüz açıklanmadı |',
         '| 2 — Snapshot / paralel yazıcı | BAŞLATILMADI: Adım1 kapısı bekleniyor |',
         '| 3 — Küçük commit / dirty open | BAŞLATILMADI: önceki kapılar bekleniyor |','',
         '## Kabul tablosu','',
         '| Kapı | Gözlem | Karar |','|---|---|---|',
         f'| 10 tur sonrası retained ≤1,3× | En yüksek {max(r["ratio"] for r in ten):.4f}×; {max(r["retained_bytes"] for r in ten)/1e6:.3f} MB | {status(g["disk_10"])} |',
         f'| Tur içi geçici tepe ≤2× canlı | 5ms örneklenen en yüksek tepe / turdaki en düşük görünür payload: {max(r["peak_over_payload_lower_bound"] for r in candidate):.4f}× | {status(g["temporary_stronger_payload_bound"])}; payload gerçek canlı DB alanının alt sınırıdır |',
         f'| Tek belge görünürlük ≤1s | Akışlı adayın en kötü üst sınırı {max(v["visibility_max_ms"] for k,v in s["quantiles"].items() if k.startswith("candidate/streamed/")):.3f} ms | {status(g["single_visibility_1s"])} |',
         f'| Solo görünürlük p95, kontrolün +%5 sınırı | Sınırı aşan tipler: {", ".join(g["single_visibility_regressions"]) or "yok"} | {status(not g["single_visibility_regressions"])} |',
         f'| 38 durum pooled p95, kontrolün +%5 sınırı | Birleşik 60 örnekte sınırı aşan {len(s["pooled_regressions"])} / 38; ayrı çiftlerde {len(s["regressions"])} / 76 | {status(not s["pooled_regressions"])}; bütün artışlar aşağıda |',
         f'| Worker sürerken tek belge görünürlük ≤1s | {s["background_write"]["under_worker"]["visible_end_to_end_ms"]:.3f} ms (tek gözlem; p95 değildir) | {status(g["background_single_visibility_1s"])} |',
         f'| Oracle / process kill | 90 tamamlanan tur; beş dışarıdan gözlenmiş kill/reopen | {status(g["oracle"] and g["kill"])} |','',
         '## Deney ve sınırlar','',
         'Aynı mevcut ilk 10.000 belge; corpus üretilmedi/indirilmedi. Gövdeler önceki churn iş yükündeki aynı donör ve UTF-8 kesme/revizyon marker kurallarını kullanır; title/fields/links donörden gelir. Her tur 1000 güncelleme + 500 silme + 500 ekleme, 100\'lük akışlı commitler ve tam bakım. Canlı belge sayısı 10k kalır.',
         'Aynı executable içinde kontrol yalnız geri kazanma politikasını kapatır. İki dengeli sıra: kontrol→aday→SQLite, SQLite→aday→kontrol. Her biri aynı 10 turda sorgu ve tek belge kıyasına girer. Ayrı sıfırlanan aday slotunda 30 tur sürekli kullanım koşusu yapılır; bu ek koşu SQLite ile eşlenmiş 30 tur kıyası değildir.',
         'Yalnız iki sabit DB slotu kullanılır; reset sahiplik/reparse kontrolleri olan benchmark-workspace.ps1 üzerinden. DB kopyası tutulmaz. Corpus ve 0.1 fixture SHA değerleri önce/sonra eşit.',
         'FDB strict/synced. SQLite FTS5 trigram/detail=none, FULL WAL, 2MiB cache, mmap=0, temp_store=FILE, statement cache=64. Owned etiketi SQLite\'ta yine tek belge beslemedir; caller Vec RAM simetrisi yoktur. OS cache sıcak; soğuk-cache/güç kesintisi iddiası yok. Arka plan/power izolasyonu yok.',
         'Commit toplamlarına adapter\'ın corpus CRC/JSON ve belge hazırlaması dahildir; oracle süreleri commit toplamına dahil değildir. Görünürlük commit başlangıcı→get/tam payload+eski/yeni marker doğrulaması bitiş üst sınırıdır. Sorgular 30 örnek/durum; nearest-rank p95, iki çalışma güven aralığı değildir. RAM private ve WS ayrı OS sayaçlarıdır; 10ms faz örneklemesi kısa sıçramayı kaçırabilir.',
         'Disk bütün çalışma alanı dosyalarıyla 5ms örneklenir; kısa ömürlü dosyalar kaçabilir. Geçici kapı, modeldeki her 100\'lük commit sınırında bağımsız OFF1 boyut formülüyle hesaplanan en düşük görünür payload toplamına karşı sınanır. Metadata/posting/dizin alanını dışarıda bırakan bu alt sınır, 2× gerçek canlı alan koşulundan daha sıkıdır; kaynak liveness toplamlarıyla her tur eşleştirilir.','',
         '## Teşhis ve politika','',
         'Eski seçim bitişik 2–4 segment, ≤100k kayıt, ≤64MiB toplam kaynak ve en büyük/en küçük kayıt oranı ≤4 koşullarını uygular. Otomatik seçim ayrıca dört benzer input ister. İlk beş ~53–54MB kaynağın ikili toplamı bile 64MiB\'ı aşar. Seçim ölü byte oranını hiç dikkate almaz.',
         'Kontrolün 10. turunda ilk beş segmentin payload\'ının %100\'ü ölüdür; diğer segmentlerde de büyük miktarda ölü payload vardır. Bu gerçek encoded payload uzunluğu hesabıdır; posting/dictionary alanının canlı/ölüye tam fiziksel dağılımı ölçülmedi. Segment başına kaynak/indeks, payload, ölü payload, kayıt ve tombstone sayıları CSV\'dedir.',
         'Yeni açık bakım önce mevcut boyut katmanı merge\'lerini, ardından ≥%20 ve ≥64KiB ölü payload taşıyan tek segmenti boşaltır. Job başına kaynak ≤256MiB, ID map ≤1MiB, tek worker; eski kaynak/builder admission havuzları korunur. Kaynak kopyası 1MiB parçalarla 32MiB/s\'ye pace edilir; kaynak doğrulama okuması ve türetilmiş indeks I/O\'su bant genişliği sınırlı değildir, tek segment işiyle sınırlıdır.',
         'Yalnız daha yeni immutable segmentte eş anahtarı bulunan kayıtlar atılır. Aktif/WAL delta\'sı silmeye yetki vermez. En yeni tombstone tutulur; eski put canlanamaz. Sıra, payload version ve mevcut codec korunur, posting ID\'leri filtrelenerek yeniden eşlenir. Tam kaynak CRC ve indeks doğrulaması çıktı yayımından önce geçer.',
         'En yeni tombstone\'lar bu prototipte koşulsuz tutulur. 30 turdaki plato gözlemi süresiz sabit disk üst sınırı kanıtı değildir; silinen anahtar metadata\'sının uzun vadeli geri kazanımı ayrıca açıktır.',
         'Worker eski kaynakları değiştirmez. Yayıma kadar okuma eski tutarlı segmenti kullanır; install mevcut atomik manifest yolunu ve Weak/pin emeklilik mantığını kullanır. Planlama paged metadata tarar; otomatik foreground yazma yoluna bu tarama eklenmedi. Uygulama açık bakım çağırınca arka plan işi başlar. Aynı &mut Database\'e paralel yazıcı/snapshot sözleşmesi Adım 2\'dir; bu değişiklik onu çözmüş sayılmaz.','',
         '## Tur başına yazma, bakım ve alan','',
         'Tam 90 satır ve bellek sayaçları [rounds CSV](benchmarks/'+P+'-rounds.csv) içinde. Aşağıda her kontrol/adaya ait bakım sonrası alan, commit toplamı ve bakım ms yer alır; bunlar önceki churn tablosuyla aynı sınırları kullanır.','',
         '| Motor/run/tur | Disk MB / oran | Commit toplam ms | Bakım ms | Tepe private/WS MB |','|---|---|---:|---:|---|']
    diagnosis=[r for r in segments if r['label']=='control' and r['pair']==0 and r['round']==10 and r['when']=='after']
    pair_bytes=[a['source_bytes']+b['source_bytes'] for a,b in zip(diagnosis,diagnosis[1:])]
    assert len(diagnosis)==15 and min(pair_bytes)>64*1024*1024
    table=['','Kontrol run0/tur10 segment teşhisi:','',
           '| Source ID | Kaynak/indeks MB | Ölü payload MB / % | Kayıt/görünür/tombstone |','|---:|---|---|---|']
    for r in diagnosis:
        table.append(f'| {r["source_id"]} | {r["source_bytes"]/1e6:.3f}/{r["index_bytes"]/1e6:.3f} | {r["dead_payload_bytes"]/1e6:.3f} / %{r["dead_payload_fraction"]*100:.2f} | {r["records"]}/{r["live_records"]}/{r["tombstones"]} |')
    table+=['',f'Bu 15 segmentte **bütün bitişik ikili kaynak toplamları** {min(pair_bytes)/1e6:.3f}–{max(pair_bytes)/1e6:.3f} MB aralığında; hepsi 64MiB (=67,109 MB) sınırını aşıyor. Dolayısıyla bu durumdaki seçim engeli doğrudan byte sınırıdır. Görünür kayıt sayısı en yeni tombstone bariyerlerini de içerir; canlı belge sayısı değildir.','']
    # Insert before the next section, after the policy explanation.
    at=out.index('## Tur başına yazma, bakım ve alan')
    out[at:at]=table
    aggregate=['','İki tekrarda 10 turun toplamlarının aritmetik ortalaması (oracle/query zamanı hariç):','',
               '| Motor | Commit toplam s | Bakım toplam s | İkisi toplam s |','|---|---:|---:|---:|']
    for label in ['control','candidate','sqlite']:
        sums=[(sum(r['commit_ms'] for r in rounds if r['label']==label and r['pair']==pair),
               sum(r['maintenance_ms'] for r in rounds if r['label']==label and r['pair']==pair)) for pair in [0,1]]
        commit=statistics.mean(x[0] for x in sums);maintenance=statistics.mean(x[1] for x in sums)
        aggregate.append(f'| {label} | {commit/1000:.3f} | {maintenance/1000:.3f} | {(commit+maintenance)/1000:.3f} |')
    # Keep the table header adjacent to its rows; insert the overview before it.
    out[-2:-2]=aggregate+['']
    for r in rounds:
        if r['pair']<2:
            out.append(f'| {r["label"]}/{r["pair"]}/{r["round"]} | {r["retained_bytes"]/1e6:.3f} / {r["ratio"]:.3f}× | {r["commit_ms"]:.1f} | {r["maintenance_ms"]:.1f} | {r["os_peak_private_bytes"]/1e6:.3f}/{r["os_peak_ws_bytes"]/1e6:.3f} |')
    out+=['','## Otuz tur','', '| Tur | Retained MB / oran | Örneklenen tepe MB | Commit / bakım ms |','|---|---|---:|---|']
    for r in extra:
        out.append(f'| {r["round"]} | {r["retained_bytes"]/1e6:.3f} / {r["ratio"]:.3f}× | {r["sampled_peak_retained_bytes"]/1e6:.3f} | {r["commit_ms"]:.1f}/{r["maintenance_ms"]:.1f} |')
    tail=[r['retained_bytes']/1e6 for r in extra[-10:]]
    out+=['',f'Son 10 tur retained aralığı {min(tail):.3f}–{max(tail):.3f} MB; 30. tur {extra[-1]["ratio"]:.4f}×. Bu sonlu gözlem tombstone metadata\'sının süresiz üst sınırını kanıtlamaz.']
    out+=['','## Tek belge — akışlı esas, owned ayrı','', '| Motor/API/tip | Commit p50/p95 ms | Görünürlük p95/max ms |','|---|---|---|']
    for k,v in s['quantiles'].items():
        out.append(f'| {k} | {v["commit_p50_ms"]:.3f}/{v["commit_p95_ms"]:.3f} | {v["visibility_p95_ms"]:.3f}/{v["visibility_max_ms"]:.3f} |')
    out+=['','## Bütün sorgu artışları','', 'Sorgular aynı mantıksal 10. tur durumunda karşılaştırılır. 30. tur ek matrisinin mantıksal durumu farklıdır; kontrolün 10. turuyla bir hız oranı verilmez. Tam p50/p95/max, sorgu/offset/limit ve bütün artışlar [queries CSV](benchmarks/'+P+'-queries.csv) ve [summary JSON](benchmarks/'+P+'-summary.json) içindedir.','',
          '| Run/case | Aday p95 ms | Kontrol p95 ms | Fark |','|---|---:|---:|---:|']
    for q in s['regressions']:
        out.append(f'| {q["pair"]}/{q["case_id"]} | {q["p95_ms"]:.6f} | {q["control_p95_ms"]:.6f} | +%{q["p95_change_percent"]:.2f} |')
    out+=['','Birleşik 60 örnekli p95 artışları:','', '| Case | Aday p95 ms | Kontrol p95 ms | Fark |','|---|---:|---:|---:|']
    for q in s['pooled_regressions']:
        out.append(f'| {q["case_id"]} | {q["p95_ms"]:.6f} | {q["control_p95_ms"]:.6f} | +%{q["p95_change_percent"]:.2f} |')
    out+=['','Ayrı çiftlerde +%5 görülen sorgular iki çiftte aynı yönde tekrar etmedi. Birleşik 60 örnekte kalan case34 (ishmael, offset130/limit1) ise belirtilen sınırı aşıyor. Okuma/doğrulama/normalleştirme/sonuç aşama ayrımı bu tur ölçülmedi; artışı kanıtsız biçimde dalgalanma sayıp kapatmadık. Sonraki iş önce bu sayfanın profilidir. En pahalı ölçülen maliyet ayrıca bakım toplamının 5,77× artmasıdır; CRC, kopyalama/pacing ve indeks yazma payları henüz ayrılmadı.']
    out+=['','## Kurtarma, uyumluluk ve doğrulama','',
          'Her değişiklikte get/tam payload ve eski-yeni marker, her tur tam canlı/dead get oracle\'ı; 10./30. tur matrislerinde toplam/sıra/version/payload. Iterator, flush, merge, yeni revizyon rewrite worker\'ı ve cache onarımı aşamalarında dış dosya gözlemi + proses askıya alma/kill kullanılır. Yeniden açmada ack edilmiş gruplar yeni ve tam, yayım öncesi grup eski ve tam kalmalıdır; pending marker oracle ayrıca sınar. Güç kesintisi iddiası yok.',
          'Yeni kalıcı codec/SEG/WAL/manifest sürümü yok. Önceki frozen okuyucu iki aday DB\'sinde verify/full payload/matris oracle\'ını çalıştırır. Corpus ve 0.1 fixture korunur. Kesik/tek byte bozuk kaynak, pin, eşzamanlı yeni delta ve kaynak CRC binding testleri eklenmiştir. Komut sonuçları ayrı verification JSON/loglarındadır.',
          'İlk kill script\'i, rewrite hazırlığında bir geçici `.pending` dosyası listelendikten sonra silinince stat sırasında FileNotFoundError ile durdu. İlk üç gerçek kill/oracle zaten geçmişti. Dizin gözlemcisi bu normal yayım yarışını atlayacak şekilde düzeltildi; başarısız gözlemci logu korundu. Kesintiden sonra tam get/matris ve pending marker oracle yeniden geçti, rewrite ve cache senaryoları ayrı kayıtlarla tamamlandı. Bu harness hatası başarılı bir kill testi diye sayılmadı.',
          f'Ayrı background read probunda aynı değişmemiş model üzerinde 38×30 arama yapılır, biten worker yalnız sorgular arasında poll/install edilir. İş pending iken okunan durum kapsamı {len(s["background_case_coverage"])}/38; en kötü arama {max(x["ms"] for x in s["background_read"]["samples"]):.3f} ms. Query süresine schedule/poll/install dahil değildir; aynı anda bağımsız foreground writer yoktur, Adım 2 kapısı sayılmaz.',
          f'Background write probunda solo tek commit {s["background_write"]["baseline"]["commit_ms"]:.3f} ms, başlatılmış rewrite sırasında tek commit {s["background_write"]["under_worker"]["commit_ms"]:.3f} ms oldu. Bu tek eşlenmemiş belge örneği p95 değildir; commit_streamed önceki job\'ı drain ettiği için bakım/yazıcı etkileşimi ayrıca açıktır.',
          'İlk 30 turluk kontrol denemesi 16 tam tur sonrası bilinçli kesildi: 1.195.838.645 byte retained ve artan oracle maliyeti. Raw dosya/SHA ve durdurma gerekçesi korunur; tamamlanmamış tur geçti sayılmaz. Nihai dengeli 10 tur serisi yeniden başlatıldı, sayıların arasından iyi koşu seçilmedi.','',
          '## Ham kanıt ve açık işler','',
          f'- [Tasarım / süreç çıkışları](benchmarks/{P}-design.json)',
          f'- [Her segmentte canlı/ölü payload](benchmarks/{P}-segments.csv)',
          f'- [Tek belge ham örnekleri](benchmarks/{P}-single.csv)',
          '- [Beş kill/reopen kanıtı](benchmarks/revisions-kill-10k-20261025-summary.json)',
          '- [İlk gözlemci hata kaydı](benchmarks/revisions-verification-20261025.json)',
          '- [Tamamlanan fmt, Clippy ve test çıkışları](benchmarks/revisions-verification-resume-20261025.json)',
          '- [Kesilen kontrol](benchmarks/revisions-10k-20261025-interrupted-control.json)',
          '- [Açık işler](OPEN-WORK.md)','',
          'Snapshot/çok okuyucu + yazıcı p95, küçük streamed commit/dirty open RAM hedefleri, genel arama gerilemeleri, yazma/bakım bedeli, soğuk yük ve kill sonrası geçici dosya temizliği yalnız bu turun kanıtı ölçüsünde kapanabilir. C ABI sonraki aşama notudur.']
    (ROOT/'docs/REVISION-RECLAIM-10K-20261025.md').write_text('\n'.join(out)+'\n',encoding='utf-8')


if __name__=='__main__':main()
