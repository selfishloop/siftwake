"""Publish current gate/status notes from completed evidence, no DB mutations."""
import importlib.util
import json
import re
from pathlib import Path

R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='posting-pages-50k-20261029-v3'
spec=importlib.util.spec_from_file_location('measure',R/'tools/measure-posting-pages.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
s=m.s

def main():
    d=json.loads((L/f'{P}-design.json').read_text(encoding='utf-8'))
    a=json.loads((L/f'{P}-analysis.json').read_text(encoding='utf-8'))
    v=json.loads((L/f'{P}-verification.json').read_text(encoding='utf-8'))
    checks=json.loads((L/'snapshot-10k-20261026-validation-posting-pages-20261029-v3.json').read_text(encoding='utf-8-sig'))
    assert len(checks)==8 and all(x['exit_code']==0 for x in checks)
    diag=(L/'posting-pages-20261029-v3-diagnostic-tests.log').read_text(encoding='utf-8')
    assert '149 passed; 0 failed; 13 ignored;' in diag
    assert v['old_reader']['oracle'] and v['old_reader']['full_payload_version_gets']==50000
    for e,p in m.programs.items():assert s.b.sha(p)==d['programs'][e]['sha256']
    for p,sha in d['implementation_source_sha256'].items():assert s.b.sha(R/p)==sha,p
    assert s.b.protected()==d['protected_after'] and s.b.sha(s.CORPUS)==d['provided_source_after']
    report=R/'docs/POSTING-PAGES-50K-20261029.md'
    text=report.read_text(encoding='utf-8')
    assert '## Son kapı tablosu' not in text
    text+='''
## Son kapı tablosu

| Madde | Durum | Kanıt |
|---|---|---|
|37 sorgu: melez N50 p95 ortak≤30ms / diğer≤150ms|GEÇTİ|20 örnek/sorgu; exact ayrıca raporlandı|
|Negatif melez: kontrol<1ms ise +2ms, aksi +%10|GEÇTİ|İki negatif; tarihsel qzxkvwnever da yeniden sınıflandı|
|Pozitif melez CPU≤serial+%5|KALDI|büyük+%6,06, +0,09375ms/çağrı; tolerans eklenmedi|
|Temiz idle private≤15MiB|GEÇTİ|Yeni2,129MiB; proses sayacı|
|Sorgu kontrollü posting bütçeleri|GEÇTİ|256KiB /8MiB /512KiB sınırları; optional alan sonradan|
|Exact/capped full payload/version oracle|GEÇTİ|Normal ölçülen bütün sayfalar, tüm37 sorgu|
|Kesilme/CRC/ID sırası/kill-reopen/eski okuyucu|GEÇTİ|Unit+integration, Pending kill,50k GET+41 sorgu|
|Paralel test takılması|ÇÖZÜLDÜ|Kontrollü wait-cycle, DB başına pause; default-paralel243 + diagnostic149 test|
|256KiB entry /kalıcı biçim /varsayılan exact-serial|KORUNDU|Kaynak SHA/old reader; melez opt-in|
|100k /yeni max200KB corpus|BEKLİYOR|Kullanıcı yeni dosyayı sağlayacak;100k başlatılmadı|
|Blok3/4 /C ABI /ön filtre|BAŞLATILMADI|Kapsam dışında|

Diagnostic clippy ve [16-thread diagnostic149 test](benchmarks/posting-pages-20261029-v3-diagnostic-tests.log)
nihai kodda geçti. Pozitif CPU kapısı KALDI olduğundan bunu tüm kapılar
geçti diye sunmuyoruz. Normal exact/serial okuma iyileştirmesi ve melezin
ölçülmüş kazanımları geri alınmadı; bir sonraki adım kullanıcı kararıdır.
'''
    text=text.replace('bütçe200','bütçe 200').replace('bütün37','bütün 37').replace('kontrollü posting bütçeleri','kontrollü posting bütçeleri')
    report.write_text(text,encoding='utf-8')
    status='''29 Ekim sonuç: **posting sayfası + ilerleyen cursor uygulandı; melez N=50
arama kapısı37/37 GEÇTİ.** [Tam37 sorgu p95/CPU/SQLite ve kötü sonuçlar](REPORT).
Aynı50k DB, iki dengeli sıra,20 örnek/yol: yanıklarda751,58→133,53ms,
külliyatı569,47→105,04ms, ormanların877,46→119,86ms,
çalışmalarıyla768,32→78,26ms, kısım dişli709,20→136,81ms,
kendisine çıkacağını666,37→106,50ms; İnternetten hazinem76,62,
oyuncudur atlanta59,87, geldiği buldu37,90ms. Exact/serial ayrıca ölçüldü.

Kök neden aynı50k seri tanısında: yanıklarda556367 adet4B probe→250 sayfa
okuması;5013 doğrulanan aday aynı. Ara page-only denemesi103–110bin tekrar
okumada iki sorguda kaldı; ilerleyen konum bu yüzden eklendi. Bütçeler
256KiB legacy /8MiB serial first-page /512KiB worker aynı; optional alan
zorunlu cursor'lardan sonra ayrılır. CRC/range/sıralı-ID kontrolleri korundu.
Temiz idle private2,129MiB; sorgu process peak katalog/model/oracle dahil
yaklaşık34,63MiB, eski max34,84MiB. Kalıcı dosyalar değişmedi.

**Kalan CPU kapısı:** büyük melez, yeni serial capped'e göre+%6,06
(+0,09375ms/çağrı),+%5 sınırında KALDI. Diğer pozitifler+%1,81/+%3,95.
Gürültü/toleransla kapatılmadı. Negatif yeni mutlak/+%10 kuralı GEÇTİ;
tarihsel qzxkvwnever+0,2435ms artık GEÇTİ. Melez200/cancel opt-in,
varsayılan exact/serial; join/CRC reuse/reclaim default kapalı.

[Paralel test deadlock'u](DEADLOCK) çözüldü: test global kaynak kilidini
tutarken başka DB fallback'i14MiB bütçeyle o kilidi bekliyor, testin6MiB
girişi bloklanıyordu. DB'ye özel worker pause ve dört iznin tamamını tutan
admission testiyle düzeltildi. Üretim pool/kilitleri artırılmadı;243 default
paralel test+149 diagnostic/fmt/clippy geçti. Oracle, Pending kill/reopen,
eski okuyucu50k GET+41 sorgu ve0.1 fixture koruması geçti.

100k **yeni kullanıcı-kırpılmış max200KB corpus BEKLİYOR**;256KiB entry
sınırı değişmedi. Yeni dosyada audit/oracle yenilenecek; private/ okunmayacak.
Aktif sabit slotlar aynı eski/onaylı corpus'un50k FDB+SQLite DB'leridir.
Açık işler: CPU+%5 kararı; yeni corpus100k;50k yazma/bakım profili, tekli,
dirty500,4R+yazıcı,100'lük commit/RAM; uzun snapshot ve gerçek-soğuk cache;
case34/parçalı geniş tarama ve normal light/owned signed-zero farkı.
Format/ön filtre/Blok3/4/C ABI/dış entegrasyon yok. Önceki sonuçlar tarihseldir.
'''
    for name in ['README.md','docs/ARCHITECTURE.md','docs/OPEN-WORK.md']:
        p=R/name;old=p.read_text(encoding='utf-8');start=old.index('29 Ekim kullanıcı kararı:');end=old.index('\n\n',start)
        if name=='docs/OPEN-WORK.md':
            new=status.replace('REPORT','POSTING-PAGES-50K-20261029.md').replace('DEADLOCK','PARALLEL-TEST-DEADLOCK-20261029.md')
            new+='''
| Güncel kapı | Durum |
|---|---|
|37-case melez N50 arama|GEÇTİ37/37|
|Negatif mutlak/+%10|GEÇTİ|
|Pozitif CPU+%5|KALDI: büyük+%6,06 (+0,094ms)|
|Idle /kontrollü sorgu bütçeleri|GEÇTİ|
|Oracle /CRC /kill /eski okuyucu|GEÇTİ|
|Paralel test deadlock'u|ÇÖZÜLDÜ; seri moda kaçılmadı|
|256KiB entry sınırı|AYNI|
|100k yeni max200KB corpus|BEKLİYOR|
'''
        elif name=='README.md':
            new='''29 Ekim [posting cursor ölçümü](docs/POSTING-PAGES-50K-20261029.md):
melez N50 arama37/37 kapısı geçti. Yanıklarda751,6→133,5ms,
çalışmalarıyla768,3→78,3ms; kaynak/indeks CRC ve bütçeler korundu.
[Paralel test deadlock'u](docs/PARALLEL-TEST-DEADLOCK-20261029.md) çözüldü;
243 default-paralel+149 diagnostic test/fmt/clippy/oracle/kill/eski okuyucu geçti.
Bir pozitif CPU kapısı KALDI: büyük+%6,06 /+0,094ms; gizlenmedi.
256KiB entry aynı;100k yeni max200KB kullanıcı corpus'unu bekliyor.
Varsayılan exact/serial; melez/cancel opt-in. [Açık işler](docs/OPEN-WORK.md).
Önceki sonuçlar tarihseldir.'''
        else:
            new='''29 Ekim: sorguya özel kontrollü posting sayfaları + ilerleyen lower-bound
cursor eklendi. Geri giden ID/evicted hint güvenli binary fallback kullanır;
CRC/ID-sırası,256KiB/8MiB/512KiB bütçeleri ve kalıcı biçimler aynı.
Optional alan bütün zorunlu cursor'lardan sonra ayrılır. Exact/serial default,
melez200/cancel opt-in kaldı. [37-case p95/CPU ve açık CPU kapısı](POSTING-PAGES-50K-20261029.md).
[Test deadlock'u](PARALLEL-TEST-DEADLOCK-20261029.md) test-only path pause ile
çözüldü; üretim working pool artırılmadı. Entry256KiB sınırı değişmedi;
100k yeni max200KB kullanıcı corpus'unu bekliyor. Ön filtre/Blok3/4/C ABI yok.
Önceki sonuçlar tarihseldir.'''
        for x,y in {'aynı50k':'aynı 50k','Aynı50k':'Aynı 50k','kapısı37':'kapısı 37','arama37':'arama 37','örnek/yol:':'örnek/yol:',
            'yanıklarda751':'yanıklarda 751','Yanıklarda751':'Yanıklarda 751','külliyatı569':'külliyatı 569','ormanların877':'ormanların 877',
            'çalışmalarıyla768':'çalışmalarıyla 768','dişli709':'dişli 709','çıkacağını666':'çıkacağını 666','hazinem76':'hazinem 76',
            'atlanta59':'atlanta 59','buldu37':'buldu 37','aday aynı':'aday aynı','probe→250':'probe→250','melez200':'melez 200',
            'büyük+%':'büyük +%','fallback\'i14':'fallback\'i 14','testin6':'testin 6','sınırı değişmedi;':'sınırı değişmedi;',
            'corpus100k':'corpus 100k','243 default':'243 default','okuyucu50k':'okuyucu 50k','ve0.1':'ve 0.1','GEÇTİ37':'GEÇTİ 37'}.items():new=new.replace(x,y)
        p.write_text(old[:start]+new+old[end:],encoding='utf-8')
    dead=R/'docs/PARALLEL-TEST-DEADLOCK-20261029.md'
    t=dead.read_text(encoding='utf-8').replace('Son kontrol logu aşağıda','243 geçti; RUST_TEST_THREADS=1 kullanılmadı')
    t+='\nNihai ilerleyen cursor sonrası [tam paralel243 test](benchmarks/snapshot-10k-20261026-validation-posting-pages-20261029-v3-test.log) ve [16-thread diagnostic149 test](benchmarks/posting-pages-20261029-v3-diagnostic-tests.log) de geçti.\n'
    dead.write_text(t,encoding='utf-8')
    links=[]
    for doc in [report,dead]:
        for target in re.findall(r'\]\(([^)]+)\)',doc.read_text(encoding='utf-8')):
            if '://' not in target and not target.startswith('#'):
                assert (doc.parent/target).exists(),target;links.append(target)
    s.b.write(L/f'{P}-final-audit.json',dict(checks=checks,parallel_tests=243,diagnostic_tests=149,
        query_gate_passed=a['query_gate_passed'],all_gates_passed=a['all_gates_passed'],positive_cpu_gate=a['positive_cpu_gates'],
        negative_gate=a['negative_gates'],entry_limit_changed=False,persistent_format_changed=False,
        default_search_changed=False,subagents_used=False,corpus_generated=False,corpus_100k_started=False,
        source_sha256=d['implementation_source_sha256'],protected_unchanged=True,provided_source_unchanged=True,
        report_sha256=s.b.sha(report),links_verified=len(links),
        reused_controls=d['reused_control_scope'],page_only_bad_results_preserved=True))
    print(json.dumps(dict(report=str(report),query_gate='37/37 passed',remaining='positive CPU büyük+6.06%',links=len(links)),ensure_ascii=False))

if __name__=='__main__':main()
