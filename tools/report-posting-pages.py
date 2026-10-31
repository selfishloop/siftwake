"""Summarize completed normal-release evidence; no benchmark DB access."""
import json
import math
import re
from collections import defaultdict
from pathlib import Path

R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='posting-pages-50k-20261029-v3'
def p95(xs):return sorted(xs)[math.ceil(.95*len(xs))-1]
def stats(xs):
    values=[x['elapsed_ms'] for x in xs]
    return dict(n=len(xs),p50_ms=sorted(values)[(len(values)-1)//2],p95_ms=p95(values),
        max_ms=max(values),cpu_total_ms=sum(x['cpu_ms'] for x in xs),cpu_mean_ms=sum(x['cpu_ms'] for x in xs)/len(xs))

def main():
    d=json.loads((L/f'{P}-design.json').read_text(encoding='utf-8'))
    assert d.get('authoritative_sources_unchanged') and len(d['completed'])==20
    verification=json.loads((L/f'{P}-verification.json').read_text(encoding='utf-8'))
    audit=json.loads((L/'trwiki-provided-20261028-v1-audit.json').read_text(encoding='utf-8'))
    cases=json.loads((L/'trwiki-provided-20261028-v1-catalog.json').read_text(encoding='utf-8'))['cases']
    mainrows=defaultdict(list);neg=defaultdict(list);blocks=defaultdict(list);idle=defaultdict(list);ram=defaultdict(list)
    for result in d['completed']:
        e=result['engine'];label=result['label']
        for line in (L/result['log']).read_text(encoding='utf-8').splitlines():
            x=json.loads(line)
            if x['phase']=='query_experiment':
                target=mainrows if 'matrix' in label else neg
                target[e,x['case_id'],x['variant']].append(x)
            if x['phase']=='cpu_block':blocks[e,x['text'],x['variant']].append(x)
            if x['phase']=='scale_idle':idle[e].append(x)
            if x['phase']=='query_experiments_finished' and 'matrix' in label:ram[e].append(x['memory'])
    rows={key:stats(xs) for key,xs in mainrows.items()};negative=[];positive=[];query_gates=[]
    for i in d['cases']:
        assert all(rows[e,i,v]['n']==20 for e,v in [('baseline','legacy'),('baseline','cap-hybrid'),('candidate','legacy'),('candidate','cap-hybrid'),('sqlite','legacy'),('sqlite','cap')])
        group=audit['query_groups'][cases[i]['text']]
        limit=30 if group.startswith(('very_common','common_')) else 150
        value=rows['candidate',i,'cap-hybrid']['p95_ms']
        query_gates.append(dict(case_id=i,text=cases[i]['text'],p95_ms=value,limit_ms=limit,passed=value<=limit))
    for i in [39,40]:
        control=p95([x['elapsed_ms'] for x in neg['candidate',i,'cap-four']]);candidate=p95([x['elapsed_ms'] for x in neg['candidate',i,'cap-hybrid']])
        limit=control+2 if control<1 else control*1.1
        negative.append(dict(text=cases[i]['text'],control_p95_ms=control,hybrid_p95_ms=candidate,delta_ms=candidate-control,limit_ms=limit,passed=candidate<=limit))
    for q in [cases[i]['text'] for i in [0,1,2]]:
        a=sum(x['cpu_ms'] for x in blocks['candidate',q,'cap']);b=sum(x['cpu_ms'] for x in blocks['candidate',q,'cap-hybrid'])
        positive.append(dict(text=q,serial_cpu_ms=a,hybrid_cpu_ms=b,delta_pct=(b/a-1)*100,passed=b<=a*1.05))
    tests=(L/'snapshot-10k-20261026-validation-posting-pages-20261029-v3-test.log').read_text(encoding='utf-8')
    passed=sum(map(int,re.findall(r'test result: ok\. (\d+) passed;',tests)))
    query_passed=all(x['passed'] for x in query_gates)
    idle_passed=max(x['memory']['private_bytes'] for x in idle['candidate'])<=15*2**20
    analysis=dict(queries=[dict(engine=e,case_id=i,variant=v,**value) for (e,i,v),value in sorted(rows.items())],
        query_gates=query_gates,negative_gates=negative,positive_cpu_gates=positive,
        default_tests_passed=passed,query_gate_passed=query_passed,
        idle_gate_passed=idle_passed,
        all_gates_passed=query_passed and idle_passed and all(x['passed'] for x in negative+positive),
        query_process_memory=dict(ram),idle_samples=dict(idle),cpu_blocks=[dict(engine=e,text=q,variant=v,
            queries=sum(x['queries'] for x in xs),cpu_total_ms=sum(x['cpu_ms'] for x in xs)) for (e,q,v),xs in sorted(blocks.items())])
    (L/f'{P}-analysis.json').write_text(json.dumps(analysis,ensure_ascii=False,indent=2),encoding='utf-8')
    text=f'''# Posting sayfaları ve paralel test düzeltmesi — 29 Ekim 2026

**Ana arama kapısı: {'GEÇTİ' if query_passed else 'KALDI'}; pozitif melez CPU kapısı: {'GEÇTİ' if all(x['passed'] for x in positive) else 'KALDI'}.** Sonuçlar aynı ilk 50k Türkçe
belgede, aynı temizlenmiş DB dosyalarında eski/yeni okuyucu karşılaştırmasıdır.
Kalıcı biçim, 256 KiB entry sınırı ve varsayılan exact/serial semantiği aynı.
Melez N=50/bütçe200 opt-in; join/CRC reuse ve reclaim default kapalı kalır.

## Uygulama ve güvenlik

`contains()` üyeliği önce sınırlı sayfalardan bir lower-bound bulur. Ardışık
artan adaylarda posting cursor'u oradan ilerler; aday ID geriye giderse veya
sınırlı hint cache'inden düşerse güvenli ikili aramaya döner. Sayfa1024 ID + iki komşu sınır
ID'sini kapsar. Okunan bütün ID'ler range/strict-order denetiminden geçer;
sayfa sınırı duplicate/inversion gizlemez. `DiskIndex::read_at` üzerindeki
mevcut indeks doğrulaması ve kaynak/payload CRC kontrolleri korunur.

LRU sayfaları ve ilerleme konumları en fazla29×(4104B +384B metadata allowance)=130152B kontrollü
alan alır. Legacy toplam posting workspace256KiB sınırında; serial first-page
8MiB ve worker512KiB sınırlarında kalır. İlk sayfada bütün zorunlu cursor'lar
kurulduktan sonra yalnız kalan bütçe kullanılır. Alan yoksa eski küçük-probe
yolu sürer; optional cache yeni mandatory-admission hatası oluşturmaz.
Bu bütçeler allocator veya process RSS ölçümü değildir; OS bellek ayrıca verilir.
Cache sorgu ömründedir, snapshot/source pin mekanizması ve dosyalar aynı.

## Corpus, query, cache, sıra

Kaynak `D:/frunner-corpus/trwiki-100k/corpus.payload-bench` eski/onaylı orijinal
dosyanın ilk50k'sıdır; SHA/audit korunur. `private/` okunmadı, metin kırpılmadı,
belge/corpus/DB kopyası üretilmedi. V1'de hazırlanan sabit C: FDB/SQLite slotları
yeniden kullanıldı. Hazırlama süresi yeni motorun ingest kazancı sayılmaz.
FDB fiziksel kaynak/indeks yerleşimi bütün okuyucular için aynıdır.

Sıcak OS/engine cache: full GET/version oracle + seçenek başına bir warmup.
Gerçek soğuk cache ölçülmedi. İki ayna sıra: eski/SQLite/yeni ve yeni/SQLite/eski.
İlk sıranın eski okuyucu/SQLite kolları, bir önceki denemede tamamlanmış,
oracle geçen aynı frozen program ve DB koşularıdır; yeniden pool edilmedi,
yalnız bir kez kullanıldı. Aradaki geliştirme/derleme aralığı tasarımda açıktır.
Nihai aday ve ters sıra yeni sürüm doğrulandıktan sonra çalıştı. Önceki aday
sonuçları nihai adayın20 örneğine katılmadı. Bu aralık tamamen izole laboratuvar
koşulu değildir; sıra dengesi ve büyük kazançların sınırı buna göre okunmalıdır.
10+10=20 normal-release örneği/sorgu/yol; offset0,N50. Ana yol melez200; exact
serial ayrıca zamanlandı. Serial capped/4 işçi bütün37 için p95 ölçülmedi;
pozitif CPU bloklarında, negatif kapıda ve tanı koşularında ayrıca çalıştırıldı.
Exact-total ve capped sayfalar full hit/payload/version oracle'ıyla aynı.

CPU Windows GetProcessTimes user+kernel, bütün query thread'leri dahil;
tek çağrıda15,625ms kuantizasyon mümkün. Pozitiflerde ayrıca2000 çağrılık CPU
blokları verilir. SQLite FTS5 trigram/detail=none/columnsize=0,WAL/FULL,
cache2MiB,mmap0,tempFILE; aynı payload/fold/key sırası. SQLite fields/links'i
payload olarak tutar; FDB bunlara ek scalar/numeric/link indeksleri de kurar.
SQLite capped COUNT yapmaz fakat ORDER BY için adayları tüketebilir.

V1 ön denemesi cursor bütçesi korunumu son düzenlemesi sırasında durduruldu;
hiçbir V1 süresi bu tablolarla pool edilmedi. Derleme ile çakışan yarım candidate
kolu da dışarıda: [duruş kanıtı](benchmarks/posting-pages-20261029-v1-measurement-stop.json).
Son sürüm tüm doğrulamalar bittikten sonra ölçüldü; DB tekrar üretilmedi.

Ara sayfalama denemesi yedi problemli sorguyu150ms altına indirdi, fakat
çalışmalarıyla342,349ms ve kendisine çıkacağını343,162ms ile KALDI
(ilk sıra10 örnek p95=max). Tanıda bu iki sorguda103359/109753 sayfa okuması
görüldü: küçük LRU'da ikili aramanın tekrar ziyaretleri kalıyordu.
[Ara deneme](benchmarks/posting-pages-50k-20261029-v2-pair0-matrix-candidate-scale-query-experiments.jsonl),
[ara tanı](benchmarks/posting-pages-20261029-v2-long-phrase-profile.jsonl).
İlerleyen cursor bu ölçülen maliyet üzerine seçildi; sorgu kelimesine özel
dal veya daha geniş bellek bütçesi eklenmedi. Son pilot3 örnekte melez
iki sorguda78/105ms altına inerken serial capped kendisine238,555ms gördü;
bu küçük pilot ana kapı değildir, kötü serial sonucu korunur.

## Melez N=50 — bütün37 sorgu

CPU sütunu20 sorgunun ortalama process CPU ms değeridir; p50/max/toplam CPU
ve bütün ham örneklerin adları JSON analiz/tasarım dosyalarındadır.

| Sorgu | Total oracle | Eski p95 ms | Yeni p95 ms | Fark % | Eski CPU ms | Yeni CPU ms | SQLite cap p95 ms | SQLite CPU ms | Kapı |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
'''
    for g in query_gates:
        i=g['case_id'];a=rows['baseline',i,'cap-hybrid'];b=rows['candidate',i,'cap-hybrid'];sql=rows['sqlite',i,'cap']
        text+=f"| {g['text']} | {audit['prefixes']['50000']['exact'][g['text']]} | {a['p95_ms']:.3f} | {b['p95_ms']:.3f} | {(b['p95_ms']/a['p95_ms']-1)*100:+.1f} | {a['cpu_mean_ms']:.3f} | {b['cpu_mean_ms']:.3f} | {sql['p95_ms']:.3f} | {sql['cpu_mean_ms']:.3f} | {'GEÇTİ' if g['passed'] else 'KALDI'} ({g['limit_ms']}ms) |\n"
    text+='\n## Exact/serial — bilgi amaçlı, aynı37 sorgu\n\n| Sorgu | Eski p95 ms | Yeni p95 ms | Eski CPU ms | Yeni CPU ms | SQLite exact p95 ms | SQLite CPU ms |\n|---|---:|---:|---:|---:|---:|---:|\n'
    for i in d['cases']:
        a=rows['baseline',i,'legacy'];b=rows['candidate',i,'legacy'];sql=rows['sqlite',i,'legacy']
        text+=f"| {cases[i]['text']} | {a['p95_ms']:.3f} | {b['p95_ms']:.3f} | {a['cpu_mean_ms']:.3f} | {b['cpu_mean_ms']:.3f} | {sql['p95_ms']:.3f} | {sql['cpu_mean_ms']:.3f} |\n"
    text+='\n## Bellek ve ek kapılar\n\nArama process peak katalog/model ve37 beklenen owned sayfayı da içerir;\nmotorun tek sorgu heap alanı değildir. Idle yeni proseste oracle yüklenmeden ölçülür.\n\n| Okuyucu | Idle private max MiB | Arama process peak private max MiB | Peak WS max MiB |\n|---|---:|---:|---:|\n'
    for e in ['baseline','candidate','sqlite']:
        text+=f"| {e} | {max(x['memory']['private_bytes'] for x in idle[e])/2**20:.3f} | {max(x['os_lifetime_peak_private_commit_bytes'] for x in ram[e])/2**20:.3f} | {max(x['os_lifetime_peak_working_set_bytes'] for x in ram[e])/2**20:.3f} |\n"
    text+='\nNegatif kontrol<1ms ise mutlak +2ms, aksi +%10. Tarihsel qzxkvwnever\n0,3685→0,612ms (+0,2435ms) yeni kuralla GEÇTİ; eski ham rapor değiştirilmedi.\n\n| Negatif sorgu | Yeni cap4 p95 ms | Yeni melez p95 ms | Mutlak fark ms | İzin verilen ms | Kapı |\n|---|---:|---:|---:|---:|---|\n'
    for x in negative:text+=f"| {x['text']} | {x['control_p95_ms']:.4f} | {x['hybrid_p95_ms']:.4f} | {x['delta_ms']:+.4f} | {x['limit_ms']:.4f} | {'GEÇTİ' if x['passed'] else 'KALDI'} |\n"
    text+='\n| Pozitif CPU (2000 çağrı/yol) | Yeni serial toplam ms | Yeni melez toplam ms | Fark % | Fark ms/çağrı | +%5 kapısı |\n|---|---:|---:|---:|---:|---|\n'
    for x in positive:text+=f"| {x['text']} | {x['serial_cpu_ms']:.3f} | {x['hybrid_cpu_ms']:.3f} | {x['delta_pct']:+.2f} | {(x['hybrid_cpu_ms']-x['serial_cpu_ms'])/2000:+.5f} | {'GEÇTİ' if x['passed'] else 'KALDI'} |\n"
    text+='\nBüyük sorgusunda +%6,06 / +0,09375ms CPU sınırı KALDI. Bu fark otomatik\nölçüm gürültüsü sayılmadı veya yeni toleransla kapatılmadı. Melez opt-in kalır.\n\n## Aynı50k üzerinde maliyet tanısı\n\nBir warmup+bir diagnostic örnek/yol; normal p95 tablosuna karışmaz.\nThread span toplamları örtüşür, CPU/wall yüzdesi olarak toplanmaz.\nAşağıda serial capped; iki okuyucu aynı adayları doğrular.\n\n| Sorgu | Eski4B probe | Yeni4B probe | Yeni sayfa okuması | Eski read_at isteği | Yeni read_at isteği | Aday eski/yeni |\n|---|---:|---:|---:|---:|---:|---|\n'
    profile={}
    for result in verification['profiles']:
        for line in (L/result['log']).read_text(encoding='utf-8').splitlines():
            x=json.loads(line)
            if x['phase']=='query_experiment' and x['variant']=='cap':profile[result['engine'],x['case_id']]=x
    for i in [15,22,23,24,28,30,32,33,34]:
        a=profile['baseline',i]['counters'];b=profile['candidate',i]['counters']
        count=lambda c,k:c.get(k,{}).get('calls',0)
        text+=f"| {cases[i]['text']} | {count(a,'query.posting_probe')} | {count(b,'query.posting_probe')} | {count(b,'query.posting_page_read')} | {count(a,'disk.read_at')} | {count(b,'disk.read_at')} | {count(a,'first.candidate_verify')}/{count(b,'first.candidate_verify')} |\n"
    text+='\nTarihsel380033 probe66k/12 segment tanısına aitti; burada50k/3 segment\nkontrolünde yanıklarda556367 probe vardı. Bunlar aynıN gibi karşılaştırılmadı.\nSayfa + ilerleyen konum, sayfa-only denemedeki109753 tekrar okumasını da azaltır.\nPayload okuma/CRC/metin doğrulama korunur; aday azaltan ön filtre uygulanmadı.\n'
    text+=f'''
## Kurtarma, uyumluluk, paralel test

Kesilme/bozulma/ID-sıra ve normal kurtarma testleri, fmt/clippy ve feature
kontrolleri geçti. Nihai varsayılan-paralel all-targets paketinde **{passed} test**
geçti. [Test kilit çevrimi ve ikinci admission-testi yarışı](PARALLEL-TEST-DEADLOCK-20261029.md)
kontrollü üretildi; test-only path pause ve doğru dört-permit tutuşuyla çözüldü.
Seri moda kaçılarak kapatılmadı; production bütçesi/kilitleri değiştirilmedi.

Son DB'de Pending açılış process kill → strict reopen/full GET oracle ve
donmuş24Ekim okuyucuyla50k GET +41 exact sorgu geçti. Kaynak hash'leri aynı;
power-loss iddiası yok. `.fdb/.ready` otorite, `.idx` türetilmiş cache olarak kalır.
256KiB giriş sınırı ile format/manifest/payload kaynak SHA'ları eskiyle aynı.

[Ham tasarım, program SHA ve sıra](benchmarks/{P}-design.json),
[bütün istatistikler/kapılar](benchmarks/{P}-analysis.json),
[CRC/limit/kill/eski okuyucu kanıtı](benchmarks/{P}-verification.json),
[fmt/clippy/test/feature komutları](benchmarks/snapshot-10k-20261026-validation-posting-pages-20261029-v3.json).

100k kullanıcı tarafından belge başına200KB ile hazırlanacak yeni corpus'u
bekliyor; eski444KB belgeye owned fallback uygulanmadı, entry limiti artırılmadı.
Yeni dosyada audit/oracle tekrar kurulmadan100k ingest başlamayacak.
Yazma/bakım profili, dirty500,4R+yazıcı,100'lük commit/RAM, uzun snapshot ve
gerçek soğuk cache açık. Case34/parçalı geniş tarama ve signed-zero normal
light/owned farkı izleniyor. Ön filtre, format değişikliği, Blok3/4, C ABI veya
dış proje entegrasyonu yapılmadı. [Güncel açık işler](OPEN-WORK.md).
'''
    for a,b in {
        'Sayfa1024':'Sayfa 1024','fazla29':'fazla 29','workspace256KiB':'workspace 256 KiB',
        'worker512KiB':'worker 512 KiB','ilk50k':'ilk 50k','adayın20':'adayın 20','offset0,N50':'offset 0,N=50',
        'melez200':'melez 200','bütün37':'bütün 37','çağrıda15':'çağrıda 15','ayrıca2000':'ayrıca 2000',
        'cache2MiB,mmap0':'cache 2 MiB,mmap 0','sorguyu150ms':'sorguyu 150 ms','çalışmalarıyla342':'çalışmalarıyla 342',
        'çıkacağını343':'çıkacağını 343','sıra10':'sıra 10','sorguda103359':'sorguda 103359',
        'pilot3':'pilot 3','sorguda78':'sorguda 78','kendisine238':'kendisine 238','sütunu20':'sütunu 20',
        'aynı37':'aynı 37','aynı50k':'aynı 50k','Eski4B':'Eski 4B','Yeni4B':'Yeni 4B',
        'Tarihsel380033':'Tarihsel 380033','probe66k':'probe 66k','burada50k':'burada 50k',
        'yanıklarda556367':'yanıklarda 556367','denemedeki109753':'denemedeki 109753','donmuş24Ekim':'donmuş 24 Ekim',
        'okuyucuyla50k':'okuyucuyla 50k','+41 exact':'+41 exact','başına200KB':'başına 200KB',
        'eski444KB':'eski 444KB','kurulmadan100k':'kurulmadan 100k'
    }.items():text=text.replace(a,b)
    (R/'docs/POSTING-PAGES-50K-20261029.md').write_text(text,encoding='utf-8')
    print(json.dumps(dict(query_gate_passed=query_passed,failed_queries=[x for x in query_gates if not x['passed']],negative=negative,positive=positive),ensure_ascii=False))

if __name__=='__main__':main()
