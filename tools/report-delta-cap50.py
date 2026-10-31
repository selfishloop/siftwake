import json,math
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='delta-cap50-20261030'
def pct(x):return sorted(x)[math.ceil(len(x)*.95)-1]
def main():
    d=json.loads((L/f'{P}-design.json').read_text(encoding='utf-8'));assert d['stage']=='completed';groups={}
    for x in d['completed']:
        if not x['label'].startswith(('four-','all41-')):continue
        matrix,threads,*_=x['label'].split('-');rows=list(map(json.loads,(L/x['log']).read_text(encoding='utf-8').splitlines()));v=next(v for v in rows if v['phase']=='snapshot_idle_probe');assert v['oracle']
        for t in v['results']:
            for q in t['records']:groups.setdefault((matrix,threads,q['case_id'],x['role']),[]).append(q['engine_ms'])
    cases=json.loads((L/'trwiki-cap200k-20261029-v3-catalog.json').read_text(encoding='utf-8'))['cases'];out=[]
    for matrix in ['four','all41']:
        for threads in ['1R','4R']:
            for i in ([0,3,39,40] if matrix=='four' else range(41)):
                a=groups[matrix,threads,i,'control'];b=groups[matrix,threads,i,'candidate'];out.append(dict(matrix=matrix,threads=threads,case_id=i,text=cases[i]['text'],control_p95=pct(a),candidate_p95=pct(b),difference_ms=pct(b)-pct(a),control_n=len(a),candidate_n=len(b)))
    (L/f'{P}-summary.json').write_text(json.dumps(out,ensure_ascii=False,indent=2),encoding='utf-8')
    lines=['# Capped first-page delta — 30 Ekim 2026','',
    'Adım 1 kullanıcı tarafından kabul edildi. Yazmasız maske sonuçları için neden atfı kanıtlanmadı; yeni maske kabul koşusu yapılmadı. Bu rapor yalnız ayrı delta düzeltmesini ölçer. A gerçek eşzamanlı yük kapısı ayrıca değerlendirilecek.', '',
    '## Değişiklik ve doğruluk', '',
    '`first_delta` açık `Option<usize>` cap alır. First-page offset0 çağrısı Some(limit), experimental exact None gönderir. Exact/serial delta yolu değişmez. Streamed primary, owned docs boşken light BTreeMap sırasıyla ilk N eşleşmede durur. Owned/mixed yol mevcut delta sonuçlarını tam hesaplar, public Key sırasıyla sıralar, sonra keser; owned slot sırası kullanılmaz. Cancellable shadow BTreeSet yolunda aynı scalar semantiği korunarak N eşleşmede durulur; son token kontrolü kalır. Full shadow/mask, revision/pin/sequence, kaynak/indeks CRC ve biçimler değişmez. Global min-heap artık kullanmadığı delta kuyruğunu hesaplamaz.', '',
    'Yeni oracle testi source/delta N sınırları, reverse ingest sırası, update/delete, cap dışında eski source revizyonlarının görünmemesi, eski snapshot/sequence ve cancel durumlarını exact referansla karşılaştırır. Signed-zero testine hem normal hem cancellable first-page eklendi: owned/light mevcut farkı korunur, birleştirilmedi. Seçilen WAL payload tek bayt bozulunca normal/cancellable capped yol CRC hatası vermelidir; ayrı test eklendi. Önceden var olan tam test paketi ayrıca çalıştırılır.', '',
    '## Yöntem ve sınırlar', '',
    '881756DA… cap200k corpus ilk50k; source/private/ değişmedi/okunmadı. Sabit FDB slotu100k mask-only olduğu için yalnız bu owner slot reset ile50k kuruldu; DB kopyası ve yeni corpus yok. Önce500 tekli update; otomatik flush nedeniyle ölçüm açılışında244 aktif delta anahtarı kaldı. Bu500 pending-delta dirty-open testi değildir. Aynı DB/modelde kontrol/aday/aday/kontrol, önce0/3/39/40 1R+4R, sonra41-case 1R+4R. Warm OS cache, eviction yok; her sorgu payload/model oracle. N50 hybrid budget200. Her kolda5 round/reader; pooled 1R10,4R40 örnek/case/kol. Esas kontrol kabul edilmiş maske + eski uncapped delta ikilisi; aday yalnız delta algoritması farklı. CPU ayrı ölçülmedi. Negatif sorgular tam delta tarar; erken bitiş kazancı beklenmez. Kötü p95 değerleri de tabloda.', '',
    '[Ham tasarım, program SHA/exit/lifecycle](benchmarks/'+P+'-design.json) · [Bütün örneklerden sonuç özeti](benchmarks/'+P+'-summary.json). Process RAM model/oracle/katalog içerir; motor-only D sayacı yerine sunulmaz. SQLite referansı A koşusunda aynı corpus/FTS5 ayarlarıyla tekrar ölçülecek; bu delta-only mikro A/B SQLite ölçümü değildir.', '']
    for matrix in ['four','all41']:
        lines+=['## '+('Önce dört sorgu' if matrix=='four' else 'Ardından41 sorgu'),'', '|Okuyucu / case / sorgu|Kontrol p95 ms|Aday p95 ms|Fark ms|','|---|---:|---:|---:|']
        for x in out:
            if x['matrix']==matrix:lines.append(f"|{x['threads']} /{x['case_id']} /{x['text']}|{x['control_p95']:.3f}|{x['candidate_p95']:.3f}|{x['difference_ms']:+.3f}|")
    lines+=['','## Kapı ve kalan işler','','Delta oracle ve tam test/kurtarma kayıtları tamamlanınca bu bölüm güncellenir. Yeni performans eşiği eklenmedi; 4R+yazıcı A için idle≤30ms ise loaded≤30ms, diğerleri≤2×idle; throughput≥%75 ve SQLite pooled üstü, uzun tekli20/50/100ms p95/p99/p99,9. A başarısızsa D/B/C/E ve WAL düzeltmesine geçilmez. RAM D, motor dışı profil/model/oracle/history hariç ölçülecek. B/C/E,2,6sn uç ve500 drain, Frunner anahtarı, timmy cache/mutex açık. Format/256KiB/ön filtre/Blok3/4/C ABI/gerçek-soğuk/uzun snapshot/100k yok.']
    (R/'docs/DELTA-FIRST-PAGE-50K-20261030.md').write_text('\n'.join(lines)+'\n',encoding='utf-8')
    print(json.dumps([x for x in out if x['matrix']=='four'],ensure_ascii=False))
if __name__=='__main__':main()
