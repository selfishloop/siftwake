"""Read saved query evidence; report failures without changing policy defaults."""
import csv,importlib,json,math
from pathlib import Path
b=importlib.import_module('measure-churn');ROOT,LOGS=b.ROOT,b.LOGS
P='revision-search-10k-20261026'
def load(p):return json.loads(p.read_text(encoding='utf-8-sig'))
def rows(p):return [json.loads(x) for x in p.read_text(encoding='utf-8-sig').splitlines()]
def q(values,p=.95):return sorted(values)[math.ceil(len(values)*p)-1]
def export(label,data):
    with (LOGS/f'{P}-{label}.csv').open('w',encoding='utf-8',newline='') as f:
        w=csv.DictWriter(f,fieldnames=list(data[0]));w.writeheader();w.writerows(data)
def main():
    design=load(LOGS/f'{P}-design.json');assert design['complete']
    query={};cases={}
    for layout in ['candidate','reference','sqlite']:
        for variant in (['before','after'] if layout!='sqlite' else ['sqlite']):
            for pair in [0,1]:
                label=f'{layout}-r{pair}'+('' if layout=='sqlite' else '-'+variant)
                engine='sqlite' if layout=='sqlite' else 'fdb'
                raw=rows(LOGS/f'{P}-{label}-{engine}-streamed-search-matrix.jsonl')
                matrix=next(x for x in raw if x['phase']=='matrix');assert matrix['oracle']
                for case_id,r in enumerate(matrix['records']):
                    cases[case_id]=r['case'];query.setdefault((layout,variant,case_id),[]).extend(r['samples_ms'])
    comparison=[]
    for c in range(38):
        vals={name:q(query[layout,v,c]) for name,layout,v in [('ref','reference','before'),('old_candidate','candidate','before'),('candidate','candidate','after'),('new_ref','reference','after'),('sqlite','sqlite','sqlite')]}
        assert all(len(query[layout,v,c])==60 for layout,v in [('reference','before'),('reference','after'),('candidate','before'),('candidate','after'),('sqlite','sqlite')])
        comparison.append(dict(case_id=c,**cases[c],**{k+'_p95_ms':v for k,v in vals.items()},candidate_max_ms=max(query['candidate','after',c]),delta_ms=vals['candidate']-vals['ref'],regression_percent=(vals['candidate']/vals['ref']-1)*100,against_old_candidate_percent=(vals['candidate']/vals['old_candidate']-1)*100,tracked=c==34))
    profiles=[]
    for variant,label in [('candidate-before','candidate-before-r1'),('candidate-after','candidate-profile-after'),('reference','reference-profile')]:
        raw=rows(LOGS/f'{P}-{label}-fdb-streamed-search-profile.jsonl')
        for c in range(38):
            data=[x for x in raw if x.get('phase')=='search_profile' and x['case_id']==c];assert len(data)==5 and all(r['oracle'] for r in data)
            calls=lambda name:sum(x['counters'].get('query.'+name,{}).get('calls',0) for x in data)/5
            ms=lambda name:sum(x['counters'].get(name,{}).get('elapsed_ns',0) for x in data)/5/1e6
            row=dict(variant=variant,case_id=c,text=cases[c]['text'],elapsed_mean_ms=sum(x['elapsed_ms'] for x in data)/5)
            row.update({n:calls(n) for n in ['seed_ids','hidden_skipped','cached_hidden_skipped','visible_seed_ids','payload_candidates','payload_read_requests','payload_window_hits','payload_read_bytes','payload_requested_bytes']})
            row.update({n+'_ms':ms('query.'+stage) for n,stage in [('mask','visibility_mask'),('setup','posting_setup'),('intersection','posting_intersection'),('verify','candidate_verify'),('read','payload_read'),('crc','payload_crc'),('schema','payload_validation'),('text','verify_text')]})
            row['page_ms']=ms('search.merge_page_hydration');row['filter_ms']=ms('search.filter_visibility_text')
            row['other_filter_ms']=row['filter_ms']-sum(row[n+'_ms'] for n in ['mask','setup','intersection','verify'])
            profiles.append(row)
    maintenance=[]
    for layout in ['candidate','reference']:
        for r in rows(LOGS/f'{P}-{layout}-prepare-fdb-streamed-churn.jsonl'):
            if r['phase']=='churn_round':maintenance.append(dict(layout=layout,round=r['round'],maintenance_ms=r['maintenance_ms'],commit_ms=sum(x['commit_ms'] for x in r['attempts']),retained_bytes=r['after']['retained_bytes'],oracle=r['full_oracle']))
    kills=load(LOGS/'revision-search-kill-10k-20261026-summary.json')
    checks=load(LOGS/f'{P}-checks.json');assert checks['status']=='complete'
    failed=[r for r in comparison if not r['tracked'] and r['regression_percent']>5]
    kill_ok=lambda r:(r['oracle'] and r['verify_exit_code']==0 and r['marker_oracle']['all_passed']) if 'mode' in r else (r['coverage_observed'] and r['reopen_full_payload_matrix_oracle_exit_code']==0 and r['pending_old_new_marker_oracle']['all_passed'])
    gates=dict(search_37=not failed,disk_1_3=all(r['retained_bytes']<=350412679*1.3 for r in maintenance if r['layout']=='candidate'),maintenance_previous_max=sum(r['maintenance_ms'] for r in maintenance if r['layout']=='candidate')<=13950.2008,oracle=all(r['oracle'] for r in maintenance),kill=all(kill_ok(r) for r in kills['results']),old_reader=kills['old_reader']['exit_code']==0,checks=all(r['exit_code']==0 for r in checks['steps']))
    summary=dict(gates=gates,failed_cases=[r['case_id'] for r in failed],queries=comparison,profiles=profiles,maintenance=maintenance,design=design,all_passed=all(gates.values()))
    b.write(LOGS/f'{P}-summary.json',summary)
    for label,data in [('queries',comparison),('profiles',profiles),('maintenance',maintenance)]:export(label,data)
    render(summary);print(json.dumps(gates,indent=2))
