"""Append the measured4R stage without rewriting historical acceptance evidence."""
import json
import math
from pathlib import Path

R = Path(__file__).resolve().parent.parent
L = R / 'docs/benchmarks'
P = 'hybrid4r50-20261029-v1-b1'


def read(p):
    return json.loads(p.read_text(encoding='utf-8-sig'))


def pct(xs, fraction=.95):
    return sorted(xs)[math.ceil(fraction * len(xs)) - 1]


def main():
    design = read(L / f'{P}-design.json')
    gate = read(L / f'{P}-gate.json')
    recovery = read(L / f'{P}-recovery.json')
    assert recovery['old_reader']['oracle']
    cases = read(L / 'trwiki-cap200k-20261029-v3-catalog.json')['cases']
    report = R / 'docs/WRITE-FOLLOWUP-50K-20261029.md'
    # Keep the measured Dirty500 section verbatim; its raw result is independent.
    before = report.read_text(encoding='utf-8').split('## 2. 4 okuyucu + yazıcı')[0]
    out = ['## 2. 4 okuyucu + yazıcı — ' + ('GEÇTİ' if gate['passed'] else 'KALDI'), '']
    out += [
        'Aynı881756DA… ilk50k, akışlı API, iki dengeli kol sırası. '
        'Her kol16384 tekli update/commit;1100 uygun mevcut2–199960B donör döngüde. '
        'Kaynak gövdeleri korunur, yalnız tekil revizyon etiketi değişir;41 sorgunun sonuç-ID kümeleri sabittir. '
        'Bu, yeni içerik türleri/insert/delete karışımını temsil etmez; önceki tekli karma test ayrı kalır. '
        'Her kontrol/yüklü kol aynı commit sayısı ve8. commit sonrası aynı bakım çağrısını kullanır. '
        'FDB okuyucuları search_first_hybrid(bütçe200),N50; SQLite capped N50, WAL/FULL,FTS5 trigram. '
        'Her motorda yüklü ölçümden önce idle1R12tur ve idle4R3tur; kapı idle4R ile karşılaştırılır. '
        'Pooled idle n=24/sorgu, yüklü örnek sayısı aşağıda. Warm OS/engine cache, eviction yok.',
        'Bu dört okuyuculu idle baseline, kabul edilmiş tek okuyuculu ölçek raporunun p95’iyle '
        'aynı yük değildir; bu adımda göreli eşzamanlı yük kapısı kullanılır.',
        '',
        'Okuma süresi snapshot admission+arama+bırakma; tam payload/version/model oracle aralığın dışında, '
        'ancak aynı proseste CPU/I/O ve snapshot pin ömrüyle yüke katılır. Oracle maliyeti arama süresi diye sunulmaz. '
        'Writer throughput loop süresi bakım ve model/history bookkeeping dahil; salt fsync hızı değildir. '
        'OS private/working set motor, katalog/model, tüm okuyucular, history ve sonuç vektörleri dahil bütün benchmark prosesidir; '
        'engine-only RAM ayrıştırılmadı. Dış Mutex yok. İç melez cache/permit beklemeleri ayrıca instrument edilmedi; '
        'JSON’daki placeholder sıfırlar ölçülmüş sıfır bekleme anlamına gelmez.', '',
        '| Kapı | Sonuç | Karar |', '|---|---|---|',
    ]
    fdb_cases = [x for x in gate['cases'] if x['engine'] == 'fdb']
    bad = [x for x in fdb_cases if not x['passed']]
    ratios = {x['engine']: x for x in gate['throughput']}
    ratio_ok = ratios['fdb']['loaded_control_ratio'] >= .75 and ratios['fdb']['loaded_control_ratio'] > ratios['sqlite']['loaded_control_ratio']
    tail = gate['writer_tail']['fdb']
    tail_ok = tail['p95_ms'] <= 20 and tail['p99_ms'] <= 50 and tail['max_ms'] <= 100
    out += [
        f'| Her41 sorgu: loaded p95≤2×idle; idle<1ms ise+2ms | {len(fdb_cases)-len(bad)}/41 geçti;{len(bad)} kaldı | {"KALDI" if bad else "GEÇTİ"} |',
        f'| Yazma pooled oran≥%75 ve SQLite üstü | FDB%{100*ratios["fdb"]["loaded_control_ratio"]:.2f}; SQLite%{100*ratios["sqlite"]["loaded_control_ratio"]:.2f} | {"GEÇTİ" if ratio_ok else "KALDI"} |',
        f'| Yüklü tekli commit p95/p99/max≤20/50/100ms | {tail["p95_ms"]:.3f}/{tail["p99_ms"]:.3f}/{tail["max_ms"]:.3f}ms | {"GEÇTİ" if tail_ok else "KALDI"} |',
        '| Payload/version/commit-sınırı oracle;3 kill/reopen;eski okuyucu | Tam50k GET+41 exact ve korunan kaynaklar geçti | GEÇTİ |', '',
    ]
    out += ['| Motor | Kontrol ops/sn | 4R ops/sn | Pooled oran |', '|---|---:|---:|---:|']
    for e in ['fdb', 'sqlite']:
        x = ratios[e]
        out.append(f'| {e} | {x["rates"]["control"]:.3f} | {x["rates"]["loaded"]:.3f} | %{100*x["loaded_control_ratio"]:.2f} |')
    out += ['', '| Kol | Motor | Kontrol ops/sn | 4R ops/sn | Oran |', '|---|---|---:|---:|---:|']
    for pair in range(2):
        for e in ['fdb', 'sqlite']:
            a = ratios[e]['samples']['control'][pair]['ops_s']
            b = ratios[e]['samples']['loaded'][pair]['ops_s']
            out.append(f'| pair{pair} | {e} | {a:.3f} | {b:.3f} | %{100*b/a:.2f} |')
    out += ['', '| Case/sorgu | FDB idle p95 | FDB 4R+yazıcı p95 | Kapı ms | Δms /oran | FDB n yüklü | Karar | SQLite idle /yüklü p95 |', '|---|---:|---:|---:|---|---:|---|---|']
    lookup = {(x['engine'], x['case_id']): x for x in gate['cases']}
    for x in fdb_cases:
        i = x['case_id']; s = lookup['sqlite', i]
        delta = x['loaded_p95_ms'] - x['idle_p95_ms']
        ratio = x['loaded_p95_ms'] / x['idle_p95_ms']
        out.append(f'| {i}: {cases[i]["text"]} | {x["idle_p95_ms"]:.3f} | {x["loaded_p95_ms"]:.3f} | {x["limit_ms"]:.3f} | {delta:+.3f} /{ratio:.2f}× | {x["loaded_n"]} | {"GEÇTİ" if x["passed"] else "KALDI"} | {s["idle_p95_ms"]:.3f}/{s["loaded_p95_ms"]:.3f} |')
    out += ['', '### Commit uçları, RAM ve bekleme', '', '| Kol | Motor | Mod | Commit p95/p99/max ms | >100ms adet | OS peak private/WS MiB |', '|---|---|---|---|---:|---|']
    waits = []; examples = []; maintenance = []; seeds = []; tails = []
    for run in design['completed']:
        rs = [json.loads(v) for v in (L / run['log']).read_text(encoding='utf-8').splitlines() if v]
        if run['mode'] == 'scale-seed':
            seeds.append(dict(engine=run['engine'], label=run['label'], process_wall_s=run['wall_s']))
            continue
        x = next(v for v in rs if v['phase'] == 'snapshot_load')
        a = [z['commit_ms'] for z in x['writes']]; mem = x['memory']; mode = 'kontrol' if x['control'] else '4R'
        out.append(f'| {run["label"]} | {run["engine"]} | {mode} | {pct(a):.3f}/{pct(a,.99):.3f}/{max(a):.3f} | {sum(v>100 for v in a)} | {mem["os_lifetime_peak_private_commit_bytes"]/1048576:.3f}/{mem["os_lifetime_peak_working_set_bytes"]/1048576:.3f} |')
        if run['engine'] == 'fdb':
            tails.extend(dict(label=run['label'], **z) for z in x['writes'] if z['commit_ms'] > 100)
        maintenance.append(dict(engine=run['engine'], label=run['label'], **x['maintenance']))
        if run['engine'] == 'fdb' and not x['control']:
            rr = [z for r in x['results'] for z in r['records'] if z['writer_active']]
            waits.extend(rr)
            for i in [0, 3, 39, 40]:
                a = sorted([z for z in rr if z['case_id'] == i], key=lambda z: z['engine_ms'])
                z = a[math.ceil(.95*len(a))-1]
                examples.append(dict(label=run['label'], case_id=i, sequence=z['sequence'], engine_ms=z['engine_ms'], search_ms=z['search_ms'], admission_ms=z['admission']['elapsed_ns']/1e6, release_ms=z['release_ms']))
    out += ['', 'SQLite için max kapısı uygulanmadı. Kontroldeki100ms üstü FDB uçları da tabloda saklanmadan gösterilir; '
        'bunlar16384 uzun update serisidir, önceki dört tipli512/tip kısa testin kabulünü geriye dönük değiştirmez. '
        'Ancak uzun yazma uçları açık kalır; bilinen74,840ms örnekle aynı neden oldukları kanıtlanmadı.', '']
    out += ['| FDB kol | Sequence | Commit ms | Kol içi bitiş ms |', '|---|---:|---:|---:|']
    for x in tails:
        out.append(f'| {x["label"]} | {x["sequence"]} | {x["commit_ms"]:.4f} | {x["elapsed_ms"]:.4f} |')
    out += ['', 'Bu profil kapalı uçların WAL/worker/yayın/scheduler ayrımı kaydedilmedi; nedenleri açık.', '']
    out += [f'Yüklü FDB toplam{len(waits)} okuma: admission contended={sum(z["admission"]["contended"] for z in waits)}, '
        f'admission max={max(z["admission"]["elapsed_ns"] for z in waits)/1e6:.4f}ms, '
        f'ölçülen admission wait max={max(z["admission"]["wait_ns"] for z in waits)/1e6:.4f}ms; '
        f'bırakma dış süresi max={max(z["release_ms"] for z in waits):.4f}ms. '
        'Arama içindeki writer/cache beklemesi ayrı ölçülmediğinden “hiç yazıcı beklemesi yok” iddiası kurulmaz. '
        'Aşağıdaki satırlar her kol/sorgunun p95’ine denk gelen AYNI çağrının ayrımıdır; ayrı yüzdelikler toplanmaz.', '',
        '| Kol/case | Sequence | Toplam ms | Arama ms | Admission ms | Bırakma ms |', '|---|---:|---:|---:|---:|---:|']
    for x in examples:
        out.append(f'| {x["label"]}/{x["case_id"]} | {x["sequence"]} | {x["engine_ms"]:.4f} | {x["search_ms"]:.4f} | {x["admission_ms"]:.4f} | {x["release_ms"]:.4f} |')
    out += ['', 'Ölçülen artış bu örneklerde ağırlıkla arama çağrısındadır; admission kapısında uzun bloklanma '
        'baskın neden değildir. Arama içi payload/posting/CPU/scheduler payları bu profil kapalı koşuda ayrılmadı; '
        'kesin alt neden veya çözüm iddiası yok. Ayrı dar, sequence eşlemeli arama profili sonraki onay için öneridir.', '',
        '### Kapsam, kurtarma ve süreç kaydı', '',
        '8. commit bakım çağrısının job sayaçları ham tasarımda yer alır. Bu koşuda varsayılan reclamation kapalı; '
        'bağımsız revision-rewrite stresini veya uzun snapshot sınamasını tamamlandı diye sunmuyoruz. '
        'Okuyucular her snapshot’ın sequence’ine göre tam hit payload/version ve son commit grubu oracle’ını doğruladı; '
        'eski revision etiketi görünmedi. Her kol sonu50k GET oracle geçti. Ayrı kurtarma kontrolünde3 kez strict-open '
        'tanığından sonra process kill, ardından reopen ve dondurulmuş eski okuyucu50k GET+41 exact oracle geçti. '
        'Bu kill’ler append ortası veya elektrik kesintisi deneyi değildir.', '',
        '**Doğrulama aracında ilk deneme başarısızdı:** üç kill/reopen ve eski okuyucu oracle geçti, '
        'ardından klasördeki bütün .fdb/.ready envanterinin aynı kalması assert’i kaldı. '
        'İlk başlangıç hash listesi dosyaya kaydedilmediği için değişen dosya adı/byte farkı geriye dönük '
        'kanıtlanamadı. Strict-open eski/manifest dışı dosyaları toplayabilir; bu olası açıklamadır, '
        'ilk farkın kanıtlanmış kök nedeni değildir. Motor değişmedi. Araç mevcut CRC kontrollü MAN03 '
        'referanslarını ve bütün dosya hash listesini her aşamada saklayacak şekilde düzeltildi. '
        'Aynı DB’de ayrı üç kill/reopen + eski okuyucu tekrarında canlı kaynaklar/current manifest '
        'hash’leri ve bütün kalan envanter aynı kaldı. İlk başarısız araç koşusu silinmedi; '
        'başlangıç envanterinin eksikliği açık gözlem sınırı olarak kaldı. '
        f'[İlk başarısız assert](benchmarks/{P}-recovery-inventory-assert-failure.json) · '
        f'[Ayrı tekrar](benchmarks/{P}-recheck-recovery.json) · '
        f'[Aşama bazlı canlı/tam hash envanteri](benchmarks/{P}-recheck-recovery-inventory.json).', '',
        'Wrapper her çocuğun PID/start/exit kodunu fsync’li lifecycle’a yazar; ayrı observer process handle ve oturum/OS '
        'olaylarını izler. WMI start/stop aboneliği erişim hatasıyla kullanılamadı; kayıt bunu açıkça taşır. '
        'Bu turdaki16 ölçüm/seed çocuğu exit0; beklenmedik ölçüm süreci kapanması gözlenmedi. '
        'İlk recovery Python driver’ı yukarıdaki assert nedeniyle exit1 oldu; çocuğun açıklanamayan çökmesi değildir. '
        'İlk recovery ve tekrarın üçer nonzero çocuk çıkışı wrapper’ın tanıklı/amaçlı kill’idir. '
        'Tüm uygulama/job birlikte kapanırsa bu izleyici kesin fail eden aktörü '
        'her durumda saptayamaz; önceki yarım oturumların nedeni geriye dönük kanıtlanmadı.', '',
        f'[Ham kapı ve41×2 motor matrisi](benchmarks/{P}-gate.json) · '
        f'[Tasarım/tüm kol logları/RAM](benchmarks/{P}-design.json) · '
        f'[Kill/reopen/eski okuyucu](benchmarks/{P}-recovery.json) · '
        f'[Lifecycle](benchmarks/{P}-lifecycle.jsonl) · '
        f'[Observer](benchmarks/{P}-process-events.jsonl).', '',
        '## Güncel sıralı kapılar', '',
        '| İş | Karar |', '|---|---|',
        '| Kısa tekli test yeni p95/p99/max20/50/100 | GEÇTİ;74,84ms açıklanmamış uç açık |',
        '| 1: Dirty500 strict≤1000ms, idle≤15MiB | GEÇTİ |',
        f'| 2: 4R+tekli yazıcı | {"GEÇTİ" if gate["passed"] else "KALDI"} |',
        '| 2: 4R+100lü yazıcı | BAŞLAMADI |',
        '| 3: 100lü profil /500lü drain önerisi | BAŞLAMADI |',
        '| 4: Gerçek Frunner anahtarı | BAŞLAMADI |', '',
    ]
    if not gate['passed']:
        out += ['**Durma kuralı uygulandı:** 4R okuma/tekli yük kapısı kapanmadan100lü eşzamanlı kol, '
            '100lü aşama profili ve Frunner anahtarı testi başlatılmadı. Optimizasyon yapılmadı. '
            'snapshot.mask35ms düzeltmesi ayrı onay bekliyor; bu sonuç onu uygulama izni değildir.', '']
    out += ['Arama regresyonları (case34/the zx parçalı yerleşim), signed-zero, yazma/bakım kuyrukları, '
        'RAM/disk ve eşzamanlı yük açık iş listesinde korunuyor. '
        '100k/format/256KiB/ön filtre/Blok3/4/C ABI/gerçek-soğuk/uzun snapshot/dış entegrasyon yapılmadı.', '']
    out += ['## Kod ve test sınırı', '',
        'cargo fmt --check ve cargo clippy --all-targets -- -D warnings geçti. '
        'cargo test --all-targets:243 geçti,15 mevcut ignored; normal paralel çalıştırma. '
        'No-default/all-features lib check ve izole SQLite fmt/clippy/test geçti. '
        'Melez adaptörün JSON etiketi düzeltildikten sonra fmt/clippy ve donmuş benchmark ikilileri yeniden oluşturuldu; '
        'zamanlı koşular sırasında build/test çalıştırılmadı. '
        '[8 komutun exit/log kanıtı](benchmarks/snapshot-10k-20261026-validation-hybrid4r50-20261029-v1.json).', '',
        'Önceki WRITE-PROFILE kaynak hash’lerine göre70 engine dosyasından69’u aynı; '
        'src/format.rs içindeki mevcut diagnostic feature’a bağlı iki satırlık WAL replay span’i '
        'çıkarılınca o dosyanın hash’i de öncekiyle aynı. Normal motor davranışı/kalıcı biçim/limit değişmedi. '
        'Kalan değişiklikler benchmark adaptörü, test aracı ve raporlardır. '
        '[Son kapsam/kanıt kontrolü](benchmarks/write-followup50-20261029-final-audit.json).', '']
    report.write_text(before + '\n'.join(out), encoding='utf-8')
    (L / f'{P}-report-detail.json').write_text(json.dumps(dict(admission_samples=len(waits), selected_p95_calls=examples, maintenance=maintenance, seed_processes=seeds, fdb_over100ms=tails), ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps(dict(report=str(report), passed=gate['passed'], failed_queries=len(bad))))


if __name__ == '__main__':
    main()
