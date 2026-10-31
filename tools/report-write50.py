"""Read completed evidence only; never opens or rewrites a DB."""
import importlib.util
import json
from collections import defaultdict
from pathlib import Path

R=Path(__file__).resolve().parent.parent
spec=importlib.util.spec_from_file_location('measure',R/'tools/measure-write50.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
L=m.L;P=m.P
def read(p):return m.read(p)
def dump(p,v):m.B.write(p,v)
def stats(xs):return dict(n=len(xs),p95_ms=m.percentile(xs),max_ms=max(xs))

def main():
    d=read(L/f'{P}-design.json');post=read(L/f'{P}-postgate.json');assert d['stage']=='STOP_single_failed'
    assert d['protected_unchanged'] and d['corpus_unchanged'] and post['same_logical_workload']
    ingests=[];single=defaultdict(list);fg=defaultdict(float);sql=defaultdict(int);frames=[]
    for r in d['completed']:
        assert r['exit_code']==0
        for x in m.rows(r):
            if 'oracle' in x:assert x['oracle']
            if x['phase']=='scale_ingested':ingests.append(dict(engine=r['engine'],label=r['label'],log=r['log'],**x))
            if x['phase']=='write50_commit':single[r['engine'],x['kind']].append(x)
            if x['phase']=='seed_commit_profile' and r['engine']=='fdb':
                frames.append(x)
                for name,v in x['counters'].items():fg[name]+=v['elapsed_ns']/1e6
            if x['phase']=='sqlite_write_profile':
                for name in ['donor_ns','fold_encode_ns','sql_fts_ns','tx_commit_sync_ns']:sql[name]+=x[name]
    assert len(frames)==100
    kinds=[]
    for (e,k),xs in sorted(single.items()):
        assert len(xs)==512
        kinds.append(dict(engine=e,kind=k,commit=stats([x['commit_ms'] for x in xs]),visibility=stats([x['visibility_ms'] for x in xs]),
            private_current_max=max(x['memory']['private_bytes'] for x in xs),private_lifetime_peak=max(x['memory']['os_lifetime_peak_private_commit_bytes'] for x in xs),
            ws_lifetime_peak=max(x['memory']['os_lifetime_peak_working_set_bytes'] for x in xs),
            bad=[x for x in xs if x['commit_ms']>50 or x['visibility_ms']>50]))
    diagrows=[json.loads(v) for v in (L/f'{P}-single-diagnostic-fdb-write50-single.jsonl').read_text(encoding='utf-8').splitlines()]
    diag=[x for x in diagrows if x['phase']=='write50_commit'];assert len(diag)==1024
    lifecycle=[];events=[]
    for name in [f'{P}-lifecycle.jsonl',f'{P}-postgate-lifecycle.jsonl',f'{P}-recovery-lifecycle.jsonl']:
        lifecycle.extend(json.loads(v) for v in (L/name).read_text(encoding='utf-8').splitlines() if v)
    for name in [f'{P}-process-events.jsonl',f'{P}-process-events-v2.jsonl',f'{P}-postgate-process-events.jsonl',f'{P}-recovery-process-events.jsonl']:
        events.extend(json.loads(v) for v in (L/name).read_text(encoding='utf-8').splitlines() if v)
    a=dict(ingests=ingests,single=kinds,gate=d['gates'][0],foreground_and_overlapping_counters_ms=dict(fg),sqlite_profile_ns=dict(sql),
        diagnostic500_total_ms=sum(x['elapsed_ms'] for x in frames),slow500=sorted(frames,key=lambda x:-x['elapsed_ms'])[:10],
        diagnostic_single_slowest=sorted(diag,key=lambda x:-x['commit_ms'])[:10],matched1070=next(x for x in diag if x['sequence']==1070),
        lifecycle=lifecycle,process_events=events,recovery=post,engine_unchanged=True,optimization=False)
    dump(L/f'{P}-analysis.json',a)
    label={'update':'Güncelle','delete':'Sil','insert':'Ekle','mtime_delete_insert':'mtime: sil+ekle'}
    t=['# 50k yazma profili — 29 Ekim 2026','',
       '**KALDI; sonraki bağlı aşamalar durduruldu.** Tekli commit p95 dört işlem tipinde geçti. '+
       'Ancak sil+ekle görünürlük max76,633ms, silme50,221ms; kabul edilen50ms sınırı aşıldı. '+
       'Dirty500,4R+yazıcı ve100’lü commit profili çalıştırılmadı. Optimizasyon yapılmadı.','',
       'Ölçülen iki kök maliyet:500’lü fallback commit’te önceki merge worker’ını beklemek; '+
       'tekli tanı tekrarında ise önceki sequence’in snapshot görünürlük maskesini hazırlamak. '+
       'İlk74,840ms örneğin kesin aşaması profil kapalı koşuda kaydedilmedi; onu çözülmüş veya ölçüm gürültüsü diye sunmuyoruz.','',
       '## Kapsam ve yöntem','',
       'Kaynak SHA256 `'+d['corpus_sha256']+'`; aynı sağlanan korpusun yalnız ilk50k belgesi. '+
       '`private/` okunmadı;100k DB koşusu, yeni corpus, DB kopyası, format/256KiB sınır değişikliği yok. '+
       'Sabit C: `data/benchmark-work/fdb` ve `sqlite` alanları ownership kontrolüyle yeniden kullanıldı. '+
       'Varsayılan exact/serial, opt-in melez/cancel ve kapalı reclaim değişmedi. Akışlı API, synced WAL; '+
       'SQLite FTS5 trigram/detail=none/columnsize=0, WAL/FULL,cache2MiB,mmap0,tempFILE.','',
       'Tekli yük:256 işlem/tip/kol; iki dengeli sıra FDB/SQLite ve SQLite/FDB; toplam512 örnek/tip/motor. '+
       'Yeni gövdeler2–4/4–16/16–64/64–200KB boyut sınıflarından seçildi; ilk50k’daki en uzun199999B donör dahil. '+
       'Gövdeler kaynak dosyada değişmedi; runtime revizyonlarında özgün gövdeye tekil oracle etiketi eklendi; '+
       '200000B sınırına sığması için gerekirse UTF-8 sınırında son birkaç bayt kesildi. '+
       'En uzun yeni metin200000B. Fields/links donörden gelir. Salt donör tekrarları ve revision etiketleri '+
       'yeni bir corpus dosyası değildir. Anahtarlar mevcut numeric kimliklerdir; mtime kalıbı farklı iki numeric '+
       'anahtarda aynı transaction’da delete+put ile temsil edildi. Gerçek16hex|uzun-yol anahtar maliyeti ölçülmedi.','',
       'Ölçüm öncesinde çıkacak kayıtlar revision1 ile hazırlanıp bakım tamamlandı. SnapshotReader açık '+
       'tutuldu; commit sonrası yeni snapshot sequence/payload ve eski/yeni tekil metin oracle’ı doğrulandı. '+
       'Görünürlük, yazma başlangıcından o snapshot’ın ilk doğrulanmış arama cevabına kadar; tüm ek GET ve '+
       'oracle kontrolleri `verified_visibility_ms` alanında ayrıca yer alır. Silinen anahtar GET=None ve '+
       'eski etiket total=0; güncellenen yalnız yeni etikette bulunur. Tam50k payload/version GET oracle’ı '+
       'her kol sonunda geçti. Sıcak OS cache, eviction yok; uygulama/CPU/enerji ortamı tamamen izole edilmedi.','',
       'Bellek Windows process private commit/working set; katalog/model, snapshot, worker ve oracle dahil. '+
       'Interleaved işlem tiplerinin bellekleri ayrı süreçlerde izole edilmedi: current max tipteki gözlem, '+
       'OS lifetime peak ise o süreçte o ana kadarki tepedir.10ms örnekleyiciye ek OS peak kullanılır. '+
       'Boşta bellek yeni süreçte oracle yüklenmeden ölçüldü. Profil kapalı kabul sonuçları ile tanı süreleri karıştırılmaz.','',
       '## Tekli yazma ve görünürlük','',
       '| Motor | İşlem | n | Commit p95/max ms | Görünürlük p95/max ms | Current private max MiB | Process peak private MiB | Peak WS MiB |',
       '|---|---|---:|---|---|---:|---:|---:|']
    for x in kinds:
        c=x['commit'];v=x['visibility'];t.append(f"| {x['engine']} | {label[x['kind']]} | {c['n']} | {c['p95_ms']:.3f}/{c['max_ms']:.3f} | {v['p95_ms']:.3f}/{v['max_ms']:.3f} | {x['private_current_max']/2**20:.3f} | {x['private_lifetime_peak']/2**20:.3f} | {x['ws_lifetime_peak']/2**20:.3f} |")
    t+=['','Bütün kötü FDB örnekleri:','', '| Kol/tip | Sequence | Commit ms | Görünürlük ms | Yeni/eski donör B |','|---|---:|---:|---:|---|']
    for r in d['completed']:
        if r['engine']=='fdb' and r['mode']=='write50-single':
            for x in m.rows(r):
                if x['phase']=='write50_commit' and (x['commit_ms']>50 or x['visibility_ms']>50):t.append(f"| {r['label']} /{label[x['kind']]} | {x['sequence']} | {x['commit_ms']:.3f} | {x['visibility_ms']:.3f} | {x['donor_text_bytes']}/{x['outgoing_donor_bytes']} |")
    t+=['','## Ingest: kısa aynı-oturum A/B','',
       'Geçmiş TR38,44–39,13sn ve cap200k51,97–57,98sn koşularının ikisinde de WAL katmanı açık, '+
       '500 işlem/commit ve son idle cleanup aynıydı. Bu nedenle ayar değiştirilmiş gibi bir neden kurulmuyor. '+
       'Bu tur aynı881756DA kaynağıyla eski/güncel donmuş ikililer birer kez FDB/SQLite ve SQLite/FDB '+
       'sırasıyla çalıştırıldı. Ek güncel WAL-off FDB kontrolü yine500 işlem/commit. Tek örnek/ayar, '+
       'düşük yüzdeler için istatistiksel üstünlük veya ortamın kesin alt nedeni değildir.','',
       '| Kol | Motor | Ingest+bakım sn | Commit toplam sn | Bakım sn |500lü p95/max ms | Peak private MiB |',
       '|---|---|---:|---:|---:|---|---:|']
    for x in ingests:
        st=stats(x['commits_ms']);t.append(f"| {x['label']} | {x['engine']} | {x['ingest_including_maintenance_ms']/1000:.3f} | {x['commit_total_ms']/1000:.3f} | {x['maintenance_ms']/1000:.3f} | {st['p95_ms']:.3f}/{st['max_ms']:.3f} | {x['memory']['os_lifetime_peak_private_commit_bytes']/2**20:.3f} |")
    t+=['','Tabloya kurulum/diagnostic ve ikinci tekli-kol seed’leri de dahil: ikinci SQLite seed83sn süreç '+
       'süresine çıktı; kötü/yavaş seed gizlenmedi. Ingest kolonu Rust ölçümüdür, `.process.json` wall '+
       'süresi başlatma ve ölçüm sonrası tam oracle’ı da içerir.','',
       'Eski ikili de bugün55,395sn; güncel56,167sn. WAL-off55,303sn. Eski/güncel fark≈%1,4, '+
       'WAL-off fark≈%1,5: bu tek çiftle küçük kazanç iddiası yok. Tarihsel yaklaşık%45 artış yeni '+
       'kod/ayar farkı olarak yeniden üretilmedi; SQLite da46–47sn ile eski36sn düzeyine inmedi. '+
       '**Ortam/oturum koşulları güçlü adaydır (çıkarım); geçmiş CPU/frekans/I/O telemetrisi olmadığından '+
       'kesin ortam nedeni kanıtlanmadı.** Kaynak ilk50k’da yalnız45192’nin637B kırpılması bakımından '+
       'eski TR girdisinden farklıdır; eski girdiyi bu tur yeniden çalıştırıp birebir korpus A/B’si yapılmadı.','',
       '## 500’lü kuyruğun ölçülen aşamaları','',
       '100 diagnostic commit toplamı: '+f"{a['diagnostic500_total_ms']/1000:.3f}sn. "+
       'Aşağıdaki foreground span’leri sıralı kapsamları gösterir; alt/worker span’leriyle toplanmaz. '+
       'Wall süresidir, CPU payı değildir.','', '| Foreground aşama | Toplam sn | Commit toplamının yüzdesi |','|---|---:|---:|']
    for key in ['stream.stage_entry','stream.finish_frame_crc','stream.fallback_worker_drain','stream.fallback_delta_flush','stream.fallback_post_flush_drain','stream.fallback_transaction_publish']:
        value=fg[key];t.append(f'| {key} | {value/1000:.3f} | {value/a["diagnostic500_total_ms"]*100:.2f}% |')
    t+=['','Stage giriş süresi entry kodlama/CRC ve indeks hazırlığını kapsar; caller donör JSON okuma/çözme '+
       've diğer commit işleri kalan sürededir. Transaction publish; WAL, kaynak/index yazımı, manifest '+
       've sync içerir. Aşağıdakiler iç içe veya worker ile örtüşebilir; yüzde olarak toplanmaz.','',
       '| İç/worker sayaç | Toplam sn |','|---|---:|']
    for key in ['merge.worker','merge.join_wait','index.change_total','builder.finish_delta','builder.finish_reused','source.encode_write_frames','segment.open','open.index_binary_directory_parse','stream.wal_copy_and_sync','stream.wal_sync','source.checkpoint_sync','builder.index_sync','manifest.publish','manifest.sync']:
        t.append(f'| {key} | {fg.get(key,0)/1000:.3f} |')
    t+=['','| En yavaş500lü sequence | Commit ms | Worker drain ms | Stage entry ms | Publish ms | WAL sync ms | Manifest sync ms |','|---|---:|---:|---:|---:|---:|---:|']
    for x in a['slow500'][:5]:
        c=x['counters'];val=lambda key:c.get(key,{}).get('elapsed_ns',0)/1e6
        t.append(f"| {x['sequence']} | {x['elapsed_ms']:.3f} | {val('stream.fallback_worker_drain'):.3f} | {val('stream.stage_entry'):.3f} | {val('stream.fallback_transaction_publish'):.3f} | {val('stream.wal_sync'):.3f} | {val('manifest.sync'):.3f} |")
    t+=['','En pahalı kuyruğun doğrudan kanıtı worker drain: örneklerde1,2–1,3sn. WAL sync1–6ms '+
       'bu uçların baskın nedeni değil. İndeks hazırlığı toplam maliyette ayrıca büyüktür.100’lü commit '+
       'aynı yola düşer varsayımı yapılmadı; o profil tekli kapısı nedeniyle ölçülmedi.','',
       '| SQLite500 iç aşama | Toplam sn |','|---|---:|']
    for key,value in sql.items():t.append(f'| {key} | {value/1e9:.3f} |')
    t+=['','SQLite `sql_fts` INSERT+trigger/FTS işi; `tx_commit_sync` COMMIT ve içeride gerçekleşebilen '+
       'sync/checkpoint işidir. Saf fsync süresi diye sunulmaz; SQLite iç C aşamaları ayrıca profillenmedi.','',
       '## Tekli aşımın tanısı','',
       'Aynı mantıksal model hash’ini üreten bir1024-commit diagnostic tekrar yapıldı; source formatı aynı. '+
       'Normal74,840ms /sequence1070, tanıda3,448ms oldu: aynı nadir örnek tekrar oluşmadı. '+
       'Buna rağmen tanıda sequence1068 silme50,171ms/visibility50,396ms ile sınır tekrar aşıldı. '+
       'Ölçülen35,528ms `snapshot.mask`,35,577ms `snapshot.prepare`; WAL sync0,581ms. '+
       'İlk maskenin visible_sequence’i1067, sonraki küçük maskenin1068: önceki sürümün yayım '+
       'hazırlığı yeni commit’e yükleniyor. Worker/poll yayımından gelen bu hazırlık maliyeti somut '+
       'bir sonraki tanı/düzeltme hedefidir; ilk74,840ms’nin kesin nedeni olduğu söylenmez.','',
       'Diğer tanı örnekleri: prefix install15–16ms; WAL reader açma6,9–9,3ms. Kaynak/manifest '+
       'işlerini kaldırmak veya güvenlik kontrolünü atlamak önerilmiyor. Bu tur hiçbir optimizasyon uygulanmadı.','',
       '## Kapılar ve durma kararı','', '| Kapı | Ölçüm | Karar |','|---|---|---|',
       '| Tekli commit p95≤10ms | dört tip2,492–4,444ms | GEÇTİ |',
       '| Önceki max commit≤50ms |74,840ms; ayrıca50,028ms | KALDI |',
       '| Yazma→görünürlük≤50ms |76,633ms ve50,221ms | KALDI |',
       '| Temiz idle≤15MiB |'+f"{max(d['gates'][0]['clean_idle_private_bytes'])/2**20:.3f}MiB"+' | GEÇTİ |',
       '| Ölçülen akışlı tepe private≤60MiB | tekli21,824MiB; tüm normal seed’ler üstte | GEÇTİ |',
       '| Dirty500 strict≤1000ms /idle≤15MiB | öncül tekli kapısı kaldı | ÖLÇÜLMEDİ |',
       '|4R+yazıcı, loaded p95≤2×idle; idle<1ms için+2ms | öncül kapı kaldı | ÖLÇÜLMEDİ |',
       '| Throughput≥%75 ve SQLite pooled üstü | öncül kapı kaldı | ÖLÇÜLMEDİ |',
       '|100lü p95/max/RAM ve kök neden | öncül kapı kaldı; yeni kapı uydurulmadı | ÖLÇÜLMEDİ |',
       '| Oracle/kill/eski okuyucu | aşağıdaki kapsam | GEÇTİ |','',
       '## Kurtarma, uyumluluk ve süreç kapanmaları','',
       'Yazılmış model üzerinde üç kez normal strict-open tanığı alınıp dirty WAL açıkken child kill edildi. '+
       'Tanıkta255 delta anahtarı ve önceki tüm commit’ler vardı. Sonra strict reopen +tam50k GET/version '+
       've arama oracle’ı geçti. Bu255-kayıt kurtarma kontrolü, durdurulan dirty500 performans koşusu değildir. '+
       'Append/flush/merge ortasında yeni kill tanığı yok; elektrik kesintisi iddiası yok. '+
       'Dondurulmuş eski okuyucu50k tam GET,512 silinmiş GET ve41 exact sorguda geçti; read-only kaynak '+
       'SHA’ları değişmedi.0.1 fixture ve korunmuş eski corpus hash’leri aynı; bütün motor `src/*.rs` '+
       'hash’leri kabul edilen sürümle aynı.','',
       '17 ana ölçüm worker’ı normal exit0 ile kapandı. Sonraki diagnostic seed/tekli de exit0. '+
       'Üç kurtarma kill’i ayrı `intentional_kill` kaydında actor/target PID ile işaretli. '+
       'Bu tur ana koşuda açıklanamayan worker kaybı gözlenmedi. Önceki oturum kesintisinin kesin nedeni '+
       'geçmişe dönük kanıtlanamadı.','',
       'v2 gözlemcisindeki `driver_disappeared` satırı normal kapanıştan sonradır: driver_finish '+
       '12:45:57.335Z (STOP_single_failed), handle_exit12:45:57.887Z/exit0, ardından '+
       'poll12:45:57.906Z. Stop dosyası da mevcuttu. Bu kayıt açıklanamayan yarıda kesilme değildir.','',
       'WMI ProcessStart/StopTrace aboneliği Call cancelled/Access denied verdi; yükseltme yapılmadı. '+
       'Wrapper fsync’li PID/start/exit kayıtları ve salt-okuma process handle exit denetimi kullanıldı. '+
       'İlk izleyicide lifecycle dosyasını paylaşım izni olmadan okumak hataya neden oldu; FileShare.ReadWrite '+
       'ile düzeltilip v2 loguna yeniden başlatıldı. Bu kısa gözlem boşluğunda wrapper kayıtları kaldı; '+
       'benchmark durmadı. System/Application ve oturum olay sorgularında ölçüm aralığında eşleşen olay '+
       'bulunmadı. İzleyici de aynı oturumla öldürülürse son aktörü saptama garantisi yoktur.','',
       'Bir yardımcı girişimi ise gerçekten hata ile bitti: kirli WAL üzerinde temiz-WAL zorunlu '+
       'OpeningDatabase prototipi çağrıldı, native hata `background opening prototype requires a clean active WAL`; '+
       'boş stdout’u JSON sayan Python yardımcısı JSONDecodeError verdi. Hata logları korunup yalnız bu '+
       'doğrulama kısmı doğru strict yolla tamamlandı; ölçümler tekrar çalıştırılmadı, hata motor çökmesi '+
       'diye sunulmadı.','',
       '## Ham kanıt ve açık iş','',
       f'- [Tasarım, ikili/source hashları ve tamamlanan bütün komutlar](benchmarks/{P}-design.json).',
       f'- [p95/max/RAM, foreground/worker sayaçları ve olayların tamamı](benchmarks/{P}-analysis.json).',
       f'- [Tekli kapı](benchmarks/{P}-single-gate.json); [kurtarma ve eski okuyucu](benchmarks/{P}-postgate.json).',
       f'- [Ana süreç kayıtları](benchmarks/{P}-lifecycle.jsonl), [handle/oturum gözlemcisi](benchmarks/{P}-process-events-v2.jsonl), [kurtarma kill nedenleri](benchmarks/{P}-recovery-lifecycle.jsonl).',
       '- [243 test/fmt/clippy/feature ve SQLite kontrolleri](benchmarks/snapshot-10k-20261026-validation-write50-20261029-v4.json). Önceki v1 unused-import ve v3 harness-fmt geliştirme hataları da silinmedi.','',
       'Sonraki onay için öneri: önce snapshot yayımındaki maske hazırlığını, ardından500’lü fallback’in '+
       'worker beklemesini hedefleyen dar değişiklik. Dirty500,4R+yazıcı ve100lü profil açık kaldı. '+
       'Uzun snapshot, gerçek-soğuk cache, Blok3/4,C ABI, ön filtre ve entegrasyon başlamadı. '+
       'Eski case34 (ishmael /130/1),the zx parçalı okuma ve signed-zero maddeleri ölçümsüz kapanmadı.']
    (R/'docs/WRITE-PROFILE-50K-20261029.md').write_text('\n'.join(t)+'\n',encoding='utf-8')
    print(json.dumps(dict(stage=d['stage'],single_rows=len(kinds),diagnostic500=len(frames))))

if __name__=='__main__':main()
