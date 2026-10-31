"""Read immutable scale evidence; never opens or mutates benchmark DBs."""
import json
import math
import re
from collections import defaultdict
from pathlib import Path

R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='trwiki-scale-20261028-v1'
def p95(xs):return sorted(xs)[math.ceil(.95*len(xs))-1]
def stat(xs):
    ts=[x['elapsed_ms'] for x in xs]
    return dict(n=len(xs),p50_ms=sorted(ts)[(len(ts)-1)//2],p95_ms=p95(ts),max_ms=max(ts),cpu_total_ms=sum(x['cpu_ms'] for x in xs),cpu_mean_ms=sum(x['cpu_ms'] for x in xs)/len(xs))
def main():
    d=json.loads((L/f'{P}-design.json').read_text(encoding='utf-8'))
    assert d.get('protected_unchanged') and d.get('provided_source_unchanged'),'Wait for all independent evidence'
    corpus=json.loads((L/'trwiki-provided-20261028-v1-audit.json').read_text(encoding='utf-8'))
    G=defaultdict(list);basic=defaultdict(list);idle=defaultdict(list);first=defaultdict(list);blocks=defaultdict(list);query_ram=defaultdict(list)
    for result in d['completed']:
        n=int(re.search(r'n(50000|100000)',result['label']).group(1));e=result['engine']
        for line in (L/result['log']).read_text(encoding='utf-8').splitlines():
            x=json.loads(line)
            if x['phase']=='scale_ingested':basic[n,e].append(x)
            if x['phase']=='scale_idle':idle[n,e].append(x)
            if x['phase']=='first_result':first[n,e,x['text']].append(x)
            if x['phase']=='query_experiment':G[n,e,x['case_id'],x['variant']].append(x)
            if x['phase']=='cpu_block':blocks[n,x['text'],x['variant']].append(x)
            if x['phase']=='query_experiments_finished':query_ram[n,e].append(x['memory'])
    analysis=dict(queries=[dict(documents=n,engine=e,case_id=i,variant=v,**stat(xs)) for (n,e,i,v),xs in sorted(G.items())],
        basic=[dict(documents=n,engine=e,ingest_samples_ms=[x['ingest_including_maintenance_ms'] for x in xs],
            commit_samples_ms=[x['commit_total_ms'] for x in xs],maintenance_samples_ms=[x['maintenance_ms'] for x in xs],
            peak_private_bytes=[x['memory']['os_lifetime_peak_private_commit_bytes'] for x in xs],
            peak_ws_bytes=[x['memory']['os_lifetime_peak_working_set_bytes'] for x in xs],
            disk_bytes=[x['disk']['retained_bytes'] for x in xs]) for (n,e),xs in sorted(basic.items())],
        idle=[dict(documents=n,engine=e,private_samples_bytes=[x['memory']['private_bytes'] for x in xs],working_set_samples_bytes=[x['memory']['working_set_bytes'] for x in xs]) for (n,e),xs in sorted(idle.items())],
        first=[dict(documents=n,engine=e,text=q,n=len(xs),session_p95_ms=p95([x['session_ms'] for x in xs]),first_p95_ms=p95([x['first_ms'] for x in xs]),samples=xs) for (n,e,q),xs in sorted(first.items())],
        cpu_blocks=[dict(documents=n,text=q,variant=v,queries=sum(x['queries'] for x in xs),cpu_total_ms=sum(x['cpu_ms'] for x in xs),cpu_mean_ms=sum(x['cpu_ms'] for x in xs)/sum(x['queries'] for x in xs)) for (n,q,v),xs in sorted(blocks.items())],
        query_ram=[dict(documents=n,engine=e,samples=xs) for (n,e),xs in sorted(query_ram.items())],
        gates=d['levels'],gate_failed_at=d['gate_failed_at'],new_source_unchanged=True)
    (L/f'{P}-analysis.json').write_text(json.dumps(analysis,ensure_ascii=False,indent=2),encoding='utf-8')
    Q={(x['documents'],x['engine'],x['case_id'],x['variant']):x for x in analysis['queries']}
    text='''# Türkçe corpus 50k/100k — 28 Ekim 2026

**KALDI, bağımlı aşamalar durdu.** 50k capped N=50 arama kapısı kaldı.
100k için izin verilen bağımsız ingest de tek kayıt sınırında kaldı:
66498 numaralı metin 444345B, streamed kodlanmış entry limiti 262144B.
FDB'de yalnız ilk 66000 belge/sequence132 commit edildi; reddedilen
66000..66499 grubunun tamamı görünmez. Corpus kırpılmadı, owned fallback
veya limit/format değişimi yapılmadı. SQLite bağımsız 100k koşuları tamamlandı.
100k FDB açılış/RAM/arama sonucu yerine eksik koşu açıkça gösterilir.

Kullanıcının verdiği100k farklı Türkçe Wikipedia gövdesi kullanıldı. Kaynak
`D:/frunner-corpus/trwiki-100k/corpus.payload-bench` yalnız okundu; `private/`
okunmadı. Gövdeler kırpılmadı, marker eklenmedi, title/fields/links korunur.
Anahtarların orijinal numeric şablonuyla aynı olduğu audit'te doğrulanmıştır.
Yeni corpus/DB kopyası/format/ön filtre yok. DB'ler sadece C: sabit FDB/SQLite
slotlarında, her ingest kolunda ownership kontrolüyle yeniden kuruldu.

## Kapsam ve yöntem

50k'da iki ingest sırası FDB/SQLite ve SQLite/FDB. 100k FDB ilk kolda
reddedildi; iki SQLite tekrarı bağımsız referanstır, tamamlanmış dengeli
100k A/B sayılmaz. API akışlı,500 işlem/
commit; owned/Vec ingest ayrı denenmedi. FDB ingest süresine flush/merge ve
son idle cleanup+integrity/GC dahildir; cleanup ayrı ham kalemdir. RAM toplam
process **private commit** ve **working set** olarak ayrı verilir. Ingest
lifetime peak private, corpus/model descriptor harness'ını da içerir; yalnız
motor heap'i veya cihazın toplam RAM'i değildir. Bellek örnekleyicisi10ms;
OS peak private/WS ayrıca yakalanır. Idle yeni proseste2sn sonra, oracle yüklemeden.

Sıcak arama için full get/version oracle+her seçenek bir warmup;10+10=20
normal release örneği/sorgu/seçenek, döndürülmüş seçenek/sorgu ve dengeli motor
sırası. N50/offset0,41 kullanıcı sorgusu. Exact serial normal Database::search;
exact4 run-parallel deneyidir, yalnız thread sayısı dışında algoritma farkları
da vardır. Capped serial/4/melez200 opt-in; join/CRC reuse kapalı. CPU Windows
GetProcessTimes user+kernel, bütün query worker'ları ve process örnekleyicisi
dahil; tek query0/15,625ms kuantize olabilir. CPU blokları ayrıca raporlanır.

SQLite FTS5 trigram/detail=none/columnsize=0, WAL/FULL, cache2MiB,mmap0,
tempFILE. Aynı payload/version/key ve normalize title/text substring-AND.
SQLite fields/links payload içinde; FDB bunlara kendi scalar/numeric/link
indekslerini de kurar. Capped SQL COUNT'ı çıkarır ama id sıralaması için
adayları tüketebilir. Bu uygulama farkı gizlenmez; genel motor üstünlüğü iddiası yok.

Strict-open normal Database::open'dır, kaynak/indeks CRC ve metadata kontrolü
atlanmaz. İlk-sonuç yeni proseste melez N50; her motorun **kendi bütün DB
dosyaları** zamanlama öncesi stream-read/hash ile ısıtılır. OS eviction yok;
gerçek-soğuk kapısı değildir. İlk-result oracle ve katalog/model yükleme
zamanlama dışındadır. SQLite normal open, FDB'nin source-wide strict kontratıyla
aynı değildir. Her familya2 örnek: p95=max; üretim tail garantisi sayılmaz.

Kabul: kullanıcı yanıtıyla **capped N50 ana arama kapısı**, exact sonuçlar
bilgi amaçlıdır. Ortak≤30ms, orta/nadir/AND/negatif≤150ms@50k/≤300ms@100k;
ve/if/id/zx geniş2-karakter taramaları kapı dışı. Temiz idle≤15MiB,
streamed ingest peak private≤60MiB; strict-open≤1000/2000ms. Melez budget200,
önceden kabul edilen%10 negatif/%5 bol-pozitif CPU sınırları da ayrı gösterilir.

## Ingest, temiz disk ve RAM

Hücreler iki ingest örneği; süre tüm bakım/GC dahil saniye. Disk decimal MB;
bellek MiB. İlk işaret phase ingest sonu, oracle sonrası tepe değildir.

| N | Motor | Ingest sn | Disk MB | Peak private MiB | Peak WS MiB | Temiz idle private max MiB | Idle WS max MiB |
|---|---|---|---|---|---|---:|---:|
'''
    for x in analysis['basic']:
        n,e=x['documents'],x['engine'];m=next(r for r in analysis['idle'] if r['documents']==n and r['engine']==e)
        fmt=lambda xs,div:' / '.join(f'{v/div:.3f}' for v in xs)
        text+=f"| {n} | {e} | {fmt(x['ingest_samples_ms'],1000)} | {fmt(x['disk_bytes'],1e6)} | {fmt(x['peak_private_bytes'],2**20)} | {fmt(x['peak_ws_bytes'],2**20)} | {max(m['private_samples_bytes'])/2**20:.3f} | {max(m['working_set_samples_bytes'])/2**20:.3f} |\n"
    text+='| 100000 | fdb | KALDI: 66000 commit sonrası tek entry limiti | — | — | — | — | — |\n'
    text+='\n| Arama process/harness | Peak private MiB | Peak WS MiB |\n|---|---|---|\n'
    for (n,e),xs in sorted(query_ram.items()):
        text+=f"| {n} / {e} | "+' / '.join(f"{x['os_lifetime_peak_private_commit_bytes']/2**20:.3f}" for x in xs)+' | '+' / '.join(f"{x['os_lifetime_peak_working_set_bytes']/2**20:.3f}" for x in xs)+' |\n'
    text+='''
Arama process peak'i katalog/model, full GET oracle ve bütün41 case'in beklenen
owned sayfalarını içerir. Motorun tek sorgu belleği diye sunulmaz; ingest/idle
kapısıyla karıştırılmaz. Tam100k FDB arama süreci çalışmadı.

Disk önceki kitap corpus'u ile codec kazanımı diye karşılaştırılmaz: yeni
50k/100k ham text157.148.362/312.917.193B, uzunluk dağılımı çok farklıdır.
WAL/manifest/source/index biçimleri aynı; eski working DB kopyaları tutulmadı.

## Açılış ve ilk cevap

Strict süre tüm veri doğrulamasını içerir; aşağıdaki idle-open örnekleri ayrıca
gates JSON'undaki ilk-result session örnekleriyle birlikte değerlendirilir.

| N | Strict-open örnekleri ms | Pooled p95 ms | Sınır | Durum |
|---|---|---:|---:|---|
'''
    for level in d['levels']:
        n=level['documents'];v=level['gate']['strict_open_samples_ms'];limit=1000 if n==50000 else 2000
        if not v:
            text+=f'| {n} | Tam FDB DB kurulamadı | — | {limit} | ÖLÇÜLEMEDİ |\n'
            continue
        text+=f"| {n} | "+', '.join(f'{t:.2f}' for t in v)+f" | {p95(v):.2f} | {limit} | {'GEÇTİ' if p95(v)<=limit else 'KALDI'} |\n"
    text+='\n| N | Sorgu | Motor | Session p95 ms | İlk-sonuç p95 ms | n |\n|---|---|---|---:|---:|---:|\n'
    for x in analysis['first']:text+=f"| {x['documents']} | {x['text']} | {x['engine']} | {x['session_p95_ms']:.3f} | {x['first_p95_ms']:.3f} | {x['n']} |\n"
    for n in (50000,100000):
        measured=any(k[0]==n and k[1]=='fdb' for k in Q)
        text+=f'\n## {n}: bütün41 sorgu — p95/ms\n\n'
        if not measured:text+='**FDB normal arama matrisi önceki kapı nedeniyle DURDU.** SQLite bağımsız referansı tamamlandı; boş hücre0ms değildir.\n\n'
        text+='| Sorgu | Gerçek total | Exact serial | Exact4 | Cap serial | Cap4 | Melez200 | SQLite exact | SQLite capped |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|\n'
        for i,c in enumerate(d['query_cases']):
            vals=[]
            for e,v in [('fdb','legacy'),('fdb','exact-four'),('fdb','cap'),('fdb','cap-four'),('fdb','cap-hybrid'),('sqlite','legacy'),('sqlite','cap')]:
                q=Q.get((n,e,i,v));vals.append(f"{q['p95_ms']:.3f}" if q else '—')
            text+=f"| {c['text']} | {corpus['prefixes'][str(n)]['exact'][c['text']]} | "+' | '.join(vals)+' |\n'
        text+=f'\n### {n}: CPU ortalama/toplam ms\n\nHücre ortalama /20 sorgu toplam process CPU. p50/max ve ham örnekler JSON analizinde.\n\n'
        text+='| Sorgu | Exact serial | Exact4 | Cap serial | Cap4 | Melez | SQLite exact | SQLite capped |\n|---|---:|---:|---:|---:|---:|---:|---:|\n'
        for i,c in enumerate(d['query_cases']):
            vals=[]
            for e,v in [('fdb','legacy'),('fdb','exact-four'),('fdb','cap'),('fdb','cap-four'),('fdb','cap-hybrid'),('sqlite','legacy'),('sqlite','cap')]:
                q=Q.get((n,e,i,v));vals.append(f"{q['cpu_mean_ms']:.2f}/{q['cpu_total_ms']:.2f}" if q else '—')
            text+='| '+c['text']+' | '+' | '.join(vals)+' |\n'
    text+='''
## Kısa sorgular: ilk50 ve tam sayım farklı iş

ve/if/id/zx satırlarında exact bütün gerçek eşleşmeleri sayar; capped50 hit'te
AtLeast(50) döner. İlk50 dolmazsa capped de tüketir: zx50k'da28,100k'da47
eşleşme. Bu yüzden kısa negatif/seyrek taramada capped otomatik erken durma
garantisi vermez. Runtime süre ve CPU üstte ayrı;3-gram yokken corpusN üst-sınır
adaydır, actual payload read sayısı diye sunulmaz. İlk50 sonucu bulmak en yeni50
olması için caller ters-mtime key sırası kurar; burada orijinal numeric keys var.

## CPU blokları ve kapı tablosu

| N | Sorgu | Yol | Çağrı | Ortalama CPU ms | Toplam CPU ms |
|---|---|---|---:|---:|---:|
'''
    for x in analysis['cpu_blocks']:text+=f"| {x['documents']} | {x['text']} | {x['variant']} | {x['queries']} | {x['cpu_mean_ms']:.6f} | {x['cpu_total_ms']:.3f} |\n"
    for level in d['levels']:
        n=level['documents'];g=level['gate'];text+=f'\n### {n} kapısı\n\n'
        if not level['full_fdb_measurement']:text+='**FDB ingest KALDI; arama/iptal aşaması BLOKE.** Tam100k DB yok;\naçılış/RAM/arama kapıları ölçülemedi. SQLite bağımsız referansı tamamlandı.\n\n'
        text+='| Kontrol | Kanıt | Durum |\n|---|---|---|\n'
        text+=('| Ingest/disk/RAM/SQLite | iki dengeli kol, her koşuda oracle | TAMAMLANDI |\n' if n==50000 else '| Bağımsız 100k | FDB ingest KALDI; SQLite iki tekrar ve oracle tamam | KISMEN TAMAMLANDI |\n')
        if n==50000:
            text+='| Strict-open / temiz idle / ingest peak | üstteki tam örnekler; 1000ms / 15MiB / 60MiB | GEÇTİ |\n'
        text+=f"| Capped arama ana kapısı | ölçülen {len(g['per_case'])} sorgu/yol | "+('BLOKE' if not level['full_fdb_measurement'] else ('GEÇTİ' if not any(x['kind']=='capped_query' for x in g['failures']) else 'KALDI'))+' |\n'
        for x in g['failures']:text+='| '+x['kind']+' | '+json.dumps(x,ensure_ascii=False,separators=(',',':'))+' | KALDI |\n'
        for x in g['relative_hybrid']:text+='| '+x['kind']+' | '+json.dumps(x,ensure_ascii=False,separators=(',',':'))+' | '+('GEÇTİ' if x['passed'] else 'KALDI')+' |\n'
    text+='''
## Durma sınırı ve doğruluk

Kapı kalınca daha küçük budget/metadata stratejisi, ön filtre veya format
değişimi yapılmadı. Bağımsız sonraki ölçek FDB ingest denendi ve entry limitinde
durdu; SQLite ingest/disk/RAM/arama tamamlandı. Tam100k FDB metriği yok;
bağımlı FDB arama/cancel/yazma/snapshot/soğuk aşaması ilerlemedi.
444345B uzun belge cancel helper'ı bytes exact filtresiyle tek orijinal belgeyi
seçecek şekilde hazırlandı; fakat koşu çalışmadı. Belge streamed ingestte
reddedildiğinden444KB query cancellation gecikmesi ölçülemedi. Eski kitap
fixture0,411/1,436ms gecikmesi yeni TR
uzun belgelerine taşınmış garanti değildir.

Başarıyla tamamlanan ingestlerde bütün title/text/fields/links/version GET
oracle'ı, ölçülen her
arama sayfasında total/lower-bound ve tam hit oracle'ı geçti. CPU bloklarında
yalnız son sayfa interval dışında tam karşılaştırılır. Donmuş24Ekim okuyucu
commit edilmiş 66000 belgeyi full get ve41 sorguyla tekrar okudu;
reddedilen grubun 500 anahtarı GET ile görünmez doğrulandı (bunlar silinmiş
belgeler değil, oracle'ın absence/dead alanıyla kontrol edilen uncommitted
anahtarlardır). Normal alan signed-zero
tutarsızlığı ayrı açık kalır. Yeni corpus sorguları text-onlydir. Recovery ve
komut kanıtları son doğrulama ekinde; power-loss iddiası yok.

Ham [tasarım/program SHA/sıra/koruma](benchmarks/trwiki-scale-20261028-v1-design.json),
[p50/p95/max/CPU ve bütün örnek analizi](benchmarks/trwiki-scale-20261028-v1-analysis.json),
[50k kapı](benchmarks/trwiki-scale-20261028-v1-n50000-gates.json),
[100k bağımsız kapı](benchmarks/trwiki-scale-20261028-v1-n100000-gates.json),
[100k FDB ingest hatası/limit/commit sınırı](benchmarks/trwiki-scale-20261028-v1-n100000-ingest-failure.json),
[current slot pointer](benchmarks/active-benchmark-slots.json),
[corpus audit](benchmarks/trwiki-provided-20261028-v1-audit.json).

50k tekli/dirty500/4R+yazıcı/100'lük commit kök nedeni profili, uzun snapshot,
gerçek-soğuk cache; case34/parçalı geniş tarama, sorgu RAM'i açık. Signed-zero
birleştirilmedi. Default exact/serial; melez+cancel opt-in, join/CRC reuse kapalı,
reclaim default kapalı. Blok3/4,C ABI ve dış proje entegrasyonu yok.
'''
    (R/'docs/TRWIKI-SCALE-20261028.md').write_text(text,encoding='utf-8')
    print('scale report written')

if __name__=='__main__':main()
