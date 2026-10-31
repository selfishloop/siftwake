"""Summarize immutable cap200k measurement logs; never opens a database."""
import json
import math
import re
from collections import defaultdict
from pathlib import Path

R = Path(__file__).resolve().parent.parent
L = R / 'docs/benchmarks'
P = 'cap200k-scale-20261029-v1'


def read(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))


def p95(xs):
    return sorted(xs)[math.ceil(.95 * len(xs)) - 1]


def summary(xs):
    return dict(n=len(xs),p50_ms=sorted(xs)[(len(xs)-1)//2],p95_ms=p95(xs),max_ms=max(xs))


def samples(xs,div=1,digits=3):
    return ' / '.join(f'{x/div:.{digits}f}' for x in xs)


def main():
    d=read(L/f'{P}-design.json')
    assert d.get('protected_unchanged') and d.get('provided_source_unchanged'),'Incomplete evidence'
    audit=read(L/'trwiki-cap200k-20261029-v3-audit.json')
    groups={name:defaultdict(list) for name in ['seed','idle','open','first','query','blocks','query_ram','cancel']}
    all_oracles=[]
    for result in d['completed']:
        assert result['exit_code']==0
        n=int(re.search(r'n(50000|100000)',result['label']).group(1));e=result['engine']
        for line in (L/result['log']).read_text(encoding='utf-8').splitlines():
            x=json.loads(line);phase=x['phase']
            if 'oracle' in x:all_oracles.append(x['oracle'])
            if phase=='scale_ingested':groups['seed'][n,e].append(x)
            if phase=='scale_idle':groups['idle'][n,e].append(x)
            if phase=='opened' and result['mode']=='scale-idle':groups['open'][n,e].append(x['open_ms'])
            if phase=='first_result':groups['first'][n,e,x['text']].append(x)
            if phase=='query_experiment':
                key=(n,e,x['case_id'],x['variant'],'negative' in result['label'])
                groups['query'][key].append(x)
            if phase=='cpu_block':groups['blocks'][n,x['text'],x['variant']].append(x)
            if phase=='query_experiments_finished':groups['query_ram'][n,e].append(x['memory'])
            if phase=='long_cancel':groups['cancel'][x['record_id'],x['variant']].append(x)
    assert all(all_oracles)
    for level in d['levels']:
        n=level['documents']
        for e,variants in [('fdb',d['variants']),('sqlite',['legacy','cap'])]:
            if e=='fdb' and not level['full_fdb_measurement']:continue
            assert len(groups['seed'][n,e])==2 and len(groups['idle'][n,e])==4
            for i in range(41):
                for v in variants:assert len(groups['query'][n,e,i,v,False])==20,(n,e,i,v)
        if level['full_fdb_measurement']:
            for q in d['first_queries']:
                for e in ['fdb','sqlite']:assert len(groups['first'][n,e,q])==2
            for i in [39,40]:
                for v in ['cap-four','cap-hybrid']:assert len(groups['query'][n,'fdb',i,v,True])==20
            for q in [c['text'] for c in d['query_cases'][:3]]:
                for v in ['cap','cap-four','cap-hybrid']:assert sum(x['queries'] for x in groups['blocks'][n,q,v])==2000
    if d['gate_failed_at'] is None:
        assert len(groups['cancel'])==8
        assert all(len(xs)==40 for xs in groups['cancel'].values())
    a=dict(queries=[],ingest=[],idle=[],opens=[],first=[],cpu_blocks=[],query_ram=[],cancel=[],
        levels=d['levels'],gate_failed_at=d['gate_failed_at'],all_recorded_oracles=True,
        recovery=d['recovery'],provided_source_unchanged=True,protected_unchanged=True)
    if 'interruption' in d:a['interruption']=d['interruption']
    for (n,e,i,v,negative),xs in sorted(groups['query'].items()):
        a['queries'].append(dict(documents=n,engine=e,case_id=i,text=d['query_cases'][i]['text'],variant=v,
            negative_gate_run=negative,**summary([x['elapsed_ms'] for x in xs]),
            cpu_total_ms=sum(x['cpu_ms'] for x in xs),cpu_mean_ms=sum(x['cpu_ms'] for x in xs)/len(xs),
            returned=sorted({x['returned'] for x in xs}),totals=xs[0]['total']))
    for (n,e),xs in sorted(groups['seed'].items()):
        a['ingest'].append(dict(documents=n,engine=e,samples=xs))
    for (n,e),xs in sorted(groups['idle'].items()):a['idle'].append(dict(documents=n,engine=e,samples=xs))
    for (n,e),xs in sorted(groups['open'].items()):a['opens'].append(dict(documents=n,engine=e,values_ms=xs,**summary(xs)))
    for (n,e,q),xs in sorted(groups['first'].items()):a['first'].append(dict(documents=n,engine=e,text=q,samples=xs))
    for (n,q,v),xs in sorted(groups['blocks'].items()):
        calls=sum(x['queries'] for x in xs);cpu=sum(x['cpu_ms'] for x in xs)
        a['cpu_blocks'].append(dict(documents=n,text=q,variant=v,queries=calls,cpu_total_ms=cpu,cpu_ms_per_call=cpu/calls))
    for (n,e),xs in sorted(groups['query_ram'].items()):a['query_ram'].append(dict(documents=n,engine=e,samples=xs))
    for (ident,v),xs in sorted(groups['cancel'].items()):
        cancelled=[x['cancel_to_query_threads_joined_ms'] for x in xs if x['status']=='cancelled']
        a['cancel'].append(dict(record_id=ident,text_bytes=xs[0]['text_bytes'],variant=v,attempts=len(xs),
            cancelled=len(cancelled),races=sum(x['status']!='cancelled' for x in xs),
            latency=summary(cancelled) if cancelled else None,
            candidates_at_request=sorted({x['candidates_at_request'] for x in xs})))
    (L/f'{P}-analysis.json').write_text(json.dumps(a,ensure_ascii=False,indent=2),encoding='utf-8')
    Q={(x['documents'],x['engine'],x['case_id'],x['variant']):x for x in a['queries'] if not x['negative_gate_run']}
    lines=['# Türkçe cap200k: 50k/100k yeni taban — 29 Ekim 2026','',
        '**'+('GEÇTİ.' if d['gate_failed_at'] is None else f"KALDI: ilk kapı {d['gate_failed_at']} belge aşamasında.")+'** '+
        ('Sonraki yazma/snapshot/soğuk-cache aşamaları ayrı onay bekler.' if d['gate_failed_at'] is None else 'Bağımlı FDB arama/iptal ilerlemesi durduruldu; bağımsız ingest/disk/RAM/SQLite tamamlandı.'),'',
        'Kullanıcının onayladığı payload SHA256: `'+d['provided_source_sha256']+'`. Kaynak '+
        '`D:/frunner-corpus/trwiki-100k-cap200k/corpus.payload-bench` yalnız okundu; `private/` okunmadı. '+
        'İlk8510408E… sürümü DB ölçümünde kullanılmadı. [Tam audit](TRWIKI-CAP200K-AUDIT-20261029.md): '+
        '100000 benzersiz gövde; eskiyle aynı sıra/anahtarlar ve41 sorgu. Yalnız45192/66498 metni kırpılmış; '+
        '45192 ilk50k içindedir. Bu rapor yeni bir tabandır; eski kitap/TR50k sonuçlarına birebir hız oranı kurulmaz.','',
        'Motor/format değişmedi. FDBLOG04/MAN03/SEG05/MET04,256KiB entry sınırı, kaynak+indeks CRC ve sıralı-ID '+
        'kontrolleri aynı. Varsayılan exact/serial; capped/melez200/cancel opt-in. Join/CRC reuse kapalı. '+
        'Ön filtre, Block3/4, C ABI veya proje entegrasyonu yapılmadı.','',
        '## Yöntem ve ölçüm sınırları','',
        'Her N için FDB/SQLite ardından SQLite/FDB; yalnız ownership kontrolünden geçen C: sabit '+
        '`data/benchmark-work/fdb` ve `sqlite` slotları resetlendi. Yeni corpus/DB kopyası yok. '+
        'Akışlı ingest500 işlem/commit; owned/Vec denenmedi. Kaynak title/text/fields/links/version korunur. '+
        'Akışlı WAL katmanı bu ölçümde açık, revizyon geri kazanımı kapalıdır. '+
        'FDB toplamına flush/merge ve son idle cleanup dahildir. Bu500lü ingest, sonraki onaylı100lü yazma profilinin yerine geçmez.','',
        'SQLite FTS5 trigram/detail=none/columnsize=0; WAL/FULL,cache2MiB,mmap0,tempFILE. Aynı numeric key '+
        'sırası ve normalize title/text substring-AND oracle. FDB ayrıca fields/link indekslerini kurar; SQLite bunları '+
        'payload içinde saklar. SQLite capped COUNT yapmaz, fakat key sıralaması için adayları tüketebilir.','',
        'Sıcak matriste full GET/version oracle+her yol bir warmup; sorgu/seçenek sırası döndürülür. '+
        'Her case/yol10+10=20 normal release örneği. Ana kapı yalnız melez200,N50,offset0; exact serial/exact4 '+
        've capped serial/4 karşılaştırmadır. Ortak≤30ms; diğer≤150ms@50k/≤300ms@100k. ve/if/id/zx kapı dışı. '+
        'Windows process CPU user+kernel bütün query işçilerini kapsar; tek çağrı0/15,625ms kuantize olabilir; '+
        'pozitif CPU ayrıca2000 çağrı/yol bloklarıyla ölçülür. Arka plan uygulamaları/OS tamamen izole edilmedi.','',
        'Bellek process private commit ve working set olarak ayrıdır. Ingest OS lifetime peak, katalog/model '+
        'descriptor harness dahil; yalnız motor heap’i değildir.10ms örneklemeye ek OS peak sayaçları kullanılır. '+
        'Temiz idle, oracle/katalog yüklenmeden yeni proseste2sn sonra. İlk sonuç yeni proseste; her kolun kendi bütün DB '+
        'dosyaları zamanlama öncesi stream-read/hash ile ısıtılır. Gerçek soğuk-cache ölçümü değildir. '+
        'Normal strict-open hiçbir doğrulama atlamaz; SQLite open aynı source-wide strict kontratı değildir.','',
        '## Ingest, disk ve bellek','',
        'İki örnek / ile ayrıldı. Disk decimal MB, bellek MiB. Ingest sonu tepe, sonradan yüklenen full oracle tepesinden ayrıdır.','',
        '| N | Motor | Ingest sn | Commit toplam sn | Bakım sn | Idle cleanup sn | Disk MB | Peak private MiB | Peak WS MiB | Idle private max MiB | Idle WS max MiB |',
        '|---|---|---|---|---|---|---|---|---|---:|---:|']
    if 'interruption' in d:
        x=d['interruption']
        lines[-2:-2]=['Ölçüm kesintisi: ikinci100k SQLite matrisi sırasında driver/worker süreçleri kapanmış bulundu; '+
            'kesin sonlandırma nedeni ölçülmedi. Tamamlanmış ingest/idle/ilk çift tekrarlanmadı. '+
            f"[{x['partial_measured_queries']} çağrılık yarım log](benchmarks/{x['partial_log']}) korundu; "+
            f"en büyük kayıtlı çağrı {x['partial_max_query_ms']:.3f}ms. Bu eksik kol pooled p95'e katılmadı; "+
            'ikinci SQLite/FDB arama çifti baştan tamamlandı. Kaynak/binary/model hashları yeniden doğrulandı. '+
            'Araya giren oturum farkı ölçüm sınırıdır.','']
    for x in a['ingest']:
        n,e=x['documents'],x['engine'];xs=x['samples'];ys=groups['idle'][n,e]
        vals=[samples([r['ingest_including_maintenance_ms'] for r in xs],1000),samples([r['commit_total_ms'] for r in xs],1000),
            samples([r['maintenance_ms'] for r in xs],1000),samples([(r['idle_cleanup'] or {}).get('elapsed_ms',0) for r in xs],1000),
            samples([r['disk']['retained_bytes'] for r in xs],1e6),samples([r['memory']['os_lifetime_peak_private_commit_bytes'] for r in xs],2**20),
            samples([r['memory']['os_lifetime_peak_working_set_bytes'] for r in xs],2**20),
            f"{max(r['memory']['private_bytes'] for r in ys)/2**20:.3f}",f"{max(r['memory']['working_set_bytes'] for r in ys)/2**20:.3f}"]
        lines.append(f'| {n} | {e} | '+' | '.join(vals)+' |')
    lines+=['','| N | Motor | Source MB | Index MB |500lü ingest commit p50/p95/max ms (pooled) |','|---|---|---|---|---|']
    for x in a['ingest']:
        xs=x['samples'];ts=[v for r in xs for v in r['commits_ms']];st=summary(ts)
        src=samples([r['disk']['all_source_bytes'] for r in xs],1e6) if x['engine']=='fdb' else '—'
        idx=samples([r['disk']['all_index_bytes'] for r in xs],1e6) if x['engine']=='fdb' else '—'
        lines.append(f"| {x['documents']} | {x['engine']} | {src} | {idx} | {st['p50_ms']:.2f}/{st['p95_ms']:.2f}/{st['max_ms']:.2f} |")
    lines+=['','SQLite aynı dosyada source/index tuttuğundan burada ayrıştırılmadı; retained üstte tamdır.',
        '500lü commit tail’i tekli/100lü commit kapısı değildir; henüz kök-neden optimizasyonu yapılmadı.','',
        '| N | Arama süreci | OS peak private MiB | OS peak WS MiB |','|---|---|---|---|']
    for x in a['query_ram']:
        lines.append(f"| {x['documents']} | {x['engine']} | "+samples([r['os_lifetime_peak_private_commit_bytes'] for r in x['samples']],2**20)+' | '+samples([r['os_lifetime_peak_working_set_bytes'] for r in x['samples']],2**20)+' |')
    lines+=['','Bu son arama tepesinde full GET oracle, model/katalog ve41 case’in beklenen owned sayfaları da vardır; tek sorgu motor RAM’i diye sunulmaz.','',
        '## Açılış ve ilk sonuç','',
        '| N | Motor | Temiz idle-open örnekleri ms | p95 ms |','|---|---|---|---:|']
    for x in a['opens']:lines.append(f"| {x['documents']} | {x['engine']} | {samples(x['values_ms'])} | {x['p95_ms']:.3f} |")
    lines+=['','İlk sonuç: session=normal open, first=oturum başlangıcından hit dönüşüne toplam. Her familyada2 örnek; p95=max, üretim tail garantisi değildir. Tam doğrulama FDB strict session sonunda tamamdır.','',
        '| N | Sorgu | Motor | Session p95 ms | İlk sonuç p95 ms | Session sonrası fark p95 ms |','|---|---|---|---:|---:|---:|']
    for x in a['first']:
        xs=x['samples'];lines.append(f"| {x['documents']} | {x['text']} | {x['engine']} | {p95([r['session_ms'] for r in xs]):.3f} | {p95([r['first_ms'] for r in xs]):.3f} | {p95([r['first_ms']-r['session_ms'] for r in xs]):.3f} |")
    order=[('fdb','legacy'),('fdb','exact-four'),('fdb','cap'),('fdb','cap-four'),('fdb','cap-hybrid'),('sqlite','legacy'),('sqlite','cap')]
    for n in [50000,100000]:
        lines+=['',f'## {n}:41 sorgunun tamamı','',
            '| Sorgu | Exact total | Exact seri | Exact4 | Cap seri | Cap4 | Melez200 | SQLite exact | SQLite cap |',
            '|---|---:|---:|---:|---:|---:|---:|---:|---:|']
        for i,c in enumerate(d['query_cases']):
            vals=[f"{Q[n,e,i,v]['p95_ms']:.3f}" if (n,e,i,v) in Q else '—' for e,v in order]
            lines.append(f"| {c['text']} | {audit['prefixes'][str(n)]['exact'][c['text']]} | "+' | '.join(vals)+' |')
        lines+=['','Süreler pooled p95 ms. — önceki kapı nedeniyle çalıştırılmadı,0ms değildir.','',
            f'### {n}:CPU ortalama/toplam ms','',
            '| Sorgu | Exact seri | Exact4 | Cap seri | Cap4 | Melez200 | SQLite exact | SQLite cap |',
            '|---|---:|---:|---:|---:|---:|---:|---:|']
        for i,c in enumerate(d['query_cases']):
            vals=[]
            for e,v in order:
                q=Q.get((n,e,i,v));vals.append(f"{q['cpu_mean_ms']:.3f}/{q['cpu_total_ms']:.3f}" if q else '—')
            lines.append('| '+c['text']+' | '+' | '.join(vals)+' |')
    lines+=['','### Gösterilen sonuç sayısının ölçekle ilişkisi','',
        'Belge sayısı2×. N50 bol sorgularda gösterilen hit sayısını50’de sabitler; bu bir doğrusal süre kapısı değildir. '+
        'En az bir ölçekte50’den az hit bulunan sorgular aşağıdadır; exact toplamlar üstte ayrı verilir.','',
        '| Sorgu |50k gösterilen |100k gösterilen | Gösterilen oranı |','|---|---:|---:|---:|']
    for c in d['query_cases']:
        q=c['text'];n50=min(audit['prefixes']['50000']['exact'][q],50);n100=min(audit['prefixes']['100000']['exact'][q],50)
        if min(n50,n100)==50:continue
        ratio=f'{n100/n50:.3f}×' if n50 else ('0/0' if not n100 else 'ilk ölçekte0')
        lines.append(f'| {q} | {n50} | {n100} | {ratio} |')
    lines+=['','## Kısa sorgular ve CPU kapıları','',
        've/if/id/zx iki karakterlidir; trigram reddi kullanılamaz. Exact bütün gerçek eşleşmeleri sayar. '+
        'Capped50 hit dolunca AtLeast(50), dolmazsa Exact(total) döner; sıralı ilk50 sonuçlar exact ile aynıdır. '+
        'Zx50k/100k total audit ve üstteki tabloda verilir;50 dolmazsa melez de bütün adayları tüketir. '+
        'Capped ve tam tarama süre/CPU üstte ayrı gösterildi. Bu genel tarama maliyeti ana kapı dışıdır.','',
        '| N | Pozitif sorgu | Yol | Çağrı | CPU ms/çağrı | Toplam CPU ms |','|---|---|---|---:|---:|---:|']
    for x in a['cpu_blocks']:lines.append(f"| {x['documents']} | {x['text']} | {x['variant']} | {x['queries']} | {x['cpu_ms_per_call']:.6f} | {x['cpu_total_ms']:.3f} |")
    lines+=['','Negatif:4 işçi p95<1ms ise melez mutlak artış≤2ms, aksi+%10. Pozitif:serial CPU<10ms/çağrı ise '+
        'mutlak artış≤0,2ms, aksi+%5. Önceki raw posting raporu değiştirilmedi; yeni kabul ayrı JSON’dadır.','',
        '## Kapı tablosu','', '| N | Kapı | Ölçüm | Durum |','|---|---|---|---|']
    for lev in d['levels']:
        n=lev['documents'];g=lev['gate'];full=lev['full_fdb_measurement']
        for label,key,limit,div in [('Strict-open','strict_open_samples_ms',1000 if n==50000 else 2000,1),('Idle private','idle_private_samples',15,2**20),('Ingest peak private','ingest_peak_private_samples',60,2**20)]:
            vals=g[key];v=(p95(vals) if key=='strict_open_samples_ms' else max(vals))/div
            lines.append(f"| {n} | {label} | {v:.3f}≤{limit} | {'GEÇTİ' if v<=limit else 'KALDI'} |")
        bad=[x for x in g['per_case'] if not x['passed']]
        lines.append(f"| {n} | Melez 37 ana sorgu | "+(f'{37-len(bad)}/37' if full else 'önceki kapı nedeniyle çalışmadı')+f" | {('GEÇTİ' if not bad else 'KALDI') if full else 'BLOKE'} |")
        for item in g['relative']:
            lines.append(f"| {n} | {item['kind']}:{item['text']} | "+json.dumps(item,ensure_ascii=False,separators=(',',':'))+f" | {'GEÇTİ' if item['passed'] else 'KALDI'} |")
        for item in g['failures']:lines.append(f"| {n} | Engel | "+json.dumps(item,ensure_ascii=False,separators=(',',':'))+' | KALDI |')
        lines.append(f"| {n} | Full payload/version + bütün41 sorgu oracle, Pending kill/reopen, eski okuyucu | aşağıdaki kanıt | GEÇTİ |")
    lines+=['','## Kesinti, oracle ve uyumluluk','']
    for n,r in d['recovery'].items():
        old=r['old_reader'];assert old['oracle'] and old['authoritative_sources_unchanged'] and r['source_hashes_unchanged']
        lines.append(f"- {n}: Pending oturum dönüşü tanıklandıktan sonra child process kill; exit={r['exit_code']}, yeniden strict-open ve full GET/version oracle geçti. Eski dondurulmuş okuyucu {old['full_payload_version_gets']} tam GET ve41 exact sorguda geçti; source SHA aynı.")
    lines+=['','Bu kill, açık/okuma sırasında proses sonlandırmadır; churn/flush/rewrite içine yeni hata enjeksiyonu veya '+
        'güç kesintisi iddiası değildir. Bu tur kaynak yazma/kurtarma algoritması değiştirilmedi.0.1 fixture ve eski10k '+
        'corpus SHA’ları korundu. Yeni corpus baştan sona aynı SHA’da kaldı. Oracle sayfa/total/sequence ve owned '+
        'payload/version alanlarını doğrular; signed-zero eski açık konu ayrı kalır.','',
        '## 200 KB belgelerde iptal','']
    if not a['cancel']:lines.append('Önceki kapı nedeniyle DURDU; ölçülmemiş iptal gecikmesine sonuç atanmadı.')
    else:
        lines+=['Exact bytes alanı uzun belgeyi tekil seçer;zx iki karakteri payload doğrulamasına ulaştırır. '+
            'Cancel isteği aday doğrulama başladı tanığından sonra verilir.40 deneme/belge/yol, '+
            'cancel→bütün query thread’lerinin join’i ölçülür. Sıcak doğrulanmış kaynak; senkron OS okuması zorla kesilmez.','',
            '| ID | Text B | Yol | Deneme | İptal | Bitiş yarışı | p50/p95/max ms |','|---|---:|---|---:|---:|---:|---|']
        for x in a['cancel']:
            t=x['latency'];lat='/'.join(f'{t[k]:.3f}' for k in ['p50_ms','p95_ms','max_ms']) if t else '—'
            lines.append(f"| {x['record_id']} | {x['text_bytes']} | {x['variant']} | {x['attempts']} | {x['cancelled']} | {x['races']} | {lat} |")
        lines+=['','İptal sonrası bütün sonuç yarışı/oracle ve check_integrity geçti. Tanık aday başlangıcıdır; '+
            'metin taramasının rastgele bir noktasında iptal veya cold I/O/üretim SLA garantisi değildir.']
    lines+=['','## Ham kanıt ve açık kalanlar','',
        f'- [Tasarım/komutlar/hashlar/bütün loglar](benchmarks/{P}-design.json).',
        f'- [p50/p95/max,CPU,örnekler ve bütün kapılar](benchmarks/{P}-analysis.json).',
        '- [Audit/katalog](benchmarks/trwiki-cap200k-20261029-v3-audit.json); [kaynak onayı](benchmarks/cap200k-source-acceptance-20261029.json).',
        '- [Fmt/clippy/default-parallel test ve feature/SQLite kontrolleri](benchmarks/snapshot-10k-20261026-validation-cap200k-20261029-v1.json).','',
        'Sonraki50k yazma profili (tekli,dirty500,4R+yazıcı,100lü commit p95/RAM nedeni),uzun snapshot, '+
        'gerçek soğuk cache ayrı onay bekler. Reclaim default kapalı; bakım/yazma, sorgu RAM/disk ve4R throughput '+
        'eski açık sınırları [OPEN-WORK](OPEN-WORK.md) içinde kalır. Eski case34 (ishmael /130/1),the zx parçalı okuma, '+
        'signed-zero ve eski4R uç sorgular ölçüm olmadan kapatılmadı.']
    (R/'docs/TRWIKI-CAP200K-SCALE-20261029.md').write_text('\n'.join(lines)+'\n',encoding='utf-8')
    print(json.dumps(dict(gate_failed_at=d['gate_failed_at'],queries=len(a['queries']),cancel_groups=len(a['cancel']),report='docs/TRWIKI-CAP200K-SCALE-20261029.md')))


if __name__=='__main__':main()
