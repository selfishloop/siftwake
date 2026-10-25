"""Render measured saved churn results; no DB/corpus access or inference of passes."""
import csv
import json
import math
import statistics
from pathlib import Path

ROOT=Path(__file__).resolve().parent.parent
LOGS=ROOT/'docs/benchmarks'
P='churn-concurrent-10k-20261024'
REPORT=ROOT/'docs/CHURN-CONCURRENT-10K-20261025.md'


def fmt(n):
    return f'{n:.3f}' if n is not None else '—'


def export_tables(summary):
    """Expose all recorded cases without rerunning a measurement."""
    concurrent=[]
    for run in summary['concurrent']:
        for case in run['cases']:
            row={k:run[k] for k in ('engine','api','run','batch')}
            row.update({k:case['case'][k] for k in ('text','offset','limit')})
            for label, values in [('idle',case['idle']),('loaded',case['all_reads']),('wait',case['read_wait'])]:
                row.update({f'{label}_{k}':v for k,v in values.items()})
            row['p95_ratio']=case['p95_ratio']
            row['active_samples']=case['active']['n']
            concurrent.append(row)
    burst=[]
    for name in summary['input_files']:
        if not name.endswith('-burst.jsonl'):
            continue
        for record in (json.loads(line) for line in (LOGS/name).read_text(encoding='utf-8-sig').splitlines()):
            if record.get('phase')!='matrix' or record['label']!='burst_unmaintained':
                continue
            for case in record['records']:
                values=sorted(case['samples_ms'])
                burst.append(dict(engine='sqlite' if '-sqlite-' in name else 'fdb',
                    api='owned' if '-owned-' in name else 'streamed',
                    run=int(name.split('-r')[1].split('-')[0]),**case['case'],
                    n=len(values),p50_ms=statistics.median(values),
                    p95_ms=values[math.ceil(len(values)*.95)-1],max_ms=values[-1]))
    for label, rows in [('concurrent',concurrent),('burst-search',burst)]:
        with (LOGS/f'churn-{label}-20261025.csv').open('w',encoding='utf-8',newline='') as stream:
            writer=csv.DictWriter(stream,fieldnames=list(rows[0]) if rows else [])
            writer.writeheader()
            writer.writerows(rows)
    return burst


