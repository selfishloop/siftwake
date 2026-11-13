"""Raw N60 p50/p95/max and unchanged per-pair acceptance, including bad results."""
import csv
import importlib.util
import json
from pathlib import Path

R = Path(__file__).resolve().parent.parent
L = R / 'docs/benchmarks'
P = 'hybrid-default50-20261102'
spec = importlib.util.spec_from_file_location('old_report', R / 'tools/report-estimator50.py')
E = importlib.util.module_from_spec(spec)
spec.loader.exec_module(E)


def main():
    d = json.loads((L / f'{P}-design.json').read_text(encoding='utf-8'))
    old = json.loads((L / 'estimator50-20261031-summary.json').read_text(encoding='utf-8'))
    catalog = json.loads((L / 'trwiki-cap200k-20261029-v3-catalog.json').read_text(encoding='utf-8'))
    rows, arms, samples = [], [], []
    for run in d['completed']:
        logs = [json.loads(v) for v in (L / run['log']).read_text(encoding='utf-8').splitlines()]
        load = next(x for x in logs if x['phase'] == 'snapshot_load')
        assert load['oracle'] and load['failure'] is None
        writes = [x['commit_ms'] for x in load['writes']]
        arm = dict(pair=run['pair'], role=run['role'], commits=load['commits'],
                   throughput=load['mutations'] * 1000 / load['writer_elapsed_ms'],
                   **{name: E.percentile(writes, p) for name, p in [('write_p95', .95), ('write_p99', .99), ('write_p999', .999)]},
                   write_max=max(writes), oracle=True)
        if run['pair'] == -1:
            arms.append(arm)
            continue
        phases = {}
        for phase in ['snapshot_idle_single', 'snapshot_idle_four', 'snapshot_load']:
            record = next(x for x in logs if x['phase'] == phase)
            assert record['oracle']
            by_case, excluded = [[] for _ in range(41)], [0] * 41
            for reader in record['results']:
                assert reader['oracle']
                for q in reader['records']:
                    assert q['oracle']
                    if phase == 'snapshot_load' and not q['writer_active']:
                        excluded[q['case_id']] += 1
                        continue
                    by_case[q['case_id']].append(q['engine_ms'])
            phases[phase] = by_case
            for i, values in enumerate(by_case):
                samples.append(dict(pair=run['pair'], role=run['role'], phase=phase, case_id=i,
                                    **E.stats(values), excluded_post_writer=excluded[i], values=values))
        for i in range(41):
            stats = {name: E.stats(phases[phase][i]) for name, phase in [('one', 'snapshot_idle_single'), ('idle', 'snapshot_idle_four'), ('loaded', 'snapshot_load')]}
            limit = 30 if stats['idle']['p95'] <= 30 else 2 * stats['idle']['p95']
            previous = next(x for x in old['rows'] if (x['pair'], x['role'], x['case_id']) == (run['pair'], run['role'], i))
            rows.append(dict(pair=run['pair'], role=run['role'], case_id=i, text=catalog['cases'][i]['text'],
                             **{f'{phase}_{k}': v for phase, st in stats.items() for k, v in st.items()},
                             limit=limit, passed=stats['loaded']['p95'] <= limit,
                             old_idle_p95=previous['idle_p95'], old_loaded_p95=previous['loaded_p95'], old_passed=previous['passed']))
        own = [x for x in rows if x['pair'] == run['pair'] and x['role'] == run['role']]
        arm.update(passed=sum(x['passed'] for x in own), failed_cases=[x['case_id'] for x in own if not x['passed']])
        arms.append(arm)
    complete = d['stage'] == 'completed'
    if complete:
        assert len(rows) == 492 and len(arms) == 15
    role_gates = {role: ('GEÇTİ' if all(x['passed'] for x in rows if x['role'] == role) else 'KALDI') for role in ['before', 'after', 'sqlite']} if complete else {}
    decision = role_gates.get('after', 'BEKLENİYOR')
    summary = dict(stage=d['stage'], decision=decision, rows=rows, arms=arms, role_gates=role_gates,
                   prototype_default=True, bounded_wait_implemented=False, gates_unchanged=True)
    (L / f'{P}-summary.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding='utf-8')
    (L / f'{P}-samples.json').write_text(json.dumps(samples, ensure_ascii=False), encoding='utf-8')
    if rows:
        with (L / f'{P}-cases.tsv').open('w', newline='', encoding='utf-8') as f:
            writer = csv.DictWriter(f, fieldnames=list(rows[0]), delimiter='\t')
            writer.writeheader()
            writer.writerows(rows)
    lines = ['# Try/seri varsayılanı — 2 Kasım 2026', '',
             f'**Yeni varsayılan normal 41-sorgu kapısı: {decision}.** Tamamlanan kol {len(arms)}/15. '
             'Varsayılan hibrit200 sonrası tek non-blocking try; izin yoksa aynı cursor/frontier ile çağıran threadde seri. '
             'REFERENCE_WAIT=true gizli, thread-local tanı/geri dönüş yolu olarak korunur. Exact/serial arama seçimi değişmedi; melez API opt-in kaldı. '
             'Yeni thread, bekleme/yeniden deneme döngüsü, format veya CRC değişikliği yok.', '',
             '## Kullanıcı kararı ve bilinen bedeller', '',
             '31 Ekim SIDE-COST ham sonuçları değiştirilmedi. Üç sıfır-toleranslı kapı KALDI: 4R+W peak private artmama, '
             'altılı p95 artmama, altılı/full41 CPU artmama. **SONRADAN REVİZE: kullanıcı kararıyla bilinen maliyet olarak kabul edildi, kapı eşiği değiştirilmedi.** '
             'Altılı4R zx p95 1055→1387ms (+%31), CPU504→607ms/query (+%20); tek-sorgu the zx4R CPU +%30..49, p95 475→341ms. '
             '4R+W peak private +1,5..+4,2MiB (yaklaşık33→36MiB). SQLite korunum>%100: okuyucusuz kontrol tavan değildir; ops/s esas. '
             '[Değiştirilmemiş yan maliyetler](SIDE-COST-50K-20261031.md).', '',
             '## Yöntem', '',
             'Aynı881756DA… ilk50k; corpus kaynağı değiştirilmedi, private/ okunmadı. Yalnız corpus.payload-bench ve mevcut katalog okundu. '
             'Sabit owner FDB/SQLite slotları, reset/yeniden ingest/DB kopyası yok. Model JSON kopyaları DB kopyası değildir. '
             'Same frozen FDB ikilisi: before=REFERENCE_WAIT=true, after=tanı override olmadan gerçek yeni varsayılan. '
             'SQLite=önceki frozen ESTIMATOR FTS5 trigram adaptörü ve aynı ayarlar. '
             'Sıra before/SQLite/after, after/SQLite/before, iki tekrar; önce aynı düzende üç okuyucusuz16k kontrol. '
             'Oturum/yerleşim ilerlediği için önceki rapora göre fark yalnız politika değişimine atfedilmez.', '',
             'N60: 1R60 tur; 4R15tur/aktör; loaded≥60 aktif örnek/sorgu. Writer≥16384 commit ve tüm okuyucular15 aktif tur bitene kadar sürer (emniyet262144). '
             'Nearest-rank ceil(p*N); p95 rank57/60; kırpma/yeniden örnekleme yok. Oracle her 1R/4R idle ve loaded sorguda, engine_ms dışında. '
             'engine_ms=snapshot admission+search+release. Warm/kontrolsüz OS cache, eviction yok. Profil kapalı; thread sayısı gözlemcisi öncekiyle aynı. '
             'Eşikler yuvarlanmamış değerle: idle p95≤30ms ise loaded≤30ms; aksi halde loaded≤2×idle. SQLite referans, ek FDB kapısı değil.', '',
             '## Çift kapıları ve önceki N60', '',
             '|Çift|Rol|Önceki geçti/41|Yeni geçti/kaldı|Kalan case|', '|---|---|---:|---|---|']
    for arm in arms:
        if arm['pair'] >= 0:
            previous = [x for x in old['rows'] if (x['pair'], x['role']) == (arm['pair'], arm['role'])]
            lines.append(f"|{arm['pair']}|{arm['role']}|{sum(x['passed'] for x in previous)}|{arm['passed']}/{41-arm['passed']}|{arm['failed_cases']}|")
    lines += ['', '## Yeni varsayılanda kalan sorgular', '',
              '|Çift|Case:sorgu|Idle p95 ms|Loaded p95 ms|Sınır ms|Aşım ms|Aynı çift SQLite idle/loaded ms|',
              '|---|---|---:|---:|---:|---:|---|']
    for row in rows:
        if row['role'] == 'after' and not row['passed']:
            sql = next((x for x in rows if x['pair'] == row['pair'] and x['role'] == 'sqlite' and x['case_id'] == row['case_id']), None)
            reference = f"{sql['idle_p95']:.3f}/{sql['loaded_p95']:.3f}" if sql else 'BEKLENİYOR'
            lines.append(f"|{row['pair']}|{row['case_id']}:{row['text']}|{row['idle_p95']:.3f}|{row['loaded_p95']:.3f}|{row['limit']:.3f}|{row['loaded_p95']-row['limit']:.3f}|{reference}|")
    lines += ['', 'Bu kaçırmalar gürültü veya belirli bir motor hatası olarak sınıflandırılmadı. '
              'Bu tur kök neden profili/optimizasyon yapılmadı. Yeni varsayılan kullanıcı kararıyla korunur; '
              'normal41 kapısı KALDI kaydı, önceki üç maliyet kapısına verilen kabulden ayrıdır.', '',
              'Kapı her yolun kendi idle değerine uygulanır; geçen sorgu sayısı mutlak hız sıralaması değildir. '
              'Örneğin pair0 timmy: FDB23,869→36,186ms, sınır30ms ile KALDI; SQLite31,943→35,853ms, '
              'sınır63,886ms ile GEÇTİ. Bu açıklama eşiği veya kararı değiştirmez.']
    lines += ['', '## Bütün 41 sorgu, her çift ve kol', '',
              '|Çift/rol|Case:sorgu|1R N,p50/p95/max ms|4R idle N,p50/p95/max ms|4R+W N,p50/p95/max ms|Sınır ms|Karar|', '|---|---|---|---|---|---:|---|']
    for row in rows:
        values = [f"{row[p+'_n']},{row[p+'_p50']:.3f}/{row[p+'_p95']:.3f}/{row[p+'_max']:.3f}" for p in ['one', 'idle', 'loaded']]
        lines.append(f"|{row['pair']}/{row['role']}|{row['case_id']}:{row['text']}|" + '|'.join(values) + f"|{row['limit']:.3f}|{'GEÇTİ' if row['passed'] else 'KALDI'}|")
    lines += ['', '## Yazma (aynı ölçüm bloğunun çıktısı)', '', '|Çift/rol|Commit|Ops/s|p95/p99/p99,9/max ms|', '|---|---:|---:|---|']
    for arm in arms:
        lines.append(f"|{arm['pair']}/{arm['role']}|{arm['commits']}|{arm['throughput']:.3f}|{arm['write_p95']:.3f}/{arm['write_p99']:.3f}/{arm['write_p999']:.3f}/{arm['write_max']:.3f}|")
    lines += ['', '|Çift|Before ops/s|Yeni default ops/s|SQLite ops/s|Default/before Δ%|Default/SQLite ops oranı|',
              '|---|---:|---:|---:|---:|---:|']
    for pair in range(4):
        group = {a['role']: a for a in arms if a['pair'] == pair}
        if len(group) == 3:
            b, a, sql = [group[k]['throughput'] for k in ['before', 'after', 'sqlite']]
            lines.append(f'|{pair}|{b:.3f}|{a:.3f}|{sql:.3f}|{100*(a/b-1):+.3f}|{a/sql:.3f}|')
    lines += ['', 'Bu turun yazma verisi de saklandı: önceki SIDE-COST dört çiftindeki tüm-pozitif '
              'throughput sonucu bu oturumun bütün çiftlerinde tekrarlanmadı. Kontrol bir tavan değildir; '
              'ops/s ve her çiftin kuyruk değerleri yukarıdadır. Bir çiftte432,95ms commit max dahil '
              'uçlar saklanır; bu turun kapsamı bu uçların nedenini çözmek değildir.']
    if complete:
        lines += ['', f'Rol kapıları: {role_gates}. ' + ('Performans kapısı KALDI; bu ölçüm bloğunda duruldu. Sonraki işler başlamadı.' if decision == 'KALDI' else 'Normal41 sorgu kapısı GEÇTİ. Ayrı onay gerektiren sonraki işler başlamadı.')]
    audit = L / f'{P}-final-audit.json'
    if audit.exists():
        a = json.loads(audit.read_text(encoding='utf-8'))
        lines += ['', '## Doğrulama', '', f"Default {a['tests_passed']} / diagnostic {a['diagnostic_tests_passed']} test; fmt ve iki clippy -D warnings GEÇTİ. "
                  'Yeni varsayılan fallback/frontier/cancel/snapshot/CRC ve gizli true geri dönüş bekleme/iptal/aynı sonuç testleri geçti. '
                  'Kill/reopen ve frozen eski okuyucu oracle geçti; kill tanıkları strict-open sonrasıdır, güç kesintisi/commit ortası iddiası yok. '
                  'Tam paket ayrıca append/build/yayım/cache kesilme/bozulma ve kill testlerini içerir. '
                  f"Örnekler N={a['minimum_samples']}–{a['maximum_samples']}; bütün p95 rankları max rankından küçük. "
                  'Kaynak/corpus/0.1fixture/frozen ikili hashleri korundu. CPU/RAM bu tur yeniden ölçülmedi; önceki bilinen bedeller kabul kaydıdır.', '',
                  f'[Doğrulama denetimi](benchmarks/{P}-final-audit.json) · [Kill ve eski okuyucu](benchmarks/{P}-pre-recovery.json) · [Son kurtarma](benchmarks/{P}-post-recovery.json).']
    lines += ['', f'[Ham örnekler](benchmarks/{P}-samples.json) · [TSV](benchmarks/{P}-cases.tsv) · [Özet](benchmarks/{P}-summary.json) · [Tasarım/SHA](benchmarks/{P}-design.json) · [Süreç olayları](benchmarks/{P}-lifecycle.jsonl).', '',
              'Reclaim default kapalı; signed-zero, geniş tarama/case34, yazma/bakım/disk/sorguRAM açık. '
              '100lü,B(Frunner anahtarı),C(500drain),2,6sn,WAL handle/File::open,zx iyileştirmesi,format/256KiB/ön filtre/Blok3/4/C ABI/gerçek-soğuk/uzun snapshot/100k yapılmadı. [OPEN-WORK](OPEN-WORK.md).']
    (R / 'docs/HYBRID-DEFAULT-50K-20261102.md').write_text('\n'.join(lines) + '\n', encoding='utf-8')
    print(json.dumps(dict(stage=d['stage'], completed=len(arms), decision=decision, role_gates=role_gates)))


if __name__ == '__main__':
    main()