def render(s):
    bad=s['failed_cases'];m=[r for r in s['maintenance'] if r['layout']=='candidate'];cost=sum(r['maintenance_ms'] for r in m)/1000
    old_fail=sum(not r['tracked'] and r['old_candidate_p95_ms']>r['ref_p95_ms']*1.05 for r in s['queries'])
    own_regress=sum(not r['tracked'] and r['against_old_candidate_percent']>5 for r in s['queries'])
    case_name=lambda r:r['text'] if len(r['text'])<60 else 'zx ×150'
    out=['# Revizyon görünürlüğü / arama — 10k, 26 Ekim 2026','',
        '**37 sorgu kapısı: '+('GEÇTİ' if not bad else 'KALDI')+f'.** Referansa göre +%5 aşan {len(bad)}/37. Case34 izleniyor. Revizyon geri kazanımı ve yeni sorgu deneyi varsayılan kapalıdır.','',
        f'Bu oturumda eski aday da referansa göre {old_fail}/37 durumda +%5 üstünde; yeni ek pruning kendi eski adayına göre {own_regress}/37 durumda +%5 üstünde. Önceki oturum24→bu oturum6 farkını deney kazancı diye sunmuyoruz. **Ek pruning deneyi reddedildi; varsayılan yapılmadı.**','',
        'Kullanıcı önceki bakım sonucunu kabul etti: eski politika yaklaşık36s, aday13,7s, SQLite24–25s. 12s ölçüm hedefi geçmiş gibi yeniden etiketlenmedi. Bu ayrı iş geri kazanma eşiklerini, WAL/manifest/payload/SEG biçimini değiştirmez.','',
        '## Ölçülen kök ayrım','',
        'Görünürlük maskesi mevcut kodda seed ID döngüsünde, posting kesişimi ve payload doğrulamasından ÖNCE uygulanıyordu. Dolayısıyla ölü kayıtların belge olarak okunup doğrulandığı varsayımı doğrulanmadı. Profil hidden-skip sayısını doğrudan ölçer. Okuma pencereleri komşu byte aralıklarını önceden getirebilir; fiziksel read-ahead içindeki her fazla byte ölü payload diye sınıflandırılmadı.','',
        '| Sürüm / sorgu | Seed / maskede elenen | Payload adayı | Okunan / istenen MB | Maske hazırlığı / diğer ms | Posting setup / intersection ms | Payload verify ms | Read / CRC / schema / text ms |','|---|---|---:|---|---|---|---:|---|']
    for r in s['profiles']:
        if r['case_id'] not in [8,21,22,23]:continue
        out.append(f'| {r["variant"]} / {r["text"]} | {r["seed_ids"]:.0f} / {r["hidden_skipped"]:.0f} | {r["payload_candidates"]:.0f} | {r["payload_read_bytes"]/1e6:.3f} / {r["payload_requested_bytes"]/1e6:.3f} | {r["mask_ms"]:.3f} / {r["other_filter_ms"]:.3f} | {r["setup_ms"]:.3f} / {r["intersection_ms"]:.3f} | {r["verify_ms"]:.3f} | {r["read_ms"]:.3f} / {r["crc_ms"]:.3f} / {r["schema_ms"]:.3f} / {r["text_ms"]:.3f} |')
    out+=['','Her profil hücresi5 gözlem ortalamasıdır; normal p95 değildir. Verify parent span; read/CRC/schema/text onun içindedir, süreler üst üste eklenmez. Setup posting lookup ve küçük cache hazırlığını içerir. Bütün38 durumun ham sayaçları ve süreleri profiles CSV içinde.','',
        '`the zx` için aday 2.013 read /7.987 window hit, referans 1.112 read /8.888 hit yapıyor. İkisi de10.000 canlı adayı ve268,304MB payload\'ı istiyor; aday286,171MB, referans287,355MB fiziksel byte okuyor. Byte toplamından çok istek parçalanması artıyor. Kod üç fiziksel komşu istekten sonra read-ahead açar; maskede atlanan kaynak aralıkları komşuluğu böler. Bu, bu sorguda ölçülmüş I/O bedelidir; bütün p95 farkları için tek neden diye sunulmaz. Darcy, holmes elementary, quixote whale canlı aday/payload byte sayıları referansla eşit.','',
        'Maske hazırlığı engine\'deki cache/mask kurulumudur. Prefix visible_count sayımı, ana seed okuma/decode döngüsü ve kalan filtreleme işi diğer sütunundadır; bunlar ayrı ayrı ölçülmüş gibi sunulmaz. Other, filter parent süresinden mask/setup/intersection/verify ayrık aralıklarının çıkarılmasıdır.','',
        '## Dar deney ve sınır','',
        '`set_query_visibility_pruning(true)` iki ek erken eleme uygular: görünür kaydı0 segmenti posting sözlüğüne girmeden atlar; küçük secondary posting cache\'lerini maskeden geçirir ve boş grup varsa seed/payload işine başlamadan döner. Eski seed maskesi ve CRC/schema/identity/sıralı ID-bounds kontrolleri korunur. Ek corpus, ek kalıcı indeks veya eşik değişimi yok. Cache aynı256KiB üst bütçede, retain yerinde çalışır. Deney varsayılan kapalıdır.','',
        'Bu deney mantıksal ölü kayıt doğrulamasını kaldıran yeni bir yol olarak sunulmaz: o eleme zaten vardı. Canlı adayların payload maliyetini ve parçalı yerleşimin I/O bedelini maske tek başına ortadan kaldırmaz.','',
        'Ara cache\'lerden elenen ID sayısı belge sayısı değildir: aynı belge farklı gram listelerinde tekrar sayılır. Örneğin holmes elementary için20.272 ara ID silindiği halde doğrulanan canlı aday sayısı925 olarak kaldı.','',
        '## Yöntem / normal ölçüm','',
        'Mevcut aynı ilk10000 belge. Mevcut420,644MB aday ilk çiftte doğrudan yeniden kullanıldı; kaynak SHA\'ları okumalar boyunca aynı. Sonra sabit FDB slotunda eski %20/sabit pacing politikasıyla10 tur referans, ardından aynı10 tur aday kuruldu. Yeni corpus hazırlanmadı; DB kopyası tutulmadı. Reader sırası ilk adayda before→after, referansta before→after / after→before, son adayda after→before. Her sorgu30 zamanlı örnek ve eşlenmiş warmup; reader/layout başına pooled60. İki adayın mantıksal modelleri eşit doğrulandı.','',
        'SQLite mevcut10 tur DB\'siyle, FTS5trigram/detailnone/FULLWAL/cache2MiB/mmap0/tempFILE ayarlarında iki30 örnekli okuma ve full oracle: yeniden ingest yok. OS cache sıcak; CPU/power/background izolasyonu yok. Oracle ve corpus okuması query timer dışında. Normal ve diagnostic derlemeler ayrıdır. Küçük negatif sorgularda yüksek yüzde için mutlak delta ayrıca verilir; küçük ms farkları kapıdan muaf tutulmaz.','',
        '| Case / sorgu / offset / limit | Ref p95 ms | Eski aday p95 | Yeni aday p95 | Δ ms / % | Yeni / eski aday % | SQLite p95 |','|---|---:|---:|---:|---|---:|---:|']
    for r in s['queries']:out.append(f'| {r["case_id"]}{" izlenen" if r["tracked"] else ""} / {case_name(r)} / {r["offset"]} / {r["limit"]} | {r["ref_p95_ms"]:.6f} | {r["old_candidate_p95_ms"]:.6f} | {r["candidate_p95_ms"]:.6f} | {r["delta_ms"]:+.6f} / %{r["regression_percent"]:+.2f} | %{r["against_old_candidate_percent"]:+.2f} | {r["sqlite_p95_ms"]:.6f} |')
    out+=['','## Bakım/disk ve doğruluk','',f'Yeni aday10tur bakım toplamı {cost:.3f}s (önceki kabul edilmiş iki koşu13,500/13,950s); disk son tur {m[-1]["retained_bytes"]/1e6:.3f}MB. Ölçüm bir ek10tur tekrarıdır; yeni30tur koşusu yapılmadı. Query değişimi bakım kodunu veya kaynakları değiştirmez. Önceki30tur1,14× sonucu tarihsel kabul olarak korunur; yeni bir30tur kanıtı sayılmaz.','',
        'Bakımın önceki kabul edilmiş üst gözlemi13,9502s aşmaması ayrıca kontrol edildi. Bu kontrol eski12s hedefini geçmiş saymaz. Query-only değişimde bakım kodu ve eşikler aynıdır.','',
        'Oracle, yedi gözlenen emeklilik/iptal/iterator/flush/merge/cache kill-reopen ve eski frozen okuyucu doğrulaması tekrar geçti. Power-loss iddiası yok. Genel dört kill penceresinin owned/streaming kapsamı önceki rapordaki gibidir; bütün responsive worker evrelerinin kill kapsamı iddia edilmez. `cargo fmt --check`, default ve diagnostic clippy all-targets -Dwarnings, tam cargo test, no-default lib ve workload testleri geçti.','',
        '## Karar ve açık işler','',
        ('37 sorgu kapısı geçmediği için snapshot Adım2 başlatılmadı. Maskenin zaten yaptığı işi ikinci kez çözmüş gibi sunmuyoruz; ek pruning deneyi kapalı kalır. Sonraki karar için gerçek fark canlı payload I/O/doğrulama tarafındadır.' if bad else '37 sorgu kapısı geçti; sonraki aşama snapshot prototipi ve kendi eşzamanlılık kapılarıdır.'),'',
        f'Snapshot\'ın ≤100ms mutlak maksimum kapısı için ayrıca başlangıç sınırı var: bu boşta matriste adayın en kötü tekil sorgusu {max(r["candidate_max_ms"] for r in s["queries"]):.3f}ms. Snapshot kilit beklemesini azaltabilir; bu boşta tam contains taramasını kendiliğinden ≤100ms yapmış sayılmaz.','',
        'Adım3 bilinen sınırı: sürekli yazma revision worker\'larını iptal ettirip geri kazanmayı boşta zamana erteleyebilir; bu yükte süresiz disk üst sınırı kanıtlanmadı. Genel RAM, dirty500 açılış, küçük commit, çok okuyucu/soğuk yük ve geçici dosyalar açık. C ABI uygulanmadı.','',
        f'[Özet / kapılar](benchmarks/{P}-summary.json) · [Ham query CSV](benchmarks/{P}-queries.csv) · [Profil CSV](benchmarks/{P}-profiles.csv) · [Bakım CSV](benchmarks/{P}-maintenance.csv) · [Komutlar](benchmarks/{P}-checks.json) · [OPEN-WORK](OPEN-WORK.md).']
    (ROOT/'docs/REVISION-VISIBILITY-SEARCH-10K-20261026.md').write_text('\n'.join(out)+'\n',encoding='utf-8')
if __name__=='__main__':main()