def main():
    summary=json.loads((LOGS/f'{P}-summary.json').read_text(encoding='utf-8'))
    burst_cases=export_tables(summary)
    design=json.loads((LOGS/f'{P}-all-design.json').read_text(encoding='utf-8'))
    kill_path=LOGS/'churn-kill-10k-20261025-summary.json'
    kills=json.loads(kill_path.read_text(encoding='utf-8')) if kill_path.exists() else None
    complete=design.get('protected_unchanged') is True and not summary['failures']
    lines=['# Güncelleme/silme churn ve eşzamanlı yük — 25 Ekim 2026','',
        f'Ölçüm durumu: **{"ana koşular tamamlandı" if complete else "eksik / devam eden kanıt"}**. '
        'Bu rapor yalnız kayıtlı normal motor ölçümlerinden üretilir; kaynak/WAL/manifest/payload biçimi ve motor algoritması değiştirilmedi.','',
        'İlk 10.000 mevcut belge yeniden kullanıldı. Gövde, aynı kaynak havuzundaki başka bir belgeden seçilir; '
        'donör belgenin title/fields/links alanları da taşınır, hedef key sabitlenir. Bu nedenle yalnız gövdesi değişip metadata’sı sabit kalan dosya güncellemesinin byte özdeşi değildir. '
        'UTF-8 sınırında en fazla 51.140 byte tutulup belge/revizyon işareti eklenir (toplam ≤51.200 byte). '
        'Yeni kitap/metin corpus dosyası üretilmedi. Kimlik ve revizyon işaretleri eski metnin görünmezliğini bağımsız sınar. '
        'Bu kontrollü marker eklenmiş iş yükü, önceki değişmemiş ingest corpus’un byte özdeşi değildir.','',
        'FDB normal strict/synced profil. SQLite FTS5 trigram/detail=none, FULL WAL, 2 MiB sayfa cache, mmap=0, temp_store=FILE, '
        '64 statement cache; update/delete trigger’ları yalnız karşılaştırıcı şemasındadır. SQLite owned etiketi altında da tek belgeyle beslenir; '
        'FDB Vec giriş belleğiyle simetrik bir caller tahsisi değildir.','',
        'Her bölüm iki dengeli motor sırası kullanır: FDB→SQLite, SQLite→FDB. Yalnız sabit FDB/SQLite slotları '
        'sahiplik kontrolüyle yeniden kurulur; okuma ilgili kurulmuş DB’yi kullanır. OS cache sıcak, güç/arka plan izolasyonu yok. '
        'İki çalışma istatistiksel güven aralığı veya tüm dosya iş yükleri garantisi değildir.','',
        'Gereksinim notu `D:/Vibeprojects/frunner/docs/content-index/CODEX-GEREKSINIM.md` salt okunur okundu '
        '(istenen repo içi `docs/CODEX-GEREKSINIM.md` yok). Eski nottaki 50k/100k, derived/20ms ve C ABI işleri bu turun 10k/strict/1s talimatının yerine uygulanmadı.','',
        '## Kabul kapıları','',
        '| Kapı | Gözlenen FDB sonucu | Karar / kapsam |','|---|---|---|']
    latency=summary['latency']
    singles=[r for r in latency if r['engine']=='fdb' and r['api']=='streamed' and r['batch']==1]
    maximum=max((r['visibility_upper_bound'].get('maximum',0) for r in singles),default=None)
    lines.append(f'| Tek belge görünürlük ≤1000 ms | En kötü commit→kontrollü arama üst sınırı {fmt(maximum)} ms | {"GEÇTİ (gözlenen solo örnekler)" if maximum is not None and maximum<=1000 else "KALDI / veri eksik"}; eşzamanlı kuyruk ayrıca |')
    stream_idle=[r for r in summary['idle'] if r['engine']=='fdb' and r['api']=='streamed' and r['mode']=='idle']
    idle_private=max((r['memory']['private_bytes'] for r in stream_idle),default=None)
    idle_ws=max((r['memory']['working_set_bytes'] for r in stream_idle),default=None)
    dirty_idle=[r for r in summary['idle'] if r['engine']=='fdb' and r['mode']=='open']
    dirty_private=max((r['memory']['private_bytes'] for r in dirty_idle),default=None)
    dirty_ws=max((r['memory']['working_set_bytes'] for r in dirty_idle),default=None)
    worst_idle=max([v for v in (idle_private,dirty_private) if v is not None],default=None)
    worst_idle_ws=max([v for v in (idle_ws,dirty_ws) if v is not None],default=None)
    def memory_verdict(value,limit,covered):
        if value is not None and value>limit:return 'KALDI'
        return 'GEÇTİ (gözlenen)' if value is not None and covered else 'SONUÇSUZ / eksik'
    idle_private_status=memory_verdict(worst_idle,15_000_000,complete and len(dirty_idle)==4)
    idle_ws_status=memory_verdict(worst_idle_ws,15_000_000,complete and len(dirty_idle)==4)
    lines.append(f'| Boşta ≤15 MB (=15.000.000 B) | Temiz private/WS {fmt(idle_private/1e6 if idle_private else None)}/{fmt(idle_ws/1e6 if idle_ws else None)} MB; dirty private/WS {fmt(dirty_private/1e6 if dirty_private else None)}/{fmt(dirty_ws/1e6 if dirty_ws else None)} MB | Private: **{idle_private_status}**; WS: **{idle_ws_status}**. Her ikisi geçmeden genel RAM geçti denmez |')
    stream_open=[r for r in summary['opening'] if r['engine']=='fdb' and r['api']=='streamed']
    # Process peaks appear in phase rows, not merely at the open event.
    memory_rows=[]
    for file in LOGS.glob(f'{P}-*-fdb-streamed-*.jsonl'):
        process=file.with_suffix('.process.json')
        if not process.exists() or json.loads(process.read_text())['exit_code']!=0:
            continue
        for line in file.read_text(encoding='utf-8-sig').splitlines():
            row=json.loads(line)
            if isinstance(row.get('memory'),dict):memory_rows.append(row['memory'])
            if isinstance(row.get('result'),dict) and isinstance(row['result'].get('memory'),dict):memory_rows.append(row['result']['memory'])
    peak=max((m['os_lifetime_peak_private_commit_bytes'] for m in memory_rows),default=None)
    peak_ws=max((m['os_lifetime_peak_working_set_bytes'] for m in memory_rows),default=None)
    peak_private_status=memory_verdict(peak,60_000_000,complete)
    peak_ws_status=memory_verdict(peak_ws,60_000_000,complete)
    lines.append(f'| Akışlı indeksleme tepe ≤60 MB (=60.000.000 B) | Private {fmt(peak/1e6 if peak else None)} MB; WS {fmt(peak_ws/1e6 if peak_ws else None)} MB | Private: **{peak_private_status}**; WS: **{peak_ws_status}**. Harness ve sonuç tahsisleri dahil; genel geçti kararı iki sayacı birlikte gerektirir |')
    fdb_con=[r for r in summary['concurrent'] if r['engine']=='fdb']
    ratios=[c['p95_ratio'] for r in fdb_con for c in r['cases'] if c['p95_ratio'] is not None]
    concurrent_coverage=bool(fdb_con) and len(fdb_con)==4 and all(r['all_cases_active'] for r in fdb_con)
    worst=max(ratios,default=None)
    lines.append(f'| Eşzamanlı sorgu p95 ≤2× idle | En kötü tüm örnekler muhafazakâr oranı {fmt(worst)}× | {"KALDI" if worst and worst>2 else "GEÇTİ gözlenen" if concurrent_coverage else "SONUÇSUZ: kapsama eksik"}; aktif kapsama {concurrent_coverage} |')
    growth=[]
    for row in summary['churn']:
        if row['engine']=='fdb' and row['round']==10:
            base=next(r for r in summary['churn_baselines'] if r['engine']==row['engine'] and r['run']==row['run'])
            growth.append(row['after']['retained_bytes']/base['disk']['retained_bytes'])
    disk_worst=max(growth,default=None)
    lines.append(f'| 10 tur sonrası disk ≤1,3× | Bakım sonrası oranlar {", ".join(fmt(v) for v in growth) or "eksik"}× | {"SONUÇSUZ: veri eksik" if len(growth)!=2 else "GEÇTİ" if disk_worst<=1.3 else "KALDI"} |')
    lines.append(f'| Doğruluk oracle | Başarısız worker sayısı {len(summary["failures"])}; kill ayrı aşağıda | {"Ana tamamlanan koşular geçti" if complete else "Eksik/fail kanıt var"}; yalnız ölçülen koşullar |')
    lines+=['','## Tek belge / toplu işlem gecikmesi','',
        'Commit zamanına benchmark adapter içindeki mevcut corpus CRC/JSON okuması ve belge hazırlama da dahildir; yalnız motorun hazır belgeyle çağrı süresi değildir. '
        'Her homojen commit tipi ayrı ölçülür; mixed satırın süresi her işlem tipine pay edilmez. '
        'Mixed n=1’de tipler çağrılar arasında 50/25/25; n=10’da delete/insert yuvarlaması karşıt çağrılarda dengelenir. '
        'Her büyüklük grubu bakımı bitmiş 10k mantıksal durumdan başlar; tipler o grup içinde sıralı çalışır ve önceki tipten worker/delta kalabilir. Önceki büyüklüklerde gövdeler güncellenmiş olabilir. '
        'Homojen silme/eklemeden sonra canlı sayı ölçüm dışı geri yüklenir. Bu restore süresi latency satırına dahil değildir.','',
        'Her turda tip/batch başına n=32/12/6/3, iki tur birleşince n=64/24/12/6 olur. '
        'Nearest-rank p95 kullanılır; n<20 satırlarda p95 maksimuma eşittir. Bu az örnekler bir nüfus p95 güven aralığı değildir.','',
        'Görünürlük üst sınırı commit başlangıcından sonra get/tam payload, eski marker ve yeni marker sorgularının bitimine kadardır. '
        'Tek belgedeki 1s kapısı için muhafazakâr üst sınırdır; büyük batch’te ardışık oracle sorguları biriktiği için son belgenin gerçek ilk görünme anını ayırmaz. '
        'Flush/merge bekleme çağrısı eklenmez. **commit_streamed kendisi önceki worker’ı bekler ve başarılı çağrı sonunda delta=0 bırakır**; bu gerçek API maliyetidir.','',
        '| Motor/API | Tip | Batch | n | Commit p50/p95/max ms | Görünürlük üst p95/max ms | Red |','|---|---|---:|---:|---|---|---|']
    for r in sorted(latency,key=lambda r:(r['api'],r['batch'],r['kind'],r['engine'])):
        q=r['commit'];v=r['visibility_upper_bound']
        lines.append(f'| {r["engine"]}/{r["api"]} | {r["kind"]} | {r["batch"]} | {q["n"]} | {fmt(q.get("p50"))}/{fmt(q.get("p95"))}/{fmt(q.get("maximum"))} | {fmt(v.get("p95"))}/{fmt(v.get("maximum"))} | {"; ".join(r["errors"]) or "0"} |')
    lines+=['','n=100 ve1000’de örnek sayısı özellikle küçüktür; nearest-rank p95 çoğu satırda maksimumdur, güvenilir kuyruk dağılımı iddiası değildir.','',
            '## Aktif delta / 38 durum','',
            'Akışlı çağrı sonrası delta=0 olduğu açıkça raporlanır; flush edilmemiş delta kapısı owned API ile ayrıca sınanır. '
            'Her durum/run 5 zamanlı örnektir; p95=max. Tam payload/sıra/version ve güncel model oracle’ı zaman dışında çalışır.','',
            '| Motor/API/run | Gerçek delta keys | En kötü active/idle p95 | >2× durum sayısı |','|---|---:|---:|---:|']
    for r in summary['delta']:
        rs=[c['p95_ratio'] for c in r['cases']]
        lines.append(f'| {r["engine"]}/{r["api"]}/{r["run"]} | {r["stats"].get("delta_keys")} | {max(rs):.3f}× | {sum(v>2 for v in rs)}/38 |')
    lines+=['','38 durumun tamamının ham süreleri ve oranları [özet JSON](benchmarks/'+P+'-summary.json) `delta` bölümündedir.','',
            '## On tur churn: disk ve bakım','',
            'Her tur 1000 güncelleme,500 silme,500 ekleme; canlı10k sabit,100’lük atomik commitler. '
            'Bütün değişikliklerde full payload/get ve eski-yeni marker; tur sonunda bütün canlı/dead get oracle’ı geçer. '
            'Disk retained bütün çalışma alanı dosyalarıdır. Source/index live sayaçları FDB stats’tan gelir; SQLite ayrımı aynı değildir. '
            'Phase peak 10ms örneklemesi kısa sıçramayı kaçırabilir; OS lifetime peak tur içinde kümülatiftir.','',
            '| Motor/run/tur | Commit toplam ms | Bakım ms | Retained önce/sonra MB | Canlı kaynak/indeks MB | Phase private/WS MB | OS lifetime peak private/WS MB |','|---|---:|---:|---|---|---|---|']
    for r in summary['churn']:
        m=r['memory'];st=r['stats']
        lines.append(f'| {r["engine"]}/{r["run"]}/{r["round"]} | {r["commit_sum_ms"]:.1f} | {r["maintenance_ms"]:.1f} | {r["before"]["retained_bytes"]/1e6:.2f}/{r["after"]["retained_bytes"]/1e6:.2f} | {st.get("source_bytes",0)/1e6:.2f}/{st.get("disk_index_bytes",0)/1e6:.2f} | {m["sampled_phase_peak_private_bytes"]/1e6:.2f}/{m["sampled_phase_peak_working_set_bytes"]/1e6:.2f} | {m["os_lifetime_peak_private_commit_bytes"]/1e6:.2f}/{m["os_lifetime_peak_working_set_bytes"]/1e6:.2f} |')
    lines+=['','### Büyüme eski nesil dosyalarda mı, canlı segmentlerde mi?','',
            '| FDB run | Başlangıç→tur10 retained MB / oran | Manifest-live kaynak+indeks MB | Retained−live MB | Segment sayısı |','|---|---|---:|---:|---:|']
    for r in [r for r in summary['churn'] if r['engine']=='fdb' and r['round']==10]:
        base=next(x for x in summary['churn_baselines'] if x['engine']=='fdb' and x['run']==r['run'])
        initial=base['disk']['retained_bytes'];retained=r['after']['retained_bytes']
        live=r['stats']['source_bytes']+r['stats']['disk_index_bytes']
        lines.append(f'| {r["run"]} | {initial/1e6:.6f}→{retained/1e6:.6f} / {retained/initial:.3f}× | {live/1e6:.6f} | {(retained-live)/1e6:.6f} | {r["stats"].get("segments")} |')
    lines+=['','Retained büyüme ile live sayaç toplamı neredeyse eşitse eski/manifest dışı generation dosyalarının birikmesi açıklaması desteklenmez. '
            'Güncel manifestin hâlâ referans verdiği segmentlerde superseded/ölü revizyonların tutulması ve sınırlı merge seçimi aday kök nedendir; '
            'bu satır fiziksel ölü-byte miktarını veya hangi seçim koşulunun tek neden olduğunu ayrı ayrı kanıtlamaz. Önceki eski-nesil temizliği kazanımı geri alınmış sayılmaz.']
    lines+=['','## 5000 değişiklik','',
            'Önce tek atomik5000 isteği denenir; kabul reddi gizlenmez. Red sonrası100’lük parçalara bölünen yol ayrıca ölçülür; bu5000 atomik işlem değildir. '
            'Red anındaki kontrol get/version üzerinden eski durumu sınar; tam payload/marker kontrolleri başarılı fallback commit’lerinde ve son model doğrulamasında yapılır. '
            'Fallback wall süresi oracle işini içerir; salt motor throughput’u diye kullanılmaz. Burst sonrası38-case/5 örnek, delta/segment stats ve bakım ham dosyalardadır.','',
            '| Motor/API/run | Aşama | Sonuç / süre | Private / WS peak MB |','|---|---|---|---:|']
    for r in summary['burst']:
        result=r.get('result',{})
        desc=(f'commit {result.get("commit_ms",0):.1f} ms; '+('kabul' if result.get('ok') else str(result.get('error')))) if result else f'wall {r.get("elapsed_including_oracle_ms",r.get("elapsed_ms",0)):.1f} ms'
        m=result.get('memory',r.get('memory',{}))
        lines.append(f'| {r["engine"]}/{r["api"]}/{r["run"]} | {r["phase"]} | {desc} | {m.get("os_lifetime_peak_private_commit_bytes",0)/1e6:.2f}/{m.get("os_lifetime_peak_working_set_bytes",0)/1e6:.2f} |')
    lines+=['','Burst sonrasındaki bütün 38 sorgunun p50/p95/max ve örnek sayıları '
            '[CSV tablosundadır](benchmarks/churn-burst-search-20261025.csv). '
            'Eşlenmiş bir burst-öncesi sorgu ölçümü veya atomik 5000 commit sürerken ayrı okuyucu yoktur; '
            'bu veri bakım beklenmeden sonrasındaki mutlak arama süreleridir, nedensel önce/sonra oranı değildir. '
            'Aşağıdaki en yavaş durum, her koşunun 38 sorgusu arasından seçilmiştir; 5 örnekte p95=max.','',
            '| Motor/API/run | En yüksek burst-sonrası p95 sorgusu | n | p50 / p95 / max ms |',
            '|---|---|---:|---|']
    for identity in sorted({(r['engine'],r['api'],r['run']) for r in burst_cases}):
        case=max((r for r in burst_cases if (r['engine'],r['api'],r['run'])==identity),key=lambda r:r['p95_ms'])
        lines.append(f'| {"/".join(map(str,identity))} | {case["text"][:40]} /{case["offset"]}/{case["limit"]} | {case["n"]} | {case["p50_ms"]:.3f}/{case["p95_ms"]:.3f}/{case["max_ms"]:.3f} |')
    lines+=['','## Eşzamanlı okuma-yazma','',
            'Mevcut FDB API aynı Database için &mut yazma ve tek-open kilidi kullanır; snapshot reader API yoktur. '
            'Harness iki iş parçacığını aynı dış Mutex ile sıraya alır. SQLite da aynı admission wrapper’ı kullanır; '
            'bu SQLite’ın ayrı native WAL connection okuyucularının en iyi olası concurrency hızı değildir. '
            'Okuma wall süresi kilit beklemeyi içerir. Ölçülen wait dış Mutex beklemesidir; motor içindeki kilitler ayrı ayrı instrument edilmedi. '
            'Writer giriş planı kilit dışındadır; stream iterator’unun corpus okuması commit’in içindedir. '
            'Tam payload oracle kilit dışında, tutarlı küçük descriptor capture süresi ayrıca kayıtlıdır; kalan capture kilit maliyeti gerçek motor maliyeti sayılmaz.','',
            'Reader38 durumu10 tur döner. Writer en az100 tekli veya10×100 commit; güvenlik üst sınırı4000/200 commit. '
            'writer_finished bayrağı sorgu sonunda örneklendiği için kuyrukta beklerken writer biten istekler kesin ayırt edilemez. '
            '**Gate bütün okuma örneklerini kullanır**, en kötü kuyruk bu filtreyle gizlenmez. Ayrıca bayrakla ayrılmış alt örnekler ham özette korunur; bunlardan tek başına geçer kararı verilmez. '
            'Aktifcase kapsaması ve sayıları ham özet `concurrent.cases` bölümündedir. '
            '10 örnekte p95=max; gate yalnız gözlenen bu örnekler içindir. '
            'En büyük kat, boşta yaklaşık 0,0101ms olan kısa negatif sorgudan gelir: 853ms kuyruklu örnek yaklaşık 84 bin kat çıkar. '
            'Bu bütün sorguların 84 bin kat yavaşladığı anlamına gelmez; mutlak gecikme ve kilit beklemesi de birlikte verilmiştir.','',
            '| Motor/run/batch | Aktif38 kapsama | En kötü p95/idle | Max read ms | Yazma ops/s solo→reader varken | Writer wait p95/max ms | Önceki worker drain / running gözlemi |','|---|---|---:|---:|---|---|---|']
    for r in summary['concurrent']:
        writer=r['writer'];rs=[c['p95_ratio'] for c in r['cases'] if c['p95_ratio'] is not None]
        lines.append(f'| {r["engine"]}/{r["run"]}/{r["batch"]} | {r["all_cases_active"]} | {max(rs,default=0):.3f}× | {r["maximum_read_ms"]:.1f} | {writer["solo_throughput_ops_s"]:.2f}→{writer["throughput_ops_s"]:.2f} | {fmt(writer["wait"].get("p95"))}/{fmt(writer["wait"].get("maximum"))} | {writer["previous_jobs_drained"]}/{writer["running_observations"]} |')
    lines+=['','Bütün motor/run/batch/sorgu kombinasyonları için boşta ve yükte p50/p95/max, '
            'okuyucu kilit bekleme p50/p95/max, aktif örnek sayısı ve oran '
            '[tam CSV tablosundadır](benchmarks/churn-concurrent-20261025.csv). '
            'Ham özet ayrıca motor arama süresini, writer commit/bekleme dağılımlarını ve model yakalama ek yükünü içerir.','',
            'Stream commit önceki compaction_running=true job’ını bekleyip kurar; bu sayacın pozitif olması reader döngüsü boyunca tamamlanmış worker işine kanıttır. '
            'Yalnız döngüden sonraki final maintain sayısı bu kapsamı geçirmez. Owned eşzamanlı ölçüm bu tur yapılmadı; owned active delta, latency/burst/open ayrı ölçüldü.','',
            '## Process kill ve yeniden açılış','',
            'Elektrik kesintisi iddiası yoktur. Iterator ortasındaki50/100 bariyeri WAL publish öncesi deterministiktir. '
            'Flush/merge/cache için yeni geçici builder dosyası dışarıdan gözlenir, process tüm thread’leriyle suspend edilip dosya tanığı kaydedilir ve kill edilir. '
            'Bu üretim instruction hook’u değildir; phase yakalanamazsa kapsam sonuçsuzdur. Acknowledged gruplar yeni modelin tamamını korumalı; '
            'iterator publish öncesi grup eski modelde kalmalıdır, belge belge karma kabul edilmez.','',
            '| Aşama | Phase tanığı | Strict reopen + full payload/38 oracle | Eski/yeni marker |','|---|---|---|---|']
    if kills:
        for r in kills['results']:
            lines.append(f'| {r["stage"]} | {r["coverage_observed"]} | exit {r["reopen_full_payload_matrix_oracle_exit_code"]} | {r.get("pending_old_new_marker_oracle",{})} |')
        leftovers=max(sum(v for name,v in r['after_recovery_files'].items() if '.build-' in name or name.startswith('input-')) for r in kills['results'])
        lines+=['',f'Kurtarma doğruluğunun geçmesi artık dosya temizliğinin de geçtiği anlamına gelmez: '
                f'sonrasında kayıtlı input/build geçici dosyaları en fazla {leftovers/1e6:.3f} MB kalmıştır. '
                'Bu dosyalar silinmedi; temizleme davranışı ayrıca açık tutulur. Bu sayı olası manifest-dışı .fdb alanının ölçüsü değildir. '
                '10 turluk churn disk sonucu kesinti testlerinden önce ölçülmüştür; bu artıklar 2,501× büyümeyi açıklamaz.']
    else:lines.append('| Henüz tamamlanmadı | — | — | — |')
    lines+=['','## Dirty strict open ve idle bellek','',
            'Bakım sonrası500 mixed **owned** değişiklik sync edilip delta flush edilmeden kapanır. Ham dosyada/API sütununda streamed etiketi bulunan churn bölümünde de dirty durum **owned çağrısıyla zorla oluşturulmuştur**; '
            'bu stream API’nin doğal delta durumu değildir (stream başarılı çağrı sonunda delta0). Yeni process strict open replay yapar; '
            'open_ms yalnız Database::open çağrısıdır.5 saniye sonraki idle ölçümü bounded metadata oracle modelini de içerir. '
            'OS lifetime peak açılış+oracle/model işini içerir; ilk opened satırının peak’i open’a kadar olan tepedir.','',
            '| Motor/API/run | Dirty open ms | Open peak private/WS MB | 5s idle private/WS MB | Delta keys |','|---|---:|---|---|---:|']
    for r in [r for r in summary['opening'] if r['mode']=='open']:
        idle=next(x for x in summary['idle'] if x['file']==r['file']);m=r['memory'];i=idle['memory']
        lines.append(f'| {r["engine"]}/{r["api"]}/{r["run"]} | {r["open_ms"]:.2f} | {m["os_lifetime_peak_private_commit_bytes"]/1e6:.2f}/{m["os_lifetime_peak_working_set_bytes"]/1e6:.2f} | {i["private_bytes"]/1e6:.2f}/{i["working_set_bytes"]/1e6:.2f} | {idle["stats"].get("delta_keys")} |')
    lines+=['','## Yorum, sınırlar ve sonraki iş','']
    fdb_write_cost=[r for r in summary['churn'] if r['engine']=='fdb']
    if fdb_write_cost:
        most_write=max(fdb_write_cost,key=lambda r:r['commit_sum_ms']+r['maintenance_ms'])
        lines.append(f'Churn turu toplam motor süresinde en pahalı gözlem run{most_write["run"]}/tur{most_write["round"]}: '
                     f'commit toplamı {most_write["commit_sum_ms"]:.1f} ms + son bakım {most_write["maintenance_ms"]:.1f} ms. '
                     'Bu toplam seri oracle wall zamanını içermez.')
    if fdb_con:
        most_block=max(fdb_con,key=lambda r:r['maximum_read_ms'])
        lines.append(f'En kötü tekil eşzamanlı okuma run{most_block["run"]}/batch{most_block["batch"]}: '
                     f'**{most_block["maximum_read_ms"]:.1f} ms**; en kötü vaka p95/idle oranı **{fmt(worst)}×**. '
                     'Kilit bekleme ve engine-only süreler ham sonuçta ayrıdır.')
    if worst is not None and worst>2:
        recommendation='Öncelik: kullanıcı aramasını bekleten tek mutable handle/admission yolunu ve commit içindeki worker drain/flush beklemesini giderecek okunabilir snapshot tasarımını ayrı prototipte ölçmek. '
    elif disk_worst is not None and disk_worst>1.3:
        recommendation='Öncelik: churn altında manifest-live segmentlerde kalan eski revizyonları ve merge seçiminin durma koşullarını profillemek; format değiştirmeden mümkün alan geri kazanımını kanıtlamak. '
    elif worst_idle is not None and worst_idle>15_000_000:
        recommendation='Öncelik: dirty replay sonrası delta/orijinal+katlanmış metin belleğinin canlı dökümünü çıkarmak. '
    else:
        recommendation='Gözlenen kapıları aşan en pahalı workload ham tablolardan seçilmeli; eksik kapsama genel geçti kararı verilmemeli. '
    lines += ['',recommendation+'Seçim yukarıdaki tamamlanmış ölçümlere dayanır; bu tur motor optimizasyonu yapılmadı. '
            'Disk ve dirty bellek kapıları ayrıca açık tutulur; aktif delta kapısının stream yolunda bulunmaması da ürün API bulgusudur.','',
            'MB=1.000.000 byte, MiB=1.048.576 byte.15 MB≈14,305 MiB;60 MB≈57,220 MiB. '
            'Private commit fiziksel/toplam RAM değildir; working set paylaşılan image ve dosya sayfalarını içerir. '
            '10k descriptor/model ve sonuç/oracle tahsisleri ölçüm process’indedir; bütün corpus RAM’e alınmaz. '
            'Soğuk cache, elektrik kesintisi, çok okuyucu, farklı corpus, üretim Frunner entegrasyonu ölçülmedi. '
            'C ABI/DLL, opak handle, akışlı yazma ve panic yakalama yalnız sonraki aşama notudur; uygulanmadı.','',
            '[Ham tasarım/komut kimlikleri](benchmarks/'+P+'-all-design.json), '
            '[tam özet ve38-case süreleri](benchmarks/'+P+'-summary.json), '
            '[process kill kanıtları](benchmarks/churn-kill-10k-20261025-summary.json). '
            'Her mode JSONL/process/stderr ve model dosyası aynı prefix altındadır. '
            '[Final kontroller ve çıkış kodları](benchmarks/churn-finalizer-20261025.json) '
            'root/native fmt, Clippy warnings-as-errors ve iki harness testinin sonuçlarını içerir. '
            '[Bağımsız kayıt/sayısal denetim](benchmarks/churn-report-proof-20261025.json) '
            'koşu kapsamı, kuantiller, kilit örnekleri, program kimlikleri ve korunan dosyaları kontrol eder. '
            'Önceki yazma/bakım, arama regresyonları, RAM/disk ve eşzamanlı/soğuk başlıklar [OPEN-WORK](OPEN-WORK.md) içinde takip edilir.']
    REPORT.write_text('\n'.join(lines)+'\n',encoding='utf-8')
    open_work=ROOT/'docs/OPEN-WORK.md'
    marker='<!-- churn-concurrent-20261025 -->'
    existing=open_work.read_text(encoding='utf-8-sig')
    update=f'''{marker}
2026-10-25 churn/concurrent ölçümü [yeni raporda](CHURN-CONCURRENT-10K-20261025.md).
92 ana worker bölümü ve dört process-kill/reopen denemesi tamamlandı; bütün
oracle'lar geçti. Motor/format değişmedi. Solo streamed tek belge görünürlük
üst sınırı184,919ms ve stream tepe private47,079MB/WS45,978MB gözlenen kapıları
tuttu. Kirli500 owned delta ile private31,003MB/WS34,800MB:15MB kapısı kaldı.
Churn bakım sonrası350,413→876,347MB(2,501×) iki koşuda da tekrarlandı;
alanın neredeyse tamamı15 manifest-live segmentte, eski nesil temizliği değil
güncel segmentlerde superseded kayıtların tutulması araştırılmalı.
OrtakMutex ile concurrency2× kapısı kaldı; en kötü FDB read1,364s,
kısa negatif sorgu örnek-p95 oranı84464,6× (5/10 örnekte p95=max).
Öncelik okunabilir snapshot/admission ve ardından churn merge politikası;
bu tur optimizasyon yok. Stream her commit sonunda delta0 bırakır; owned
active delta ayrı ölçüldü. Atomik5000 her iki API'de reddedildi;100'lük
fallback ayrı ve atomik değil. Kill sonrası input/build artıkları da açık.
C ABI/DLL/opaque handle/stream/panic-catch sonraki aşama notudur, uygulanmadı.
Önceki yazma/son bakım, arama regresyonları, RAM/disk, gerçek-soğuk ve çok-okuyucu
başlıkları yalnız bu raporun kanıtladığı kapsamda kapanabilir.

'''
    if marker in existing:
        before,marked=existing.split(marker,1)
        _,tail=marked.split('\n\n',1)
        open_work.write_text(before+update+tail,encoding='utf-8')
    else:
        heading,rest=existing.split('\n',1)
        open_work.write_text(heading+'\n\n'+update+rest.lstrip('\n'),encoding='utf-8')
    print(json.dumps(dict(report=str(REPORT.relative_to(ROOT)),complete=complete,kill_complete=kills is not None)))


if __name__=='__main__':main()
