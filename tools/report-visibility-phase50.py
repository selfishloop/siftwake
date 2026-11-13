"""Exclusive visibility timeline plus nested measured spans; no acceptance gates."""
import csv
import json
import math
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='visibility-phase50-20261102'
STEPS=['commit_return','history_done','acquire_done','probe_prepared','diagnostic_begin_done','query_done','release_done']
NAMES=['commit','history','acquire','probe_prepare','diagnostic_init','first_query','release']
def stat(v):
    s=sorted(v);n=len(s);return dict(n=n,mean=sum(s)/n,p50=s[math.ceil(.5*n)-1],p95=s[math.ceil(.95*n)-1],max=s[-1])
def read(p):return json.loads(p.read_text(encoding='utf-8'))
def summarize(records):
    fields=['commit','gap','visibility','history','acquire','probe_prepare','diagnostic_init','first_query','release','delta','query_mask','query_total','posting_setup','candidate_verify','term_wait','cache_blocked','term_decode','read_at','term_wait_calls','cache_blocked_calls','term_page_hits','term_page_misses','term_lookup_hits','term_lookup_misses','requested_read_bytes','commit_mask','commit_prepare','collect','sql_build','sql_prepare','sql_step','sql_decode','sql_total']
    return {k:stat([r[k] for r in records]) for k in fields}
