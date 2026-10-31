"""Report exact-total costs and strict-open evidence; never invent capped results."""
import hashlib
import json
import re
import statistics
from pathlib import Path

ROOT=Path(__file__).resolve().parent.parent
LOGS=ROOT/'docs/benchmarks'
P='block2-query-open-50k-20261028-v1'


def load(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))


def main():
    data=load(LOGS/'block2-query-open-50k-20261028-analysis-v1.json')
    design=load(LOGS/f'{P}-design.json')
    assert design['protected_unchanged'] and design['fdb_sources_unchanged']
    assert len(design['completed'])==13
    compat=load(LOGS/'block2-query-open-50k-20261028-compat-v1-old-reader-utf8-v2.json')
    assert compat['oracle'] and compat['full_payload_version_gets']==50000
    validation=load(LOGS/'snapshot-10k-20261026-validation-query-open-50k-20261028-v1.json')
    assert all(row['exit_code']==0 for row in validation)
    test=(LOGS/'snapshot-10k-20261026-validation-query-open-50k-20261028-v1-test.log').read_text(encoding='utf-8-sig')
    counts=[tuple(map(int,m)) for m in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',test)]
    passed=sum(x[0] for x in counts)
    diagnostic=(LOGS/'block2-query-open-50k-20261028-v2-diagnostic-lib-tests.log').read_text(encoding='utf-8-sig')
    assert '139 passed; 0 failed; 12 ignored' in diagnostic
    rows=data['queries']
    profiles=data['strict_open_profiles']
    times=lambda k:[r['counters'].get(k,{}).get('elapsed_ns',0)/1e6 for r in profiles]
    mean=lambda k:statistics.mean(times(k))
    top=mean('open.total')
    probe=load(LOGS/'block2-query-open-50k-20261028-boundary-probe-v1.json')['samples']
    out=[
        '# Exact-total maliyeti ve açılış profili — 50k, 28 Ekim 2026','',
        '**Ölçüm tamamlandı; yeni arama/açılış API\'leri henüz uygulanmadı.**',
        'Kullanıcının son kararıyla önceki ≤5× ölçek kapısı kaldırıldı. Yeni orta/nadir',
        'sorgu hedefi p95≤150ms@50k, ≤300ms@100k; yaygın sorgu≤30ms. zx/the zx',
        'geniş taraması bilinen sınırdır. Bu rapor beş orta sıklıktaki sorgunun',
        'exact yolunu ve mevcut strict-open maliyetini ayırır. API tasarımı',
        '[ayrı belgede](CAPPED-SEARCH-FAST-OPEN-DESIGN-50K-20261028.md).','',
        '| Kapı / iş | Kanıt | Durum |','|---|---|---|',
        '| Beş hedef exact-total sorgu ≤150ms | Aşağıdaki20 örnek/case pooled p95; beşi de150ms üstü | KALDI, baseline |',
        '| Yaygın≤30ms | Önceki50k raporunda max7,392ms; bu tur hedef5 sorgu seçildi | Önceki GEÇTİ korunur, yeniden ölçülmedi |',
        '| Capped N=50 ≤150ms | Henüz uygulanmadı; sözleşme/kapı seçimi değerlendirmede | ÖLÇÜLMEDİ |',
        '| Hızlı oturum <100ms | Tasarım kullanıcı tarafından onaylandı; capped kapısından sonra | UYGULANMADI / ÖLÇÜLMEDİ |',
        '| Kaynak/0.1/kalıcı biçim | FDB kaynakları ve protected dosyalar byte-eşit; yazıcı mantığı değişmedi | GEÇTİ |',
        f'| Doğruluk/uyumluluk | Her örnekte full page oracle; normal paket{passed} test, diagnostic139; eski okuyucu50k/38-case | GEÇTİ |',
        '| Sonraki50k yazma, 2c, 2b, 100k | API/kapı kararı beklenir;100k gerçek corpus\'u kullanıcı sağlayacak | BAŞLANMADI |','',
        '## Aynı veri ve ölçüm sözleşmesi','',
        '- Mevcut sabit FDB/SQLite50k DB\'leri kullanıldı; sıfırlama/corpus üretme/',
        '  indirme/DB kopyası yok. FDB, önceki kill+temizlikten kalan temiz50k',
        '  durumudur: ilk50k donör havuzu ve bakım testlerinin sınırlı değişiklikleri.',
        '- SQLite başlangıçtaki temiz50k\'dan, mevcut FDB ledger\'ındaki son',
        '  payload/version\'lara dört hazırlık commit\'iyle eşitlendi; bu işlemler',
        '  sorgu zamanlaması dışındadır. İki model dosyasının SHA\'sı birebir aynı.',
        '- FDB→SQLite, SQLite→FDB;10 örnek/case/koşu, pooled20, nearest-rank p95.',
        '  Aynı5 sorgu her turda döndürülerek çalıştırıldı. Her backend\'de full',
        '  get/version oracle ve beş warmup sonrası ölçüm; her sayfa ayrıca doğrulandı.',
        '- Cache: OS cache düşürülmedi. Bu bir warmup prosedürüdür; tüm dosyaların',
        '  RAM\'de kalmasını veya arka plan izolasyonunu garanti etmez. Gerçek cold',
        '  test yapılmadı. SQLite FTS5 trigram/detail=none, FULL WAL, cache2MiB,',
        '  mmap0, tempFILE; önceki LIKE/AND count+ordered-page yolu aynı.',
        '- Normal binary p95 için kullanılır. Ayrı diagnostic binary\'de iç içe',
        '  duvar süreleri/çağrı/byte sayaçları alınır; bunlar CPU attribution değildir.',
        '  Yüzdeler instrumented query süresine göredir, normal p95\'e taşınmaz.',
        '- İki ilk diagnostic koşu aynı sorguyu arka arkaya5 kez ölçer. Ana aşama',
        '  tablosu ayrıca normalle aynı dönüşümlü sıradaki10 diagnostic örneğe dayanır;',
        '  cache etkisini sırayla değiştirmiş iki veri seti birleştirilmez.','',
        '## Normal exact-total sonucu','',
        '| Sorgu | Gerçek total | Dönen hit | FDB p95 ms | FDB max ms | SQLite p95 ms |150ms |',
        '|---|---:|---:|---:|---:|---:|---|']
    for row in rows:
        n,s=row['normal'],row['sqlite']
        assert (n['total'],n['returned'])==(s['total'],s['returned'])
        out.append(f"| {row['query']} | {n['total']} | {n['returned']} | {n['p95_ms']:.3f} | {n['max_ms']:.3f} | {s['p95_ms']:.3f} | {'GEÇTİ' if n['p95_ms']<=150 else 'KALDI'} |")
    out += ['', 'Çıktı50 hit ile sınırlı olsa da exact total işi sınırlamıyor. Sonuç adedi',
            'bilgi amaçlıdır; eski10k/50k oranları bir kabul kapısı değildir. Güncel',
            '50k modelindeki küçük churn farkı nedeniyle önceki10k zamanlarıyla yeni',
            'bir doğrudan A/B yapıldığı iddia edilmez.','',
            '## Adaylar, I/O ve metin — dönüşümlü diagnostic ortalamaları','',
            '| Sorgu | Payload adayları | Okuma isteği | Okunan MiB | Tanılama toplam ms | Aday doğrulama ms/% |',
            '|---|---:|---:|---:|---:|---:|']
    for row in rows:
        c=row['profile_mean'];get=lambda k,field='ms':c.get(k,{}).get(field,0)
        out.append(f"| {row['query']} | {get('query.payload_candidates','calls'):.0f} | {get('query.payload_read_requests','calls'):.0f} | {get('query.payload_read_bytes','calls')/1048576:.2f} | {row['profile_mean_ms']:.2f} | {get('query.candidate_verify'):.2f} / {100*get('query.candidate_verify')/row['profile_mean_ms']:.1f}% |")
    out += ['', '| Sorgu | Payload okuma ms | CRC ms | Binary schema ms | Metin kontrolü ms | Posting hazırlık ms | Kesişim ms |',
            '|---|---:|---:|---:|---:|---:|---:|']
    for row in rows:
        c=row['profile_mean'];get=lambda k:c.get(k,{}).get('ms',0)
        out.append('| '+row['query']+' | '+' | '.join(f'{get(k):.2f}' for k in ['query.payload_read','query.payload_crc','query.payload_validation','query.verify_text','query.posting_setup','query.posting_intersection'])+' |')
    out += ['', 'Payload/CRC/schema/metin süreleri aday doğrulama üst süresinin içindedir;',
            'bunları üst süreyle tekrar toplamayın. Metin kontrolü katlama, tarama ve',
            'mevcut ASCII positive proof yolunu kapsar. Okunan byte mantıksal source',
            'isteğidir; OS cache miss ya da fiziksel disk byte sayısı değildir.','',
            '| Sorgu | Metadata cache hit/miss | Term lookup hit/miss | Term page miss |',
            '|---|---:|---:|---:|']
    for row in rows:
        c=row['profile_mean'];get=lambda k:c.get(k,{}).get('calls',0)
        out.append(f"| {row['query']} | {get('query.metadata_page_hits'):.1f}/{get('query.metadata_page_misses'):.1f} | {get('query.term_lookup_hits'):.1f}/{get('query.term_lookup_misses'):.1f} | {get('query.term_page_misses'):.1f} |")
    out += ['', 'Bunlar motorun mevcut metadata256KiB, term-directory1MiB içindeki64KiB',
            'lookup bütçelerinin sayaçlarıdır; OS cache hit/miss ölçümü değildir.',
            'Cache miss vardır ama temel maliyet aday doğrulamadır. Yalnız sözlük',
            'cache\'ini artırmanın bütün150ms açığını kapatacağı söylenmez.','',
            '## Total sorusunun cevabı ve capped sınırı','',
            'Bu beş uzun/AND sorguda **posting kesişiminden geçen bütün görünür',
            'payload adayları** kontrol ediliyor. Bütün50k belge her zaman okunmuyor;',
            'tabloda aday sayıları görülüyor. İlk50 hit capture edildikten sonra',
            'exact count için döngü devam ediyor. Sayım ayrı ikinci geçiş değil;',
            'her doğrulanmış eşleşmedeki increment. Maliyet, bu increment\'i yapabilmek',
            'için ilk sayfadan sonra da doğrulamayı sürdürmek.','',
            'Capped olumlu sorgularda erken durabilir; bu henüz ölçülmüş bir hız',
            'kazancı değildir. **Transubstantiation total=0:**7563 false-positive',
            'trigram adayı/267,81MiB doğrulanıyor. Capped doğru sıfır sonucu için yine',
            'tüketmelidir. Bu sorgu150ms\'yi aşarsa kullanıcı kuralıyla durulacak;',
            'kendi kendimize negatif/uzun kelime istisnası tanımlamadık.','',
            '## Strict-open aşamaları','',
            'Normal, yeni proses açılış örnekleri(ms): '+', '.join(f'{v:.3f}' for v in data['strict_open_normal_ms'])+'.',
            'İlk kötü2378,830ms örneği korunur. OS cache evict edilmedi; bu örnek',
            'gerçek-soğuk diye adlandırılmaz, dalgalanmanın kök nedeni bu tur ölçülmedi.',
            'Üç örnek p95 üretim garantisi değildir. Diagnostic aşamalar ayrı:','',
            '| Aşama | r0 ms | r1 ms | r2 ms | Ortalama / toplam % |',
            '|---|---:|---:|---:|---:|']
    for key,label in [('open.total','Toplam'),('open.source_native_check','Kaynak tam native/CRC/schema'),('open.index','İndeks açılışı'),('open.index_metadata_prepare','↳ Metadata hazırlığı'),('open.index_binary_directory_parse','↳↳ SEG05 directory parse'),('open.index_crc','↳ İndeks CRC')]:
        a=times(key)
        out.append(f'| {label} | '+' | '.join(f'{v:.3f}' for v in a)+f' | {statistics.mean(a):.3f} / {100*statistics.mean(a)/top:.1f}% |')
    remainder=top-mean('open.source_native_check')-mean('open.index')
    out += ['',f'Kaynak ve indeks dışı kalan toplam ortalama {remainder:.3f}ms. Kaynak ve',
            'indeks üst aşamaları ayrıdır; metadata/parse/CRC satırları indeksin',
            'içindedir. Yalnız kaynak kontrolünü erteleyince bile indeks294–344ms',
            'kalıyor. **CRC ertelemesi tek başına <100ms hedefini sağlamıyor.**','',
            '## Açılış sınır I/O probu — işlevsel DB değildir','',
            f"Beş tekrar {min(s['elapsed_ms'] for s in probe):.3f}–{max(s['elapsed_ms'] for s in probe):.3f}ms;26 run,81 dosya açma,2.247MB mantıksal okuma.",
            'DB kilidi, manifest CRC, source boyut/magic/footer sınırları, SEG05',
            'başlık/son bağları ve mevcut MET04 checksum/root header kontrol edildi.',
            '**Kaynak bütünlüğü, tam indeks CRC ve SEG05/MET04 record karşılaştırması',
            'yapılmadı; Database/Snapshot kurulmadı, sorgu çalışmadı.** Bu sayı hızlı',
            'açılış geçti değildir; yalnız ertelemeye uygun I/O alt işinin maliyetidir.',
            'Hızlı oturum dönüşü, ilk sonuç/kaynak beklemesi ve tüm doğrulama bitişi',
            'gerçek prototipte ayrı ölçülmeden100ms kapısı kapatılamaz.','',
            '## Güvenlik, uyumluluk, devam','',
            f'- Normal test paketi{passed} geçti; diagnostic lib139 geçti/12 subprocess yardımcı ignored. fmt/clippy, no-default/all-features, native SQLite kontrolleri geçti.',
            '- Mevcut strict kaynak kesilme/tek-byte bozulma ve process-kill testleri',
            '  tekrar geçti. Önceki gerçek50k dört kill penceresi aynı blokta kabul',
            '  edilmişti; bu read-only profilde yeni50k kill yükü oluşturulmadı.',
            '- Donmuş24Ekim okuyucu50k payload/version,100 deleted get ve38 sorguyu',
            '  tekrar doğruladı. Corpus/0.1 fixture/FDB source bytes aynı. SQLite',
            '  yalnız zamanlama dışı model eşitleme için değişti.',
            '- Ürün değişikliği yalnız mevcut diagnostic-timing altında sayaçtır;',
            '  default query, storage/recovery ve codec mantığı değişmedi. Capped',
            '  API, lazy open, yeni kalıcı dosya/sürüm yok. Reclaim varsayılan kapalı.',
            '- Hızlı oturum tasarımı onaylandı; **capped N=50/150ms kapı seçimi ve',
            '  uygulama yanıtı bekleniyor**. Fast prototype ancak capped adımının',
            '  kapısı izin verince. 50k tekli/dirty500/4R/100\'lü yazı profili,2c,2b',
            '  henüz başlamadı. 100k yeni gerçek dosya corpus\'unu bekler.',
            '- Case34, geniş zx/the zx, parçalı read sayısı, query RAM ve yazma/bakım',
            '  açık kalır. Blok3/4 ve C ABI başlamadı.','',
            '## Ham kanıt','',
            f'- [Program hash\'leri, sıra, ayarlar, ledger/source SHA](benchmarks/{P}-design.json).',
            '- [Normal p95 ve bütün diagnostic sayaç ortalamaları](benchmarks/block2-query-open-50k-20261028-analysis-v1.json).',
            '- [Dönüşümlü ham diagnostic](benchmarks/block2-query-open-50k-20261028-v2-interleaved-profile.jsonl).',
            '- [Açılış sınır probu](benchmarks/block2-query-open-50k-20261028-boundary-probe-v1.json).',
            '- [Yeni50k eski okuyucu oracle\'ı](benchmarks/block2-query-open-50k-20261028-compat-v1-old-reader-utf8-v2.json).',
            '- [Normal test/fmt/clippy komut kanıtı](benchmarks/snapshot-10k-20261026-validation-query-open-50k-20261028-v1.json).',
            '- [Diagnostic lib testleri](benchmarks/block2-query-open-50k-20261028-v2-diagnostic-lib-tests.log).',
            f'- Ham normal sorgular: `{P}-pair-r{{0|1}}-{{normal|sqlite}}-scale-target-matrix.jsonl`; kaynak açılışları: `{P}-open-r{{0|1|2}}-*`.','']
    target=ROOT/'docs/QUERY-COST-OPEN-50K-20261028.md'
    assert not target.exists()
    target.write_text('\n'.join(out),encoding='utf-8')
    print(target)


if __name__=='__main__':
    main()
