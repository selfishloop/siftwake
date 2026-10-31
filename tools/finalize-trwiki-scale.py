"""Attach completed checks/counter evidence and update current status, no DB I/O."""
import importlib.util
import json
import re
from pathlib import Path

R=Path(__file__).resolve().parent.parent
L=R/'docs/benchmarks'
P='trwiki-scale-20261028-v1'
spec=importlib.util.spec_from_file_location('scale',R/'tools/measure-trwiki-scale.py')
s=importlib.util.module_from_spec(spec)
spec.loader.exec_module(s)


def main():
    checks=json.loads((L/'snapshot-10k-20261026-validation-trwiki-scale-20261028-v2.json').read_text(encoding='utf-8-sig'))
    assert len(checks)==8 and all(x['exit_code']==0 for x in checks)
    diag_test=L/f'{P}-diagnostic-lib.log'
    dt=diag_test.read_text(encoding='utf-8')
    assert 'test result: ok.' in dt and 'test result: FAILED' not in dt
    d=json.loads((L/f'{P}-design.json').read_text(encoding='utf-8'))
    post=json.loads((L/f'{P}-post-verification.json').read_text(encoding='utf-8'))
    assert post['full_payload_version_oracle_after_kill']
    assert all(s.b.sha(R/p)==v for p,v in post['engine_source_sha256'].items())
    assert s.b.protected()==d['protected_after'] and s.b.sha(s.CORPUS)==d['provided_source_after']
    report=R/'docs/TRWIKI-SCALE-20261028.md'
    text=report.read_text(encoding='utf-8')
    assert '## Tanı ve son doğrulama' not in text,'Do not duplicate report appendix'
    text+='''
## Tanı ve son doğrulama

50k araması durduktan sonra mevcut **66000 commit /12 segment** üzerinde
yalnız iki sorgunun counter tanısı alındı. Bu,50k/100k normal p95 matrisinin
yerine geçmez. Diagnostic release mutex/counter maliyeti taşır;1 örnek/yol,
bir warmup ve full GET/model oracle vardır. Kaynak/index değiştirilmedi.
Kaynak başına CRC tekrarları korunur; fiziksel disk erişimi değil, sıcak OS
cache'ine yapılan dosya okuma istekleri sayılmıştır.

| Sorgu | Yol | Süre ms | Posting probe (her biri4B) | Probe span toplamı ms | Doğrulanan aday | Payload read MB | CRC ms | Metin doğrulama ms | Sonuç |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
'''
    rows=[]
    for line in (L/post['profile']['log']).read_text(encoding='utf-8').splitlines():
        x=json.loads(line)
        if x['phase']!='query_experiment':continue
        rows.append(x)
        c=x['counters']
        calls=lambda k:c.get(k,{}).get('calls',0)
        ms=lambda k:c.get(k,{}).get('elapsed_ns',0)/1e6
        text+=f"| {x['text']} | {x['variant']} | {x['elapsed_ms']:.3f} | {calls('query.posting_probe')} | {ms('query.posting_probe'):.3f} | {calls('first.candidate_verify')} | {calls('query.payload_read_bytes')/1e6:.3f} | {ms('query.payload_crc'):.3f} | {ms('query.verify_text'):.3f} | {x['returned']} |\n"
    serial=next(x for x in rows if x['text']=='yanıklarda' and x['variant']=='cap')
    c=serial['counters'];pct=lambda k:100*c[k]['elapsed_ns']/1e6/serial['elapsed_ms']
    text+=f'''
Seri tanıda posting probe span'leri toplam sürenin **%{pct('query.posting_probe'):.2f}**'si,
metin doğrulama **%{pct('query.verify_text'):.2f}**, payload CRC **%{pct('query.payload_crc'):.2f}**.
Metin doğrulamanın içindeki body fold ayrı olarak{c['query.fold.body']['elapsed_ns']/1e6:.3f}ms;
bu alt span'ler üst span'lerle toplanmaz. Paralel satırlarda thread span'leri
örtüşür; wall yüzdesi veya CPU toplamı sayılmaz. Seri/4/melez aynı6577 adayı
doğrular; melez prefix'i tekrar tüketmez.

Kodda4096 ID'yi aşan posting grupları küçük-group cache'ine alınmıyor;
`contains()` her seed için `id_at()` üzerinden4B probe'larıyla ikili arama
yapıyor. Ölçülen380033 probe yalnız1,52MB veri istediği halde seri tanıda
868ms tutuyor. Bu veri hacmi problemi kadar **okuma isteği sayısı** problemidir.
Sonraki öneri, aynı biçim/CRC/sıralı-ID kontrolleri ve bellek bütçesi içinde
sayfalı posting okuma veya ilerleyen intersection cursor'u denemek; bu tur
uygulanmadı.50k p95'inin tam aşama yüzdeleri ayrıca ölçülmedi.

Pending background oturumu{post['pending_kill']['witness']['session_ms']:.3f}ms'de döndü;
bu durumda process kill yapıldı. Sonraki normal strict-open+tam payload/version
GET ve reddedilen500 anahtarın absence oracle'ı geçti. `.fdb/.ready` hash'leri
değişmedi. Bu process-kill kanıtıdır; güç kesintisi garantisi değildir.

### Komut kapıları

Paralel ilk `cargo test --all-targets` paketi ilerlemeden bekledi; elle
durduruldu, **GEÇTİ sayılmadı**. Sebebi teşhis edilmedi, açık işte tutulur.
Tekrar aşağıdaki aynı komutlarda `RUST_TEST_THREADS=1` ile yapıldı. Sadece
testlerin birbirleriyle paralelliği sınırlandı; testlerin kendi worker/thread
ve subprocess-kill senaryoları çalışmaya devam eder. Test değiştirilmedi.

| Komut | Durum | Log |
|---|---|---|
'''
    for x in checks:
        text+=f"| `{x['command']}` | GEÇTİ | [{x['name']}](benchmarks/{Path(x['log']).name}) |\n"
    text+=f'| `cargo test --lib --features diagnostic-timing` (`RUST_TEST_THREADS=1`) | GEÇTİ | [diagnostic](benchmarks/{diag_test.name}) |\n'
    default=(R/next(x['log'] for x in checks if x['name']=='test')).read_text(encoding='utf-8')
    totals=lambda value:dict(passed=sum(int(x) for x in re.findall(r'test result: ok\. (\d+) passed;',value)),ignored=sum(int(x) for x in re.findall(r'test result: ok\. \d+ passed; \d+ failed; (\d+) ignored;',value)))
    total=totals(default);diagnostic=totals(dt)
    text+=f'''
Default suite:{total['passed']} geçti/{total['ignored']} subprocess helper ignored;
diagnostic lib:{diagnostic['passed']} geçti/{diagnostic['ignored']} helper ignored.
Kesilme/tek-byte bozulması, WAL/streamed commit, reclaim/merge/snapshot ve
process-kill kurtarma testleri bu seri tekrarın içindedir. Motorun
{len(post['engine_source_sha256'])} kaynak dosyasının SHA'sı önceki kabul edilen çalışma
sonuyla aynı; bu tur değişiklikleri ölçüm araçları ve raporlardır.

[Paralel doğrulama duruşu](benchmarks/{P}-parallel-validation-stop.json),
[ilk test logu](benchmarks/snapshot-10k-20261026-validation-trwiki-scale-20261028-v1-test.log),
[tanı/kill/hash kanıtı](benchmarks/{P}-post-verification.json),
[eski okuyucu66000 GET+500 absence+41 sorgu](benchmarks/{P}-compat-partial66000-old-reader-utf8-v2.json).

Otomatik onay denetimi, durdurulan testten kalan16 Temp klasörünü temizleme
işlemini `blocked by policy` gerekçesiyle reddetti; daha dar literal-path
denemesi de reddedildi. [6729B artık](benchmarks/{P}-aborted-test-cleanup.json)
kaldı; corpus veya benchmark DB temizliği değildir. Hiçbir ret gizlenmedi.
'''
    report.write_text(text,encoding='utf-8')
    status='''28 Ekim güncel sonuç: **50k capped N=50 arama KALDI; 100k FDB ingest KALDI;
bağımlı aşamalar durdu.** [Türkçe corpus raporu](REPORT).
50k iki dengeli kolda: ingest38,44–39,13sn, disk300,84MB, temiz idle private
2,23MiB altı, ingest peak19,95MiB altı, strict-open p95281,83ms: RAM/açılış
kapıları geçti. Melez arama37 kapılı sorgunun9'unda kaldı (yanıklarda582ms,
külliyatı459ms). Exact ayrıca raporlandı; varsayılan exact/serial değişmedi.

100k ingest, tek encoded entry256KiB sınırında durdu:66498 numaralı text444345B.
İlk66000 belge/sequence132 tutarlı; reddedilen500 anahtar görünmez. Corpus
kırpılmadı, owned fallback/limit/format değişimi yok. SQLite100k iki bağımsız
ingest/disk/RAM/41-case tekrarı tamam; tam100k dengeli A/B ve FDB açılış/arama
ölçümü yok.444KB cancel ölçülemedi. [Aktif slot kimliği](SLOT) artık FDB66000
commit + SQLite100k; eski kitap-DB slot pointer'ları güncel değildir.

Tanı:66000 commit'te yanıklarda380033 adet4B posting probe'u, seri tanı
süresinin%70,58'i; metin doğrulama%20,32, payload CRC%0,66. Sonraki aday
biçim değiştirmeden posting okuma isteğini azaltmak; henüz uygulanmadı.
Oracle, eski okuyucu ve Pending kill/reopen geçti. İlk paralel test paketi
takıldı/durduruldu; sebebi açık. Seri tekrar ve fmt/clippy geçti; ayrıntı raporda.

Açık sıra: bu iki engel için kullanıcı kararı →50k tekli/dirty500/4R+yazıcı/
100'lük commit-RAM kök nedeni → uzun snapshot/maks ömür → gerçek-soğuk cache.
Case34/parçalı geniş tarama, sorgu RAM'i, signed-zero normal light/owned farkı
ve paralel test takılması açık. Melez200+cancel opt-in; join/CRC reuse ve
reclaim default kapalı. Ön filtre/format değişimi, Blok3/4, C ABI ve dış proje
entegrasyonu yok. Testten kalan6729B Temp temizliği otomatik denetimce reddedildi.
Aşağıdaki eski karar ve sonuçlar tarihsel kayıttır.'''
    for name in ['README.md','docs/ARCHITECTURE.md','docs/OPEN-WORK.md']:
        p=R/name;old=p.read_text(encoding='utf-8');start=old.index('28 Ekim en son karar:');end=old.index('\n\n\n',start)
        new=status.replace('REPORT','docs/TRWIKI-SCALE-20261028.md' if name=='README.md' else 'TRWIKI-SCALE-20261028.md').replace('SLOT','docs/benchmarks/active-benchmark-slots.json' if name=='README.md' else 'benchmarks/active-benchmark-slots.json')
        if name=='README.md':
            new='''28 Ekim [Türkçe50k/100k raporu](docs/TRWIKI-SCALE-20261028.md):
50k ingest38,44–39,13sn, disk300,84MB, idle private≤2,23MiB,
ingest peak≤19,95MiB, strict-open p95281,83ms. RAM/açılış geçti;
melez N50 arama37 kapılı sorgunun9'unda kaldı.100k ingest,444KB belgeyi
tek-entry256KiB sınırında reddetti; FDB66000 commit'te, SQLite100k referansı
tamam. Bağımlı aşamalar durdu; uzun-belge cancel/100k FDB query ölçülmedi.
Varsayılan exact/serial, kalıcı biçimler ve motor kaynakları değişmedi.
Melez/cancel opt-in; [güncel kapılar/açık işler](docs/OPEN-WORK.md),
[aktif slot kimliği](docs/benchmarks/active-benchmark-slots.json).
İlk paralel test takılması açık; seri tekrar ve fmt/clippy geçti.
Aşağıdaki eski karar ve sonuçlar tarihsel kayıttır.'''
        if name=='docs/ARCHITECTURE.md':
            new='''28 Ekim [Türkçe corpus ölçek ölçümü](TRWIKI-SCALE-20261028.md):
bu tur motor kaynakları ve kalıcı biçimler değişmedi; exact/serial varsayılan,
melez200/cancel opt-in, join/CRC reuse ve reclaim default kapalı.
50k RAM/strict-open geçti, capped arama kaldı.100k streamed ingest,
tek kodlanmış entry256KiB sınırında444345B metni reddetti; atomik grup
görünmedi. Bu64MiB transaction sınırı değildir. Küçük posting probe'ları
tanıda baskın; sayfalı/ilerleyen okuma önerisi uygulanmadı. Uzun snapshot,
50k yazma ve soğuk cache ölçümleri [açık işte](OPEN-WORK.md) bekliyor.
İlk paralel test takılması ayrıca açık; seri test tekrarları geçti.
Aşağıdaki eski karar ve sonuçlar tarihsel kayıttır.'''
        if name=='docs/OPEN-WORK.md':
            new+='''

| Güncel kapı | Durum |
|---|---|
|50k temiz idle≤15MiB / ingest peak≤60MiB / strict≤1000ms|GEÇTİ|
|50k capped N50: ortak≤30ms, diğer≤150ms|KALDI: melez9/37|
|Bol-pozitif melez CPU≤serial+%5|GEÇTİ: en kötü+%3,46|
|Negatif melez p95≤4 işçi+%10|KALDI:0,369→0,612ms (+0,244ms)|
|100k FDB ingest|KALDI: tek kayıt256KiB|
|100k FDB open/query /444KB cancel|BLOKE, ölçülmedi|
|SQLite100k bağımsız iki tekrar|TAMAMLANDI|
|Oracle / eski okuyucu / Pending kill-reopen|GEÇTİ: ölçülen kapsamda|
|fmt / clippy / tek-thread test tekrarları|GEÇTİ; ilk paralel hang açık|
'''
        p.write_text(old[:start]+new+old[end:],encoding='utf-8')
    links=[]
    for target in re.findall(r'\]\(([^)]+)\)',text):
        if '://' not in target and not target.startswith('#'):
            assert (report.parent/target).exists(),target
            links.append(target)
    s.b.write(L/f'{P}-final-audit.json',dict(checks=checks,test_environment={'RUST_TEST_THREADS':'1'},
        default_tests=total,diagnostic_tests=diagnostic,parallel_first_attempt_passed=False,
        engine_source_changed=[],engine_source_sha256=post['engine_source_sha256'],
        corpus_unchanged=True,old_corpus_and_fixture_unchanged=True,private_read=False,
        source_formats_changed=False,subagents_used=False,search_default_changed=False,
        fdb_100k_completed=False,fdb_current_committed_documents=66000,
        long_document_cancel_measured=False,gate_failed_at=50000,
        report_sha256=s.b.sha(report),report_links_verified=len(links),
        temp_cleanup=json.loads((L/f'{P}-aborted-test-cleanup.json').read_text(encoding='utf-8'))))
    print(json.dumps(dict(report=str(report),default_tests=total,diagnostic_tests=diagnostic,links=len(links)),ensure_ascii=False))


if __name__=='__main__':main()