def main():
    d=read(L/f'{P}-design.json');rows=[];arms=[];selected=[];plans=[]
    for run in d['completed']:
        phases=[json.loads(x) for x in (L/run['log']).read_text(encoding='utf-8').splitlines()]
        writes=next(x for x in phases if x['phase']=='writes');assert writes['oracle']
        rr=[]
        for record in writes['records']:
            assert record['oracle'];diag=record['diagnostic'];m=diag['milestones_ns'];points=[m[k] for k in STEPS];assert points==sorted(points)
            values=[points[0]]+[b-a for a,b in zip(points,points[1:])]
            r=dict(engine=run['engine'],variant=run['variant'],kind=record['kind'],sequence=record['sequence'],slot=record['slot'],body_band=record['body_band'],path_bytes=record['path_bytes'],key_bytes=record['key_bytes'],**{k:v/1e6 for k,v in zip(NAMES,values)},gap=(points[-1]-points[0])/1e6,visibility=points[-1]/1e6,collect=diag['collection_after_visibility_ns']/1e6)
            assert abs(r['commit']-record['commit_ms'])<1e-8 and abs(r['visibility']-record['visibility_ms'])<1e-8
            assert abs(sum(r[k] for k in NAMES)-r['visibility'])<1e-8
            query=diag['query'];c=query.get('counters',{})
            for field,name in [('delta','first.delta_filter'),('query_mask','first.visibility_mask'),('query_total','first.total'),('posting_setup','first.posting_setup'),('candidate_verify','first.candidate_verify')]:r[field]=c.get(name,{}).get('elapsed_ns',0)/1e6
            for field,name in [('term_wait','query.term_cache_mutex_wait'),('cache_blocked','lock.cache_wait'),('term_decode','query.term_page_decode'),('read_at','disk.read_at')]:r[field]=c.get(name,{}).get('elapsed_ns',0)/1e6
            for field,name in [('term_wait_calls','query.term_cache_mutex_wait'),('cache_blocked_calls','lock.cache_wait'),('term_page_hits','query.term_page_hits'),('term_page_misses','query.term_page_misses'),('term_lookup_hits','query.term_lookup_hits'),('term_lookup_misses','query.term_lookup_misses'),('requested_read_bytes','disk.requested_read_bytes')]:r[field]=c.get(name,{}).get('calls',0)
            commit=diag['commit'];frames=commit.get('frames',[]) if isinstance(commit,dict) else []
            if run['engine']=='fdb':
                assert commit['dropped_frames']==0 and len(frames)==1 and frames[0]['operation_sequence']==record['sequence']
                assert c['first.total']['calls']==1 and c['first.delta_filter']['calls']==1
                assert r['query_total']<=r['first_query']+.01
            for field,name in [('commit_mask','snapshot.mask'),('commit_prepare','snapshot.prepare')]:r[field]=sum(p['elapsed_ns'] for f in frames for p in f['phases'] if p['name']==name)/1e6
            for field,name in [('sql_build','sql_build_ns'),('sql_prepare','statement_prepare_ns'),('sql_step','step_and_payload_copy_ns'),('sql_decode','json_decode_ns'),('sql_total','total_ns')]:r[field]=query.get(name,0)/1e6
            if run['engine']=='sqlite':
                assert query['returned']==(0 if record['kind']=='delete' else 1)
                if 'explain_after_visibility' in query:plans.append(dict(engine='sqlite',variant=run['variant'],sequence=r['sequence'],details=query['explain_after_visibility']))
            r['diagnostic_visibility_ns']=points[-1];r['diagnostic_gap_ns']=points[-1]-points[0]
            rows.append(r)
            if not record['kind'].startswith('edge'):rr.append(r)
        assert len(rr)==4096
        summary=summarize(rr);summary.update(engine=run['engine'],variant=run['variant'])
        gap=sum(r['gap'] for r in rr);query=sum(r['first_query'] for r in rr)
        summary['gap_share_percent']={k:100*sum(r[k] for r in rr)/gap for k in NAMES[1:]}
        summary['delta_share_query_percent']=100*sum(r['delta'] for r in rr)/query
        summary['delta_share_gap_percent']=100*sum(r['delta'] for r in rr)/gap
        summary['posting_share_gap_percent']=100*sum(r['posting_setup'] for r in rr)/gap
        summary['term_wait_share_gap_percent']=100*sum(r['term_wait'] for r in rr)/gap
        arms.append(summary)
        order=sorted(rr,key=lambda r:r['gap']);selected.append(dict(**order[math.ceil(.95*len(order))-1],selection='same event at gap nearest-rank p95; not sum of independent percentiles'))
    complete=d['stage']=='completed'
    if complete:assert len(arms)==4 and len(rows)==4097*4
    historical=[]
    if complete:
        old=read(L/'frunner-key50-20261102-design.json')
        for run in old['completed']:
            for line in (L/run['log']).open(encoding='utf-8'):
                phase=json.loads(line)
                if phase['phase']=='writes':
                    values=[x['visibility_ms']-x['commit_ms'] for x in phase['records'] if not x['kind'].startswith('edge')]
                    historical.append(dict(block=run['block'],engine=run['engine'],variant=run['variant'],gap=stat(values)));break
    summary=dict(stage=d['stage'],diagnostic_only=True,gates=None,arms=arms,selected_p95_gap_events=selected,sqlite_plans=plans,rows=len(rows),historical_B_per_event_gap=historical)
    (L/f'{P}-summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2),encoding='utf-8')
    (L/f'{P}-samples.json').write_text(json.dumps(rows,ensure_ascii=False),encoding='utf-8')
    if rows:
        with (L/f'{P}-samples.tsv').open('w',encoding='utf-8',newline='') as f:
            w=csv.DictWriter(f,fieldnames=list(rows[0]),delimiter='\t');w.writeheader();w.writerows(rows)
    lines=['# Görünürlük aşama tanısı — 2 Kasım 2026','',f'**Yalnız tanı, kapı yok.** Tamamlanan kol {len(arms)}/4. B kabul edildi; eski görünürlük KALDI kaydı/eşiği değişmedi. Bu tur motor/format/varsayılan optimizasyonu yapılmadı.','']
    if complete:
        fdb=[a for a in arms if a['engine']=='fdb'];sql=[a for a in arms if a['engine']=='sqlite']
        lines += ['**Ölçülen yer: ilk marker sorgusunun posting/terim-sözlüğü hazırlığı.** '
                  f"Numeric/Frunner FDB gap ortalamaları {fdb[0]['gap']['mean']:.3f}/{fdb[1]['gap']['mean']:.3f}ms; "
                  f"ilk sorgu gapin %{fdb[0]['gap_share_percent']['first_query']:.3f}/%{fdb[1]['gap_share_percent']['first_query']:.3f}'ü. "
                  f"Posting hazırlığı %{fdb[0]['posting_share_gap_percent']:.2f}/%{fdb[1]['posting_share_gap_percent']:.2f}; "
                  f"term-cache edinimi {fdb[0]['term_wait']['mean']:.3f}/{fdb[1]['term_wait']['mean']:.3f}ms ve sayfa okuma/CRC/decode {fdb[0]['term_decode']['mean']:.3f}/{fdb[1]['term_decode']['mean']:.3f}ms. "
                  'Bu alt süreler nested toplamların üstüne tekrar eklenmedi.', '',
                  f"Acquire {fdb[0]['acquire']['mean']:.6f}/{fdb[1]['acquire']['mean']:.6f}ms; aynı commit sequence hemen edinildi. "
                  f"Query maskesi {fdb[0]['query_mask']['mean']:.3f}/{fdb[1]['query_mask']['mean']:.3f}ms, delta {fdb[0]['delta']['mean']:.3f}/{fdb[1]['delta']['mean']:.3f}ms. "
                  f"Snapshot hazırlığı {fdb[0]['commit_prepare']['mean']:.3f}/{fdb[1]['commit_prepare']['mean']:.3f}ms ve maskesi {fdb[0]['commit_mask']['mean']:.3f}/{fdb[1]['commit_mask']['mean']:.3f}ms commitin İÇİNDE ölçüldü. "
                  'Bu koşuda commit sonrası yayımlanacak sürümü bekleme değil, edinilmiş sürüm üzerindeki ilk doğrulama aramasının çalışması baskın.', '',
                  f"SQLite gap {sql[0]['gap']['mean']:.3f}/{sql[1]['gap']['mean']:.3f}ms, ilk sorgu {sql[0]['first_query']['mean']:.3f}/{sql[1]['first_query']['mean']:.3f}ms. "
                  'Bu tanıdaki daha düşük uçtan uca sürenin farkı ilk-query aşamasında ölçüldü; geçmiş B bloklarının neden değiştiğini bu ölçüm tek başına kanıtlamaz.', '',
                  '**Önemli sınır:** eski Bdeki yaklaşık15ms bu tanıda aynı sayı olarak üretilmedi: profilsiz numeric B ilküç blok gerçek işlem-bazlı gap p95leri14,719–16,831ms; yeni tanıda numeric/Frunner gap ortalamaları27–30ms, p95leri45,799/48,629ms. '
                  'Sayaçların maliyeti ile oturum/ilerlemiş yerleşim etkisi ayrı A/B ile ölçülmedi, payları **bilinmiyor**. '
                  'Eski profilsiz blokların tam aşama dağılımı **bilinmiyor**; aşağıdaki tarihsel gapler yalnız ham işlem farklarıdır.', '']
    lines += [
           '## Yöntem ve zaman sınırları','',
           'Aynı881756DA ilk50k taban ve önceki257-record ayrı havuz. Aynı256 donör/gövde/yol,4096 normal+1 uzun-yol transaction,4R beşli sorgu yükü,N60+. Tek seferlik bir dengeli blok: FDB numeric / SQLite numeric / SQLite Frunner / FDB Frunner. Reset/yeniDBkopyası/yeniden50kingest yok;private/ okunmadı. Ana50k ön/son oracle/digest ile korunur; havuzlar temizlenir. Sıcak/kontrolsüz OS cache, eviction yok.','',
           'Commit başlangıcından tek monotonic clock üzerindeki sınırlar: commit return → benchmark history yayımı → acquire → marker/Query hazırlığı → profiler init → ilk marker sorgusu → snapshot release. Ardışık sınır farkları ayrık ve toplamı visibility ile tam eşittir. Gap=visibility−commit; history kısmı motora ait değildir. Tam GET/eski-marker oracle daha sonra gelir ve visibility içinde değildir. Profiler sonuçlarını toplama/JSON/SQL EXPLAIN görünürlük zamanından SONRA yapılır ve ayrıca ölçülür.','',
           'FDB mevcut opt-in commit trace snapshot.prepare/mask ile ilk sorgunun request-local first.total/delta_filter/visibility_mask/posting/candidate sayaçları kullanıldı. Bunlar nested wall süreleridir; commit maskesi commit süresinin, delta/query maskesi ilk sorgunun alt kümesidir. Üst/alt süreler toplanıp yeni toplam üretilmez. Background okuyucular ayrı request context kullanır; yeni worker/izin politikası yok. SQLite yalnız writer probe için SQL hazırlama/step+payload kopyası/JSON decode alt zamanları; ilk plan EXPLAIN queryden sonra alındı.','',
           'Profil derlemesi ve sayaçlar zamanlamayı/4R kaynak takvimini etkileyebilir. Normal B p95leri bu tanıyla değiştirilmez; farklı seans/yerleşim etkisi ölçülmedi. Bu veri kabul kapısı veya önce/sonra kazanç deneyi değildir. Eski blokların neden farklı çıktığını bu tek tanıyla kesinleştirmiyoruz.','',
           '## Ayrık zamanlar — normal4096 işlem/kol ortalama ms','',
           '|Motor/anahtar|Commit|Commit→visibility gap|History|Acquire|Probe hazırlık|Profiler init|İlk sorgu|Release|', '|---|---:|---:|---:|---:|---:|---:|---:|---:|']
    for a in arms:lines.append(f"|{a['engine']}/{a['variant']}|"+'|'.join(f"{a[k]['mean']:.6f}" for k in ['commit','gap','history','acquire','probe_prepare','diagnostic_init','first_query','release'])+'|')
    lines+=['','## Gap payları; aynı olayların toplamından yüzde','','|Motor/anahtar|History %|Acquire %|Hazırlık %|Profiler init %|İlk sorgu %|Release %|','|---|---:|---:|---:|---:|---:|---:|']
    for a in arms:lines.append(f"|{a['engine']}/{a['variant']}|"+'|'.join(f"{a['gap_share_percent'][k]:.3f}" for k in NAMES[1:])+'|')
    lines+=['','## FDB: ilk sorgu ve commit alt kümeleri','','|Anahtar|İlk sorgu ort ms|Delta filtre ort ms|Delta /sorgu %|Delta /gap %|Query maskesi ort ms|Commit snapshot.prepare ort ms|Commit snapshot.mask ort ms|','|---|---:|---:|---:|---:|---:|---:|---:|']
    for a in arms:
        if a['engine']=='fdb':lines.append(f"|{a['variant']}|{a['first_query']['mean']:.6f}|{a['delta']['mean']:.6f}|{a['delta_share_query_percent']:.3f}|{a['delta_share_gap_percent']:.3f}|{a['query_mask']['mean']:.6f}|{a['commit_prepare']['mean']:.6f}|{a['commit_mask']['mean']:.6f}|")
    lines+=['','## FDB posting/sözlük hazırlığı altındaki mevcut sayaçlar','','|Anahtar|Posting setup ort ms|Term-cache acquire ort ms|Contended cache-wait ort ms (nested)|Term-page decode/read/CRC ort ms|Read-at ort ms (nested)|Posting/gap %|Term-cache acquire/gap %|','|---|---:|---:|---:|---:|---:|---:|---:|']
    for a in arms:
        if a['engine']=='fdb':lines.append(f"|{a['variant']}|"+'|'.join(f"{a[k]['mean']:.6f}" for k in ['posting_setup','term_wait','cache_blocked','term_decode','read_at'])+f"|{a['posting_share_gap_percent']:.3f}|{a['term_wait_share_gap_percent']:.3f}|")
    lines+=['','|Anahtar|Term-cache lock çağrı/probe|Contended cache lock/probe|Term page hit/probe|Term page miss/probe|Lookup hit/probe|Lookup miss/probe|İstenen read byte/probe|','|---|---:|---:|---:|---:|---:|---:|---:|']
    for a in arms:
        if a['engine']=='fdb':lines.append(f"|{a['variant']}|"+'|'.join(f"{a[k]['mean']:.3f}" for k in ['term_wait_calls','cache_blocked_calls','term_page_hits','term_page_misses','term_lookup_hits','term_lookup_misses','requested_read_bytes'])+'|')
    lines+=['','`query.term_cache_mutex_wait` bütün term-cache kilit edinme süresi; `lock.cache_wait` yalnız contended cache lockların bloklanan kısmı. Aynı bekleme iki sayaçta örtüşebilir; tekrar eklenmez. İkinci sayaç diğer cache mutexlerini de kapsayabilir; mutex sahibi okuyucu/yazıcı thread kimliği ölçülmedi. Term cache süreçte ortaktır; sayfa missinde read/CRC/decode aynı guard içindedir ([kod](../src/segment/term_pages.rs)). Bekleme ölçüldü; belirli bir thread, scheduler veya antivirüse neden atfedilmedi. `disk.read_at` decode içinde de bulunduğundan üstüne eklenmez. İstenen read byte gerçek OS cache miss/diskten okunan fiziksel byte değildir.','',
           'Delta filtre spanı birlikte gram filtre kontrolü, gerekirse WAL payload okuma/CRC, decode, fold/contains ve key dolaşımını kapsar ([kod](../src/engine/light.rs)). Bunların tek tek payları bu tur ölçülmedi: **bilinmiyor**. Wall süresinin saf CPU/I/O/OS-cache miss/scheduler/antivirüs dağılımı da **bilinmiyor**. Commit içindeki snapshot hazırlığını post-commit gap ile karıştırma.','',
           '## SQLite: ilk sorgu alt süreleri','','|Anahtar|İlk sorgu ort ms|SQL kurma|Statement prepare|Step+payload copy|JSON decode|','|---|---:|---:|---:|---:|---:|']
    for a in arms:
        if a['engine']=='sqlite':lines.append(f"|{a['variant']}|"+'|'.join(f"{a[k]['mean']:.6f}" for k in ['first_query','sql_build','sql_prepare','sql_step','sql_decode'])+'|')
    for p in plans:lines += ['',f"EXPLAIN ({p['variant']}, sequence{p['sequence']}; visibility dışında):",'']+[f'- {detail}' for detail in p['details']]
    lines+=['','SQL step+sütun kopyası; VM çalışması, satır bulma/sıralama ve folded LIKE doğrulamasını tek tek ayırmaz. Referansın folded alanı/FTS trigrama yazıda hazırlanır ([kod](../tools/native_sqlite/src/bin/key_pool.rs)); sorgu planı yukarıda ölçüldü. Bu tanıdaki düşük query süresi ölçülmüş gözlemdir. SQLite için FDB counter alanlarının0 olması SQLite içinde hiç kilit/CRC maliyeti olmadığı anlamına gelmez; bu sayaçlar SQLite içi için uygulanmadı. Tarihsel B bloklarının ortam/cache/yerleşim farkı profillenmediğinden o blokların kesin nedeni **bilinmiyor**.','',
           '## Gap p95ine denk gelen aynı sequence olayları','','|Motor/anahtar|Sequence/tip|Commit|Gap|History|Acquire|İlk sorgu|Release|Delta altı|Query maskesi altı|','|---|---|---:|---:|---:|---:|---:|---:|---:|---:|']
    for r in selected:lines.append(f"|{r['engine']}/{r['variant']}|{r['sequence']}/{r['kind']}|"+'|'.join(f"{r[k]:.6f}" for k in ['commit','gap','history','acquire','first_query','release','delta','query_mask'])+'|')
    lines+=['','## P50/p95/max; tüm normal işlem örnekleri','','|Motor/anahtar|Metrik|N|p50/p95/max ms|','|---|---|---:|---|']
    for a in arms:
        for k in ['commit','visibility','gap','history','acquire','first_query','release','collect']:
            x=a[k];lines.append(f"|{a['engine']}/{a['variant']}|{k}|{x['n']}|{x['p50']:.6f}/{x['p95']:.6f}/{x['max']:.6f}|")
    if historical:
        lines+=['','## B hamlarından gerçek işlem-bazlı gap (profilsiz, tarihsel)','','Her işlemde visibility−commit hesaplandı; bağımsız p95lerden çıkarma yapılmadı. Bu bloklarda aşama sayacı yoktu; yeni tanı onların tam nedeni sayılmaz.','',
                '|Blok/motor/anahtar|Gap ort/p50/p95/max ms|', '|---|---|']
        for x in historical:lines.append(f"|{x['block']}/{x['engine']}/{x['variant']}|"+'/'.join(f"{x['gap'][k]:.6f}" for k in ['mean','p50','p95','max'])+'|')
    audit=L/f'{P}-audit.json'
    if audit.exists():
        a=read(audit);lines+=['','## Doğrulama ve kapanış','',f"{a['tests_passed']} default +{a['diagnostic_tests_passed']} diagnostic +{a['sql_tests_passed']} SQLite test; fmt/default ve diagnostic clippy geçti.9 witnessedkill/reopen ve eski okuyucu oracle geçti. Ana50k GET/41case ve SQLite payload digest korundu. İki pool canlı kaydı0;motor kaynakları accepted-default ile SHA eşit. {a['rows']} sample zaman sınırları monoton ve ayrık toplam denetimi geçti. Güç kesintisi/in-flight commit iddiası yok. Bu tanıdan sonra duruldu."]
    lines+=['',f'[Tasarım/sıra/SHA/süreç logları](benchmarks/{P}-design.json) · [Özet](benchmarks/{P}-summary.json) · [Tüm ölçülen örnekler](benchmarks/{P}-samples.tsv) · [JSON örnekler](benchmarks/{P}-samples.json) · [Olaylar](benchmarks/{P}-lifecycle.jsonl).','',
           '100lü,C500drain,2,6sn,WAL handle/File::open,zx,reclaim varsayılanı,C ABI yapılmadı. Geniş tarama/case34,signed-zero,yazma/bakım,disk/sorguRAM açık. [OPEN-WORK](OPEN-WORK.md).']
    (R/'docs/VISIBILITY-PHASE-50K-20261102.md').write_text('\n'.join(lines)+'\n',encoding='utf-8')
    print(json.dumps(dict(stage=d['stage'],completed=len(arms),diagnostic_only=True,arms=[dict(engine=a['engine'],variant=a['variant'],gap_ms=a['gap']['mean'],first_query_ms=a['first_query']['mean'],delta_ms=a['delta']['mean']) for a in arms]),ensure_ascii=False))
if __name__=='__main__':main()
