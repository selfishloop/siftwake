"""Same gates, better order-statistic sample size. Supports partial reports."""
import csv
import json
import math
from pathlib import Path

R = Path(__file__).resolve().parent.parent
L = R / 'docs/benchmarks'
P = 'estimator50-20261031'


def percentile(values, p):
    return sorted(values)[math.ceil(p * len(values)) - 1]


def stats(values):
    assert len(values) >= 60
    return dict(n=len(values), p50=percentile(values, .50), p95=percentile(values, .95),
                max=max(values), p95_rank=math.ceil(.95 * len(values)),
                p95_equals_max=percentile(values, .95) == max(values))


def main():
    d = json.loads((L / f'{P}-design.json').read_text(encoding='utf-8'))
    old = json.loads((L / 'hybrid-serial50-20261030-summary.json').read_text(encoding='utf-8'))
    cases = json.loads((L / 'trwiki-cap200k-20261029-v3-catalog.json').read_text(encoding='utf-8'))['cases']
    rows, arms, samples = [], [], []
    for run in d['completed']:
        logs = [json.loads(v) for v in (L / run['log']).read_text(encoding='utf-8').splitlines()]
        load = next(x for x in logs if x['phase'] == 'snapshot_load')
        assert load['oracle'] and load['failure'] is None
        writes = [x['commit_ms'] for x in load['writes']]
        arm = dict(pair=run['pair'], role=run['role'], commits=load['commits'],
                   throughput=load['mutations'] * 1000 / load['writer_elapsed_ms'],
                   write_p95=percentile(writes, .95), write_p99=percentile(writes, .99),
                   write_p999=percentile(writes, .999), write_max=max(writes))
        if run['pair'] == -1:
            arms.append(arm)
            continue
        phase_samples = {}
        for phase in ['snapshot_idle_single', 'snapshot_idle_four', 'snapshot_load']:
            records = next(x for x in logs if x['phase'] == phase)['results']
            by_case = [[] for _ in range(41)]
            excluded = [0] * 41
            for reader in records:
                for q in reader['records']:
                    assert q['oracle']
                    if phase == 'snapshot_load' and not q['writer_active']:
                        excluded[q['case_id']] += 1
                        continue
                    by_case[q['case_id']].append(q['engine_ms'])
            phase_samples[phase] = by_case
            for i, values in enumerate(by_case):
                st = stats(values)
                samples.append(dict(pair=run['pair'], role=run['role'], phase=phase,
                                    case_id=i, **st, excluded_post_writer=excluded[i],
                                    values=values))
        for i in range(41):
            one = stats(phase_samples['snapshot_idle_single'][i])
            idle = stats(phase_samples['snapshot_idle_four'][i])
            loaded = stats(phase_samples['snapshot_load'][i])
            limit = 30 if idle['p95'] <= 30 else 2 * idle['p95']
            previous = next(x for x in old['rows'] if x['pair'] == run['pair'] and x['role'] == run['role'] and x['case_id'] == i)
            rows.append(dict(pair=run['pair'], role=run['role'], case_id=i, text=cases[i]['text'],
                             **{f'one_{k}':v for k,v in one.items()},
                             **{f'idle_{k}':v for k,v in idle.items()},
                             **{f'loaded_{k}':v for k,v in loaded.items()},
                             limit=limit, passed=loaded['p95'] <= limit,
                             old_idle_n=previous['idle_n'], old_idle_p95=previous['idle_p95'],
                             old_one_n=previous['one_n'], old_one_p95=previous['one_p95'], old_limit=previous['limit'],
                             old_loaded_n=previous['loaded_n'], old_loaded_p95=previous['loaded_p95'],
                             old_passed=previous['passed']))
        own = [x for x in rows if x['pair'] == run['pair'] and x['role'] == run['role']]
        arm.update(passed=sum(x['passed'] for x in own), failed_cases=[x['case_id'] for x in own if not x['passed']],
                   old_passed=sum(x['old_passed'] for x in own))
        arms.append(arm)
    complete = d['stage'] == 'completed'
    if complete:
        assert len(rows) == 12 * 41
    role_gates = {role: ('GEÇTİ' if all(x['passed'] for x in rows if x['role'] == role) else 'KALDI')
                  for role in ['before', 'after', 'sqlite']} if complete else {}
    # SQLite is a reference result, not a newly invented FDB acceptance gate.
    decision = role_gates.get('after', 'BEKLENİYOR')
    summary = dict(stage=d['stage'], decision=decision, rows=rows, arms=arms,
                   sqlite_all_41=all(x['passed'] for x in rows if x['role']=='sqlite') if complete else None,
                   engine_changed=False, prototype_default=False, bounded_wait_implemented=False,
                   role_gates=role_gates)
    (L / f'{P}-summary.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding='utf-8')
    (L / f'{P}-samples.json').write_text(json.dumps(samples, ensure_ascii=False), encoding='utf-8')
    if rows:
        with (L / f'{P}-cases.tsv').open('w', newline='', encoding='utf-8') as f:
            w = csv.DictWriter(f, fieldnames=list(rows[0]), delimiter='\t')
            w.writeheader()
            w.writerows(rows)
    lines = ['# P95 örnek sayısı düzeltmesi — 31 Ekim 2026', '',
             f'**Aday A kapısı: {decision}.** Tamamlanan kollar: {len(d["completed"])}/15. ' +
             ('Ölçüm tamamlandı. ' if complete else 'Ölçüm sürüyor; tablo nihai karar değildir. ') + 'Motor değişikliği yok; '
             'REFERENCE_WAIT=true varsayılan, anında try/seri prototipi diagnostics opt-in. '
             'Kabul edilmiş maske ve capped delta korunuyor.', '',
             '## Yöntem ve sınırlar', '',
             'Eşik aynı: idle p95 ≤30 ms ise loaded p95 ≤30 ms; diğerlerinde loaded '
             'p95 ≤2×idle. Nearest-rank ceil(p×N) aynı kaldı. N=12 ile p95 rank12=max '
             'iken N=60 ile rank57, max rank60. p50/p95/max ve ham örnekler birlikte '
             'verilir. Eşit süreler varsa sayısal p95=max olabilir; rank yine ayrıdır. '
             'Örnek kırpma, bootstrap veya iyi koşu seçimi yok.', '',
             'Gecikme ölçütü eskiyle aynı engine_ms: snapshot admission + search + '
             'release; oracle süresi hariçtir ve ham logda ayrıca bulunur. Kapı '
             'yuvarlanmamış değerlerle verilir; tablodaki süreler üç ondalıktır.', '',
             'Her kol 1R idle60 tur, 4R idle15 tur/aktör =60 örnek/sorgu. Yüklü '
             'yalnız writer_active=true örnekleri kullanır. Yazıcı en az16.384 commit '
             'yapar ve dört okuyucunun her biri15 tam tur bitirmeden durmaz; en az60 '
             'aktif örnek sağlanır. 262.144 commit emniyet sınırıdır, kalite kapısı '
             'değildir; örnek yetmezse ölçüm eksik sayılır. Eski SQLite kollarında '
             '15–17 aktif örnek vardı. Daha fazla örnek gerçek çalışma süresi gerektirir; '
             'minimum60 seçildi,100 alınmadı. Oracle her sorguda korunur.', '',
             'Aynı 881756DA… corpus ilk50k,41 ana sorgu,N50/hibrit200; before/SQLite/after '
             've ters sıra dört dengeli çift. Aynı frozen FDB ikilisinde politika scope '
             'ile seçilir; SQLite aynı FTS5 trigram ayarlarıyla yeniden derlenen ölçüm '
             'adaptörüdür. Yalnız ortak benchmark örnekleme/izleme kodu değişti. '
             'Sabit owner slotları reset edilmez, corpus/private/ değişmez. Yazılar '
             'fiziksel yerleşim ve sequence ilerletir. Warm OS cache, eviction yok. '
             'Derleme/test timed kollarla paralel yapılmaz. Eski/yeni farkı yalnız '
             'tahminciye nedensel olarak atfedilemez: oturum/yerleşim ve daha uzun '
             'writer penceresi de farklıdır.', '',
             '## Eski N12 ve yeni tahminci yan yana', '',
             '|Çift|Rol|Eski geçti/41|Yeni geçti/kaldı|Kalan case|Aktif commit|',
             '|---|---|---:|---|---|---:|']
    for a in arms:
        if a['pair'] != -1:
            lines.append(f"|{a['pair']}|{a['role']}|{a['old_passed']}|{a['passed']}/{41-a['passed']}|{a['failed_cases']}|{a['commits']}|")
    lines += ['', '[Eski N12 raporu ve ham kanıt](HYBRID-SERIAL-ACCEPTANCE-50K-20261030.md). '
              'Yeni kalite koşulu (yeterli örnek / p95 rankı max değildir) ile motor '
              'A kapısı ayrı kararlardır. Daha iyi örnekleme bütün sorguların kapıyı '
              'geçeceği veya farkın tek nedeninin örnek sayısı olduğu anlamına gelmez.', '',
              '## Yeni kapıda kalanlar: eski/yeni p95 yan yana', '',
              '|Çift/rol|Case|Eski idle / loaded p95|Yeni idle / loaded p95|Yeni loaded N / max|Sınır|',
              '|---|---|---|---|---|---:|']
    for x in rows:
        if not x['passed']:
            lines.append(f"|{x['pair']}/{x['role']}|{x['text']}|{x['old_idle_p95']:.3f}/{x['old_loaded_p95']:.3f}|{x['idle_p95']:.3f}/{x['loaded_p95']:.3f}|{x['loaded_n']} / {x['loaded_max']:.3f}|{x['limit']:.3f}|")
    lines += ['', '## Her sorgu p50 / p95 / max (ms)', '',
              '|Çift/rol|Case/sorgu|1R idle N: p50/p95/max|4R idle N: p50/p95/max|Loaded N: p50/p95/max|Sınır|Karar|',
              '|---|---|---|---|---|---:|---|']
    for x in rows:
        cols = [f"{x[p+'_n']}: {x[p+'_p50']:.3f}/{x[p+'_p95']:.3f}/{x[p+'_max']:.3f}" for p in ['one','idle','loaded']]
        lines.append(f"|{x['pair']}/{x['role']}|{x['case_id']}:{x['text']}|" + '|'.join(cols) + f"|{x['limit']:.3f}|{'GEÇTİ' if x['passed'] else 'KALDI'}|")
    lines += ['', '## Yazma ve süre sınırı', '',
              '|Çift/rol|Commit|İşlem/sn|p95/p99/p99,9/max ms|', '|---|---:|---:|---|']
    for a in arms:
        lines.append(f"|{a['pair']}/{a['role']}|{a['commits']}|{a['throughput']:.3f}|{a['write_p95']:.3f}/{a['write_p99']:.3f}/{a['write_p999']:.3f}/{a['write_max']:.3f}|")
    if complete:
        lines += ['', f'Rol kapıları: kontrol **{role_gates["before"]}**, aday **{role_gates["after"]}**, SQLite referansı **{role_gates["sqlite"]}**. SQLite sonucu yeni bir FDB kapısı olarak eklenmedi.',
                  '**SQLite bu yeni tahminciyle de 41/41 olmadı.**' if not summary['sqlite_all_41'] else '**SQLite dört çiftin tamamında 41/41 oldu.**',
                  'Bir sorgunun neden kaldığı yalnız bu kapı tablosundan motor hatası '
                  'veya ölçüm gürültüsü olarak sınıflandırılmadı. Kapı değişmedi. '
                  'Sınırlı condvar bekleme prototipi uygulanmadı; önce bu sonuçlar '
                  'raporlanır ve ayrı kullanıcı onayı gerekir. Sonraki kapsam dışı '
                  'işlere geçilmedi.']
    audit_path = L / f'{P}-final-audit.json'
    evidence_path = L / f'{P}-wait-evidence.json'
    if complete and audit_path.exists():
        audit = json.loads(audit_path.read_text())
        lines += ['', '## Ölçüm kalitesi ve doğrulama kapıları', '',
                  '|Koşul|Sonuç|', '|---|---|',
                  f'|Her sorgu/faz yeterli gerçek örnek|GEÇTİ: {audit["phase_rows"]} faz satırında N={audit["minimum_samples"]}–{audit["maximum_samples"]}|',
                  '|p95 maksimum örnekten ayrı|GEÇTİ: bütün ranklar max ranktan küçük; sayısal p95=max olan satır 0|',
                  '|Latans eşikleri / engine kaynakları / frozen ikililer|DEĞİŞMEDİ; SHA denetimi geçti|',
                  '|Oracle / kaynak / 0.1 fixture|GEÇTİ; hash korundu|',
                  f'|Tam test / fmt / clippy|{audit["tests_passed"]} test geçti ({audit["tests_ignored"]} ignored); fmt ve all-targets clippy -D warnings geçti|',
                  '|Process kill/reopen / eski okuyucu|3 kill + strict reopen + frozen eski okuyucu oracle GEÇTİ|',
                  '|Normal ölçüm süreçleri|15/15 child normal exit 0; açıklanmayan kesilme yok|', '',
                  'Kill tanığı strict-open sonrasıdır; bu üç kill commit ortası/güç '
                  'kesintisi kanıtı değildir. Test paketi append/build/yayım/cache '
                  'kesilme/bozulma ve kill senaryolarını da içerir. Kaynak/WAL/manifest/'
                  'payload veya256KiB sınırı bu turda değişmedi. Model/oracle/history '
                  'içeren benchmark belleği motor-only RAM diye sunulmadı. Bu tur '
                  'parent+child sorgu CPU veya yeni D ingest/idle/loaded RAM ölçümü '
                  'yapılmadı; önceki CPU/RAM artışları kapanmış sayılmadı.', '',
                  f'[SHA/örnek/kapı/test denetimi](benchmarks/{P}-final-audit.json) · '
                  f'[Test logu](benchmarks/{P}-tests.log) · '
                  f'[Fmt](benchmarks/{P}-fmt.log) · [Clippy](benchmarks/{P}-clippy.log) · '
                  f'[Kill ve eski okuyucu](benchmarks/{P}-recovery.json).']
    if complete and evidence_path.exists():
        evidence = json.loads(evidence_path.read_text())
        lines += ['', '## Tek ara-yol değerlendirmesi: en fazla 2 ms istenen condvar beklemesi', '',
                  '**Uygulanmadı; runtime deneyi yok.** Bu değerlendirme yukarıdaki '
                  'yeni tahminci matrisi tamamlandıktan sonra, yalnız mevcut 30 Ekim '
                  'profillerinin çevrimdışı sayımıdır. Yeni N60 ölçümünde permit '
                  'sayaçları açık değildi. Aşağıdaki oranlar başka bir izin politikasının '
                  'kaynak takvimini veya kazanımını tahmin eden simülasyon değildir.', '',
                  '|Case|Eski çekişmeli request|Eski toplam acquire ≤2 ms|Pay|',
                  '|---|---:|---:|---:|']
        for x in evidence['rows']:
            if x['old_contended']:
                lines.append(f"|{x['case_id']}:{cases[x['case_id']]['text']}|{x['old_contended']}|{x['old_acquired_within_2ms']}|%{x['old_contended_within_2ms_percent']:.2f}|")
        lines += ['',
                  'Üreticisi/timmy/zx için yalnız yaklaşık %5–8 oranı 2 ms içinde '
                  'izin almış. Tarihsel beklemelerin çoğu 2 msden uzun; yeni kaynak '
                  'takviminde kaçının fallback olacağı ölçülmedi. CPU/yazma bedelinin '
                  'toparlanacağına güçlü kanıt yok. Ayrıca önceki '
                  'isimli profilinde 158 aday requestte handoff=0/fallback=0: yeni '
                  'isimli sınır aşımının izin beklemesiyle doğrudan çözüleceği '
                  'kanıtlanmadı. Bunlar tarihsel yol gözlemleridir; yeni koşunun '
                  'isimli requestleri için sayaç iddiası değildir.', '',
                  'Onaylanırsa değerlendirilecek **tek** dar tasarım: ilk mutex '
                  'try başarısızsa hemen aynı frontier ile seri; mutex alınmış ama '
                  'free=0 ise mevcut condvar üzerinde yalnız bir kez 2 ms '
                  'wait_timeout; dönüşte token kontrolü ve mevcut min-grant/RAII '
                  'muhasebesiyle izin varsa paralel, yoksa aynı seri frontier. Loop, '
                  'spin/sleep veya yeni worker yok. Mevcut bütçe, CRC, sequence ve '
                  'görünürlük değişmez. Bu yalnız tasarımdır, code patch değildir.', '',
                  '2 ms, istenen condvar timeout değeridir; mutexin yeniden '
                  'alınması ve OS zamanlaması nedeniyle mutlak wall üst sınırı '
                  'garanti etmez. Beklenen sayısal kazanç ölçülmedi. Bugünkü A '
                  'kapısı kaldığı için yeni runtime deneyi veya optimizasyon '
                  'başlatılmadı; uygulama ayrı kullanıcı onayı gerektirir.', '',
                  f'[Tarihsel sayaçlar ve kaynak SHA](benchmarks/{P}-wait-evidence.json) · '
                  '[Çevrimdışı hesap aracı](../tools/assess-estimator-wait50.py).']
    lines += ['', f'[Ham örnekler](benchmarks/{P}-samples.json) · [Tam eski/yeni TSV](benchmarks/{P}-cases.tsv) · '
              f'[Özet](benchmarks/{P}-summary.json) · [SHA/kol/örnek sayımı](benchmarks/{P}-design.json) · '
              f'[Süreç olayları](benchmarks/{P}-lifecycle.jsonl).', '',
              '4R+100lü,Frunner anahtarı,500 drain,2,6sn,WAL handle,format,256KiB,'
              'ön filtre,Blok3/4,C ABI,gerçek-soğuk,uzun snapshot ve100k yok. '
              'Yazma/bakım,arama kabulü/RAM/disk ve geniş tarama açık işleri '
              '[OPEN-WORK](OPEN-WORK.md) üzerinde korunur.']
    (R / 'docs/ESTIMATOR-50K-20261031.md').write_text('\n'.join(lines) + '\n', encoding='utf-8')
    print(json.dumps(dict(stage=d['stage'], decision=decision, arms=len(arms), sqlite_all_41=summary['sqlite_all_41'])))


if __name__ == '__main__':
    assert percentile(list(range(1, 13)), .95) == 12
    assert percentile(list(range(1, 61)), .95) == 57
    main()
