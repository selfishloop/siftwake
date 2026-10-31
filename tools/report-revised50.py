"""Measured gates and diagnostic attribution. Preserve all historical tables."""
import json
import math
from collections import defaultdict
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='write50-revised-20261029'
read=lambda p:json.loads(p.read_text(encoding='utf-8-sig'))
pct=lambda a,p=.95:sorted(a)[math.ceil(p*len(a))-1]
def rows(name):return [json.loads(v) for v in (L/name).read_text(encoding='utf-8').splitlines()]
def flat(x):return [v for rr in x['results'] for v in rr['records']]
def cpu(z):return sum(t['cpu_100ns'] or 0 for t in z['query_profile']['threads'])/10000
def cycles(z):return sum(t['cycles'] or 0 for t in z['query_profile']['threads'])
def stage(z,name):return z['query_profile']['counters'].get(name,{}).get('elapsed_ns',0)/1e6

def main():
    g=read(L/f'{P}-v1-gate.json');catalog=read(L/'trwiki-cap200k-20261029-v3-catalog.json')['cases']
    out=['# Yazma ve 4R profil devamı — 50k / 29 Ekim 2026','',
        '**SONRADAN REVİZE:** kullanıcı bu kapıları önceki sonuçları GÖRDÜKTEN SONRA değiştirdi. '
        'Eski kapı ve sonuç tabloları [WRITE-FOLLOWUP](WRITE-FOLLOWUP-50K-20261029.md) içinde aynen korunur. '
        'Yeni kararlar ham örneklerden yeniden hesaplandı; gözlemler değiştirilmedi. '
        'Bu tur optimizasyon yok; snapshot.mask düzeltmesi uygulanmadı.','',
        'Aynı881756DA400952500BA871F6C7BA8FF8D17E5348E5975420BC5CD90952A7CF12 kaynağın ilk50k belgesi, '
        'aynı41 sorgu ve sabit C: FDB/SQLite slotları. Yeni corpus/DB kopyası yok; private/ okunmadı. '
        'WAL/FULL akışlı API; SQLite FTS5 trigram/detail=none/columnsize=0/cache2MiB/mmap0/tempFILE. '
        'OS cache sıcak/kontrolsüz; gerçek-soğuk deneyi yapılmadı. Yüzdelikler nearest-rank ceil(p*n)-1.','',
        '## 1. Yeni kapılar: mevcut ham veri','',
        '| Kapı | Eski karar | Sonradan revize karar |','|---|---|---|',
        '| Sorgu: idle≤30ms ise loaded≤30ms, aksi halde≤2×idle | KALDI:16/41 | GEÇTİ:41/41 |',
        '| Uzun tekli: p95≤20,p99≤50,p99,9≤100ms; max bilgi | KALDI: yüklü max199,93ms | GEÇTİ: bütün FDB kolları |',
        '| Throughput≥%75 ve SQLite pooled üstü | GEÇTİ | Aynı: FDB%80,11 /SQLite%73,97 |',
        '| Dirty500 strict-open /idle | GEÇTİ | Aynı:291–292ms /4,32–4,44MiB;replay≈35ms |','',
        '| Case/sorgu | Idle p95 ms | Loaded p95 ms | Eski sınır/karar | Yeni sınır/karar |',
        '|---|---:|---:|---|---|']
    for x in g['queries']:
        if x['engine']!='fdb':continue
        i=x['case_id'];out.append(f'| {i}: {catalog[i]["text"]} | {x["idle_p95_ms"]:.3f} | {x["loaded_p95_ms"]:.3f} | {x["limit_ms"]:.3f} /{"GEÇTİ" if x["old_passed"] else "KALDI"} | {x["new_limit_ms"]:.3f} /{"GEÇTİ" if x["new_passed"] else "KALDI"} |')
    out += ['', '| Motor/kol | n | p95 ms | p99 ms | p99,9 ms | Max ms (bilgi) | >100ms adet /oran | Yeni FDB kapısı |', '|---|---:|---:|---:|---:|---:|---|---|']
    for x in g['runs']+g['pooled']:
        decision=('GEÇTİ' if x['new_passed'] else 'KALDI') if x['engine']=='fdb' else 'referans'
        out.append(f'| {x["engine"]}/{x["label"]}/{x["arm"]} | {x["n"]} | {x["p95_ms"]:.3f} | {x["p99_ms"]:.3f} | {x["p999_ms"]:.3f} | {x["max_ms"]:.3f} | {x["over100_count"]} /%{100*x["over100_ratio"]:.5f} | {decision} |')
    out += ['', 'SQLite’a kabul/max kapısı uygulanmadı. Yeni uzun-seri kuralı kısa dört-tipli tekli testin '
        '20/50/max100ms kararını değiştirmez. Kısa testin74,840ms ucunun nedeni açık. '
        f'[Yeniden hesaplanan bütün kararlar + ham SHA’lar](benchmarks/{P}-v1-gate.json).','',
        '## 2. Makine, thread sayısı ve oracle','',
        'AMD Ryzen7 7840HS:8 fiziksel,16 mantıksal çekirdek (CIM ve Python os.cpu_count doğrulaması). '
        'Dört reader thread + writer/main + bellek örnekleyici ve etkin engine worker’ları bulunur. '
        'Hibrit dört işçiye kadar çıkar fakat süreç genelinde toplam DÖRT permit vardır; bir işçi çağıran reader’dır. '
        'Dolayısıyla4×4 bağımsız ek işçi varsayımı geçersizdir. Yeni dar4R koşusunda ToolHelp ile50ms aralıkla '
        'ölçülen process thread tepesi normal10–11,trace11; writer ve engine worker’ları da dahildir. '
        'Bu örneklenen alt sınırdır; kısa yaşayan thread kaçabilir. Eski uzun koşuda thread tepesi kaydedilmemişti, '
        'ona sonradan kesin thread sayısı atamıyoruz.','',
        'Oracle ESKİ idle1R ve idle4R kayıtlarında da aktiftir: dönen hit payload/version/sequence kontrolü '
        'her sorguda yapılır. O yüzden idle tabanını yeniden üretmek gerekmedi. Idle’da değişen commit grubu '
        'yoktur; yüklü durumda son yayımlanmış gruba ek oracle uygulanır. Oracle arama intervalinin dışında, '
        'aynı proseste CPU/I/O ve pin ömrüne dahildir. Yeni bütün deneyler de oracle’lıdır. '
        'Model/katalog/history dahil process RAM, saf motor RAM’i diye sunulmaz.','',
        '## 3. Uzun commit profili','']
    complete=L/f'{P}-v2-control-design.json'
    if not complete.exists() or read(complete)['stage']!='completed':
        out += ['Tam aşama kaydı devam ediyor; kabul süreleriyle diagnostic süreler karıştırılmayacak.','']
        control_complete=False
    else:
        control_complete=True;d=read(complete);x=next(x for x in rows(d['completed'][-1]['log']) if x['phase']=='snapshot_load');writes=x['writes'];frames={z['operation_sequence']:z for z in x['write_profile']['frames']};a=[z['commit_ms'] for z in writes]
        out += [f'Profil açık16384 commit: p95={pct(a):.3f},p99={pct(a,.99):.3f},p99,9={pct(a,.999):.3f},max={max(a):.3f}ms; '
            f'ops/sn={x["ops_s"]:.3f}. Trace dropped={x["write_profile"]["dropped_frames"]}. '
            'Karşılaştırma önceki profil kapalı kontrol387,18ops/sn; bu aynı-anda kontrol değildir, '
            'ortam ve worker hazır olma zamanları değişebilir. Profil açık/kapalı farkından optimizasyon kazancı '
            'veya saf instrumentation overhead yüzdesi çıkarmıyoruz.','',
            '| >50ms sequence | Commit ms | Thread CPU ms | Prefix poll | Merge poll | Stage | WAL auth /tail | Worker drain | Publish aktif | Mask (görünür seq) |',
            '|---|---:|---:|---:|---:|---:|---|---:|---:|---|']
        detailed=[]
        for w in writes:
            if w['commit_ms']<=50:continue
            f=frames[w['sequence']];ph=f['phases'];by=defaultdict(float)
            for z in ph:by[z['name']]+=z['elapsed_ns']/1e6
            mask=';'.join(f'{z["elapsed_ns"]/1e6:.3f}({z["visible_sequence"]})' for z in ph if z['name']=='snapshot.mask')
            out.append(f'| {w["sequence"]} | {w["commit_ms"]:.3f} | {w["cpu"]["thread_cpu_ms"]:.3f} | {by["stream.prefix_poll"]:.3f} | {by["stream.merge_poll"]:.3f} | {by["stream.stage_frame"]:.3f} | {by["stream.wal_authoritative_sync"]:.3f}/{by["stream.wal_tail_sync"]:.3f} | {by["stream.worker_drain"]:.3f} | {by["stream.publish_active_snapshot"]:.3f} | {mask} |')
            detailed.append(dict(**w,trace=f,non_cpu_residual_ms=w['commit_ms']-w['cpu']['thread_cpu_ms']))
        (L/f'{P}-v2-control-slow-detail.json').write_text(json.dumps(detailed,indent=2),encoding='utf-8')
        top=sorted(detailed,key=lambda z:z['commit_ms'],reverse=True)[:4]
        out += ['', 'En büyük ölçülmüş uçların iç aşamaları:', '', '| Sequence | Commit ms | En büyük iç aşama | Wall ms | Thread CPU ms |','|---|---:|---|---:|---:|']
        for z in top:
            phase=max((v for v in z['trace']['phases'] if v['name'] in ['prefix.capture_old_wal_open','snapshot.wal_open','snapshot.mask','stream.wal_authoritative_sync']),key=lambda v:v['elapsed_ns'])
            out.append(f'| {z["sequence"]} | {z["commit_ms"]:.3f} | {phase["name"]} | {phase["elapsed_ns"]/1e6:.3f} | {z["cpu"]["thread_cpu_ms"]:.3f} |')
        out += ['', 'Büyük gecikme bu örneklerde handle açma çağrısının wall süresindedir. File::open içindeki '
            'dosya sistemi/filter/scheduler ayrımı ölçülmedi; antivirüs gibi bir alt neden kanıtlanmadı. '
            'Aynı genlikte yeni uçlar görülmesi eski uçlarla aynı neden olduklarını kanıtlamaz.','',
            'Aşama süreleri wall-time; mask/publish/poll iç içedir, sütunlar toplanmaz. WAL authoritative '
            've tail append süreleri yazma+sync içerir; saf fsync değildir. Manifest/prefix/install/join ayrımı '
            'sequence bazlı ham trace’te vardır. GetThreadTimes CPU saati kısa intervalde≈15,625ms kuantalanır. '
            'Commit−threadCPU; I/O, kilit, deschedule ve CPU sayaç hatasını içerir. ETW context-switch kaydı '
            'alınmadı: saf scheduler payı AYRI ÖLÇÜLMEDİ, kalan süre scheduler diye etiketlenmedi.','',
            'İlk profil pilotunda WAL/stage/poll trace’i eksikti; snapshot mask30–33ms görüldü, '
            '53,75–63,05ms üç uç vardı. Bu eksik koşu silinmedi. İstenen aşama dağılımı için yukarıdaki '
            'ayrı, tamamlanmış16k koşu eklendi. Tarihsel74,84ms ve125–200ms uçlarla aynı neden olduğu '
            'kanıtlanmadı: **BİLİNMİYOR**. snapshot.mask düzeltmesi yapılmadı. '
            f'[Tam aşamalı profil](benchmarks/{P}-v2-control-design.json) · '
            f'[>50ms bütün sequence’ler](benchmarks/{P}-v2-control-slow-detail.json) · '
            f'[Eksik ilk pilot](benchmarks/{P}-v1-control-design.json).','']
    out += ['## 4. Dar arama ve worker deneyi','']
    d=read(L/f'{P}-v1-queries-design.json');groups=defaultdict(list)
    for run in d['completed']:
        if run['mode']!='snapshot-one':continue
        x=next(x for x in rows(run['log']) if x['phase']=='snapshot_load')
        for z in flat(x):
            if z['writer_active']:groups['trace' if run['label'].startswith('trace') else 'normal',z['case_id']].append(z)
    out += ['Aynı dört hızlı case,4096 update/kol; normal/trace/trace/normal sırası. '
        'Her kol kontrol edilmiş setup+maintenance ile başlar; donor/query ID kümeleri aynı, sequence/yerleşim '
        'tam aynı değildir. Yeni dar4R eski41-case yükünün yerini almaz; atıf deneyi ve instrumentation '
        'etkisi için ayrıca raporlanır.','',
        '| Case | Normal p95 ms | Trace p95 ms | Δms /% | Trace ort. CPU ms/çağrı | Ort. Mcycles/çağrı |', '|---|---:|---:|---|---:|---:|']
    selected=[]
    for i in [0,3,39,40]:
        normal=groups['normal',i];trace=groups['trace',i];a=pct([z['engine_ms'] for z in normal]);b=pct([z['engine_ms'] for z in trace])
        out.append(f'| {i}: {catalog[i]["text"]} | {a:.3f} | {b:.3f} | {b-a:+.3f} /{100*(b/a-1):+.2f}% | {sum(map(cpu,trace))/len(trace):.3f} | {sum(map(cycles,trace))/len(trace)/1e6:.3f} |')
        z=sorted(trace,key=lambda z:z['engine_ms'])[math.ceil(.95*len(trace))-1];selected.append(z)
    out += ['', '| P95 çağrı case/sequence | Engine ms | Delta tarama | Posting hazırlık | Intersection | Cache lock | Permit | Mask | Thread CPU ms |', '|---|---:|---:|---:|---:|---:|---:|---:|---:|']
    for z in selected:
        out.append(f'| {z["case_id"]}/{z["sequence"]} | {z["engine_ms"]:.3f} | {stage(z,"first.delta_filter"):.3f} | {stage(z,"first.posting_setup"):.3f} | {stage(z,"first.posting_intersection"):.3f} | {stage(z,"lock.cache_wait"):.3f} | {stage(z,"first.permit_acquire"):.3f} | {stage(z,"first.visibility_mask"):.3f} | {cpu(z):.3f} |')
    (L/f'{P}-query-selected.json').write_text(json.dumps(selected,ensure_ascii=False,indent=2),encoding='utf-8')
    out += ['', 'Aynı çağrı içindeki nested wall ölçüleri; cache lock posting hazırlığının içinde olabilir, '
        'paylar toplanmaz. Case0/3: aktif delta kayıtlarının get/decode/fold/contains taraması baskın '
        '(first.delta_filter); bu örneklerde≈%80. Case39/40: posting hazırlığı ve ortak cache mutex beklemesi '
        'öne çıkıyor. CPU ortalaması bütün request thread’lerinin GetThreadTimes farkı toplamıdır; '
        'yeni/kısa worker thread’lerinde kuantalama sapması olabilir. Mcycles QueryThreadCycleTime’dan '
        'ölçüldü, ms’ye çevrilmedi. Gerçek fiziksel OS cache miss/hit ve saf scheduler payı ayrıştırılmadı. '
        'Dar trace’te case40 p95’ine≈1,10ms/+%11 instrumentation/yük farkı geldi; profil bozulmasız sayılmadı.','',
        '### Aynı eski tabanda1R ve4R: FDB ve SQLite','',
        '| Motor/sorgu | Idle1R p95 ms | Idle4R p95 ms | Fark /oran |','|---|---:|---:|---|']
    baseline=read(L/'hybrid4r50-20261029-v1-b1-design.json');oldidle=defaultdict(list)
    for run in baseline['completed']:
        if 'loaded'!=run['label'].split('-')[-1] or run['mode']=='scale-seed':continue
        for x in rows(run['log']):
            if x['phase'] in ['snapshot_idle_single','snapshot_idle_four']:
                for z in flat(x):oldidle[run['engine'],x['phase'],z['case_id']].append(z['engine_ms'])
    for i in [25,31,33,32]:
        for e in ['fdb','sqlite']:
            a=pct(oldidle[e,'snapshot_idle_single',i]);b=pct(oldidle[e,'snapshot_idle_four',i]);out.append(f'| {e}/{catalog[i]["text"]} | {a:.3f} | {b:.3f} | {b-a:+.3f}ms /{b/a:.2f}× |')
    exp=read(L/f'{P}-v1-idle-design.json');assert exp['stage']=='completed';idle=defaultdict(list)
    for run in exp['completed']:
        x=next(x for x in rows(run['log']) if x['phase']=='snapshot_idle_probe')
        for z in flat(x):idle[run['workers'],run['readers'],z['case_id']].append(z)
    out += ['', '### Ayrı temiz, sabit yerleşimde yalnız ölçüm: hibrit tavan1/2/4','',
        'Tek layout/model12 kol boyunca değişmedi; iki ters sıra. Diagnostic limit yalnız çağıran thread’de '
        'etkin, global4 permit/bütçeler/CRC aynı. Eski tabanın mutlak süreleriyle birebir karşılaştırılmaz. '
        'n=16/case/ayar1R,64/case/ayar4R. Her sorgunun CPU’su oracle dışında capture edilir.','',
        '| İşçi/sorgu | 1R p95 ms | 4R p95 ms | 1R/4R ort. CPU ms | 1R/4R ort. Mcycles | 4R permit p95 ms |','|---|---:|---:|---|---|---:|']
    for w in [4,2,1]:
        for i in [25,31,33,32]:
            a=idle[w,1,i];b=idle[w,4,i];mean=lambda f,rs:sum(map(f,rs))/len(rs)
            out.append(f'| {w}/{catalog[i]["text"]} | {pct([z["engine_ms"] for z in a]):.3f} | {pct([z["engine_ms"] for z in b]):.3f} | {mean(cpu,a):.3f}/{mean(cpu,b):.3f} | {mean(cycles,a)/1e6:.3f}/{mean(cycles,b)/1e6:.3f} | {pct([stage(z,"first.permit_acquire") for z in b]):.3f} |')
    out += ['', 'Gözlenen thread sayısı16 logical core’u aşmadı; “4 reader×4 ayrı işçi=16+thread aşırı aboneliği” '
        'açıklaması desteklenmedi. Global permit ve cache/I/O çekişmesi ayrı etkiler.1–2 işçi bazı sorguları '
        'iyileştiriyor, bazılarını kötüleştiriyor (örn.İnternetten hazinem1 işçide4R≈114ms,4 işçide≈84ms). '
        'Genel ayar değişikliği önerilmedi/uygulanmadı. ETW olmadığı için kısa deschedule/çekirdek dağılımı '
        'tam çözüldü denmiyor. OS thread sayısı runnable thread sayısı değildir; SMT’nin8 fiziksel '
        'çekirdekteki ortak kaynak çekişmesi ve dış süreç yükü bu sayaçla dışlanamaz. '
        '[Makine ve örneklenen benchmark affinity65535/Normal](benchmarks/write50-revised-20261029-hardware.json).','',
        '### Araç hataları ve sınırlar','',
        'İlk idle denemesi unsupported CLI mode snapshot-idle-only yüzünden exit1; sorgu ölçülmedi. '
        'Tamamlanan dört loaded profil ve cleanup tekrarlanmadı; desteklenen snapshot-one + idle-only env ile '
        'ayrı12-kol deneyi tamamlandı. Hata logu korundu. Çalışma model dosyası daha sonra özel query-state '
        'dosyasına taşındı; ilk control model dosyası aynı başlangıç modelinden byte-for-byte geri kondu. '
        'Kaynak DB/corpus kopyası yapılmadı. '
        f'[Dar kollar](benchmarks/{P}-v1-queries-design.json) · '
        f'[Worker deneyi/OS thread örnekleri](benchmarks/{P}-v1-idle-design.json) · '
        f'[P95 aynı çağrı detayları](benchmarks/{P}-query-selected.json).','',
        '## 5. Sıralı sonraki işler','']
    b100=L/'hybrid4r50-20261029-v1-b100-gate.json'
    if b100.exists():
        z=read(b100);out += [f'4R+100lü kapısı: **{"GEÇTİ" if z["passed"] else "KALDI"}**. '
            '[Bütün ham kapılar](benchmarks/hybrid4r50-20261029-v1-b100-gate.json).','']
        d100=read(L/'hybrid4r50-20261029-v1-b100-design.json')
        out += ['Tek TAMAMLANMIŞ dengeli çift kabul hesabında: FDB kontrol→SQLite kontrol→SQLite yüklü→FDB yüklü. '
            'Idle n=12/case, FDB loaded n=46–48/case. İlk çiftte yeni kapı kalınca ek tekrarın başlamış SQLite '
            'kolu amaçlı sonlandırıldı; bu kol kabul yüzdeliklerine/throughput’una katılmadı. '
            'İlk dört tamamlanmış kolun tamamı oracle’lıdır; kötü sonuçlar çıkarılmadı.','',
            'Belge sayısı yine50k; her kol1024 commit×100 update=102400 işlem.100k belge ölçeğine '
            'geçilmedi. Sorgu kapısı bu koşudan ÖNCE revize kuralla tanımlandı. Uzun tekli-seri20/50/p99,9 '
            'kapısı100’lük transaction’a taşınmadı;100’lük commit p95/max/RAM bilgi amaçlı.','',
            '| Motor | Kontrol ops/sn | Yüklü ops/sn | Pooled oran |','|---|---:|---:|---:|']
        for t in z['throughput']:
            out.append(f'| {t["engine"]} | {t["rates"]["control"]:.3f} | {t["rates"]["loaded"]:.3f} | %{100*t["loaded_control_ratio"]:.2f} |')
        lookup={(a['engine'],a['case_id']):a for a in z['cases']}
        out += ['', '| Case/sorgu | FDB idle p95 | FDB loaded p95 | Sınır ms | Karar | SQLite idle/loaded p95 |','|---|---:|---:|---:|---|---|']
        for i in range(41):
            a=lookup['fdb',i];b=lookup['sqlite',i]
            out.append(f'| {i}:{catalog[i]["text"]} | {a["idle_p95_ms"]:.3f} | {a["loaded_p95_ms"]:.3f} | {a["limit_ms"]:.3f} | {"GEÇTİ" if a["passed"] else "KALDI"} | {b["idle_p95_ms"]:.3f}/{b["loaded_p95_ms"]:.3f} |')
        out += ['', '| Motor/kol | Commit p95/max ms | OS peak private/WS MiB |','|---|---|---|']
        for run in d100['completed']:
            if not run['mode'].startswith('snapshot'):continue
            if run['log'] not in z.get('accepted_logs',[run['log']]):continue
            x=next(x for x in rows(run['log']) if x['phase']=='snapshot_load');a=[v['commit_ms'] for v in x['writes']];mm=x['memory']
            out.append(f'| {run["engine"]}/{run["label"]} | {pct(a):.3f}/{max(a):.3f} | {mm["os_lifetime_peak_private_commit_bytes"]/1048576:.3f}/{mm["os_lifetime_peak_working_set_bytes"]/1048576:.3f} |')
        out += ['', 'Yazıcı donörleri mevcut ilk50k’dan aynı ilk1100 uygun kayıt: gerçek metin2048–89681B, '
            'ortalama6958,13B. Bu100’lük yük,100 adet200KB belgenin aynı transaction’da değişmesini sınamaz; '
            'kısa dört-tipli tekli testte200KB ayrıca vardı. Yeniden açılışta delta1100 anahtar doğrulandı.','',
            'Bellek bütün benchmark prosesidir; model/history/oracle dahildir. Yüklü oracle her yeni '
            'snapshot’ta son100 kayıtlık commit grubunu ek doğrular. Bu iş arama intervalinin dışında fakat '
            'CPU/I/O ve pin ömrüyle yükün içindedir; idle’da böyle değişen grup yoktur. Bu ek stresin etkisi '
            'ayrı oracle-off koşusuyla ayrıştırılmadı.','']
        if not z['passed']:
            out += ['**DURULDU:** bu yeni kapı da kalınca100’lük ayrı aşama profili,500’lük drain devamı '
                've gerçek Frunner anahtarı tekrarı başlatılmadı. Henüz ölçülmeyen işler bitmiş sayılmadı. '
                'Önceki500 worker-drain kanıtı WRITE-PROFILE raporunda duruyor; yeni100 profiline genellenmedi.','',
                'Ek SQLite kolunun exit4294967295 çıkışı araç kimliği doğrulanarak stop-on-failure nedeniyle '
                'yapılan amaçlı TerminateProcess’tir. Wrapper bunu RuntimeError/worker_exit olarak kaydetti; '
                'açıklanamayan çökme veya dayanıklılık testi diye sunulmaz. O SQLite slotu '
                'aborted_requires_owner_reset işaretlendi, hazır oracle DB’si sayılmaz. '
                '[Amaçlı durma kaydı](benchmarks/hybrid4r50-20261029-v1-b100-intentional-gate-stop.json).','']
    else:out+=['4R+100lü henüz sonuçlanmadı.100lü aşama profili/500lü worker drain ve gerçek Frunner anahtarı '
        'yalnız önceki kapılar geçince sırayla yapılacak.','']
    recfile=L/'write50-revised-20261029-b100-recovery.json'
    if recfile.exists():
        recovery=read(recfile);assert recovery['live_sources_unchanged'] and recovery['old_reader']['oracle']
        out += ['## 6. Doğrulama ve kalan işler','',
            'İlk tamamlanan100lü yüklü FDB durumu üzerinde3 strict-open tanıklı kill/reopen,50000 payload/version '
            'GET ve eski okuyucu41 exact sorgu geçti. Her kill tanığında delta1100 ve aynı sequence vardı. '
            'Canlı kaynak/current manifest ve bütün .fdb/.ready hash’leri aynı kaldı. Append ortası veya '
            'elektrik kesintisi iddiası yok. '
            '[Kurtarma/eski okuyucu/evre envanteri](benchmarks/write50-revised-20261029-b100-recovery.json).','',
            'cargo fmt --check,cargo clippy --all-targets -- -D warnings,diagnostic clippy geçti. '
            'cargo test --all-targets:243 geçti,15 mevcut ignored. Diagnostic request-local parent/child '
            'izolasyon testi ayrıca geçti. No-default/all-features lib check ve izole SQLite fmt/clippy/test geçti. '
            'Kaynak değişiklikleri profiling/timing ve benchmark araçlarıdır; varsayılan motor arama/yazma '
            'algoritması ve dosya biçimi değiştirilmedi. Normal kabul koşularında trace kapalıdır. '
            '[Default test logu](benchmarks/write50-revised-20261029-v2-test.log) · '
            '[Clippy](benchmarks/write50-revised-20261029-v2-clippy.log) · '
            '[Diagnostic izolasyon](benchmarks/write50-revised-20261029-v2-request-test.log) · '
            '[Son kapsam/ham SHA kontrolü](benchmarks/write50-revised-20261029-final-audit.json).','',
            '## 7. Sadece öneri — uygulanmadı','',
            'En dar sonraki aday: capped first-page aktif delta taramasını key sırasıyla N eşleşmede durdurmak. '
            'Mevcut first_delta bütün shadow’ı dolaşıp her belgeyi get/decode/fold/contains’den geçiriyor. '
            'İlkN sonucu isteyen offset0 yolunda, delta içindeki ilkN eşleşmeden sonraki bir delta anahtarı '
            'global ilkN’e giremez; önünde zatenN daha küçük delta eşleşmesi vardır. Tam shadow görünürlük '
            'maskesi korunur, eski revizyon saklama kuralı değişmez. Exact/serial search aynı kalır. '
            'Bu bir ön filtre veya biçim değişikliği değildir. Beklenen ms kazancı henüz ÖLÇÜLMEDİ. '
            '100lü yükte ayrı aşama profili durma kuralıyla yapılmadığından onun kesin kök nedeni diye sunulmaz; '
            'adayın dayanağı case0/3 için ölçülmüş delta maliyeti ve delta1100 tanığıdır.','',
            'Onay verilirse dar oracle kapsamı: N’in iki yanında delta/source anahtar sırası, güncelleme/silme, '
            'full-shadow maskesi, snapshot sequence, cancel ve strict CRC davranışı. Önce aynı100lü kapısı '
            'tekrar ölçülür; mask hazırlığına/worker tavanına dokunulmaz. WAL handle açmayı worker’da hazırlama '
            'ayrı öneridir; bu tur uygulanmadı. snapshot.mask düzeltmesi ayrı onay bekliyor.','']
    out += ['Biçim/256KiB/ön filtre/Blok3/4/C ABI/gerçek-soğuk/uzun snapshot/100k/dış entegrasyon yok. '
        'snapshot.mask düzeltmesi ayrı onay bekliyor;74,84ms/tarihsel125–200ms nedenleri açık. '
        'Case34/the zx, signed-zero, RAM/disk, yazma/bakım ve eşzamanlı yük açık listede korunur.','']
    report=R/'docs/WRITE-REVISED-PROFILE-50K-20261029.md';report.write_text('\n'.join(out),encoding='utf-8')
    if control_complete:
        (L/f'{P}-profile4-complete.json').write_text(json.dumps(dict(completed=True,gate_passed=g['passed'],control_profile=str(complete.name),query_profile='completed; original idle-dispatch error preserved',oracle_idle=True,optimization=False,report=str(report.relative_to(R))),indent=2),encoding='utf-8')
    print(json.dumps(dict(report=str(report),profile4_complete=control_complete)))

if __name__=='__main__':main()
