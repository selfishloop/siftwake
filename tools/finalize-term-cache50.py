"""Derived tables only, after the fixed experiment. No engine/DB mutation."""
import collections,csv,importlib.util,json,statistics
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='term-cache50-20261102'
spec=importlib.util.spec_from_file_location('report',R/'tools/report-term-cache50.py');report=importlib.util.module_from_spec(spec);spec.loader.exec_module(report)
def read(p):return json.loads(p.read_text(encoding='utf-8-sig'))
def write(p,v):p.write_text(json.dumps(v,ensure_ascii=False,indent=2),encoding='utf-8')
def tsv(path,rows):
    with path.open('w',encoding='utf-8',newline='') as f:
        w=csv.DictWriter(f,fieldnames=list(rows[0]),delimiter='\t');w.writeheader();w.writerows(rows)
def main():
    report.main();s=read(L/f'{P}-summary.json');d=read(L/f'{P}-design.json');assert s['stage']=='completed'
    samples=read(L/f'{P}-pool-samples.json');a=dict(throughput={},ram={},normal_failures=[],profiles=[],matched_secondary=[],gates={},default='K unchanged',block2='closed')
    normal=[]
    for x in s['normal']:
        r={k:x[k] for k in ['pair','arm','case_id','text','limit','passed']}
        for phase in ['one','idle','loaded']:
            for k in ['n','p50','p95','max']:r[f'{phase}_{k}']=x[phase][k]
        normal.append(r)
        if not x['passed']:a['normal_failures'].append(r)
    tsv(L/f'{P}-normal-table.tsv',normal)
    pt=[]
    for x in s['pools']:
        for kind,st in x['by_kind'].items():
            r=dict(pair=x['pair'],arm=x['arm'],kind=kind,passed_if_FDB_rule=st['passed'],reference_only=x['arm']=='S')
            for name in ['commit','visibility','gap','first_query']:
                for k in ['n','p50','p95','p99','max']:r[f'{name}_{k}']=st[name][k]
            pt.append(r)
    tsv(L/f'{P}-pool-types.tsv',pt)
    # Supplement only: deterministic intersection of unprofiled ordinal slots.
    # Primary gates keep their original per-arm unprofiled cohorts.
    for pair in range(4):
        common=set(range(4096));byarm={}
        for arm in ['K','A','D','S']:
            rs={r['ordinal']:r for r in samples if r['pair']==pair and r['arm']==arm and not r['profiled'] and r['ordinal']<4096}
            byarm[arm]=rs;common.intersection_update(rs)
        for arm in ['K','A','D','S']:
            xs=[byarm[arm][n] for n in sorted(common)]
            a['matched_secondary'].append(dict(pair=pair,arm=arm,n=len(xs),gap=report.stat([x['gap'] for x in xs]),first_query=report.stat([x['first_query'] for x in xs])))
    for pair in range(4):
        for arm in ['K','A','D','S']:
            xs=[x for x in samples if x['pair']==pair and x['arm']==arm and x['profiled']]
            p=dict(pair=pair,arm=arm,n=len(xs),kinds=dict(collections.Counter(x['kind'] for x in xs)))
            if arm!='S':
                for name,key in [('cache_wait','query.term_cache_mutex_wait'),('slot_wait','query.term_decode_slot_wait'),('decode','query.term_page_decode'),('posting','first.posting_setup')]:
                    p[name]=sum(x['profile']['query']['counters'].get(key,{}).get('elapsed_ns',0) for x in xs)/len(xs)/1e6
                    p[name+'_calls']=sum(x['profile']['query']['counters'].get(key,{}).get('calls',0) for x in xs)/len(xs)
                p['parent_child_cpu_ms']=sum(sum(t.get('cpu_100ns') or 0 for t in x['profile']['query']['threads']) for x in xs)/len(xs)/10000
            a['profiles'].append(p)
    for arm in ['K','A','D','S']:
        xs=[x for x in s['arms'] if x['arm']==arm and x['phase']=='normal'];c=next(x for x in s['arms'] if x['arm']==arm and x['phase']=='control')
        fps=sum(x['mutations'] for x in xs)*1000/sum(x['writer_elapsed_ms'] for x in xs)
        a['throughput'][arm]=dict(ops_s=fps,control_ops_s=c['ops_s'],retention=fps/c['ops_s'],normal_counts=[x['passed'] for x in xs],
                                 cpu_ms=[x['cpu_ms_all_process_threads'] for x in xs],write_long_pass=all(x['write']['p95']<=20 and x['write']['p99']<=50 and x['write']['p999']<=100 for x in xs))
        if arm!='S':
            a['ram'][arm]={mode:[round(x['private_bytes']/2**20,6) for x in s['ram'] if x['arm']==arm and x['mode']==mode] for mode in ['idle','idle-warm']}
            a['ram'][arm]['loaded_model_included']=[x['memory']['sampled_phase_peak_private_bytes']/2**20 for x in xs]
    for arm in ['K','A','D']:
        a['gates'][arm]=dict(normal41=all(x['passed'] for x in s['normal'] if x['arm']==arm),
                            throughput75=a['throughput'][arm]['retention']>=.75,
                            throughput_above_sqlite=a['throughput'][arm]['retention']>a['throughput']['S']['retention'],
                            long_commit=a['throughput'][arm]['write_long_pass'],
                            idle15=all(v<=15 for mode in ['idle','idle-warm'] for v in a['ram'][arm][mode]),
                            short_commit=all(x[f'commit_{k}']<=v for x in pt if x['arm']==arm and not x['kind'].startswith('edge') for k,v in [('p95',20),('p99',50),('max',100)]),
                            visibility=all(x['passed_if_FDB_rule'] for x in pt if x['arm']==arm))
    run_counts={}
    for phase in ['pre-A','post-normal-A','final-K']:
        inv=read(L/f'{P}-{phase}-recovery-inventory.json');run_counts[phase]=len(inv[0]['manifest']['runs'])
    a['manifest_live_runs']=run_counts
    write(L/f'{P}-analysis.json',a)
    lines=['**KAPANIŞ: A ve D varsayılan yapılmadı; K korunuyor. Blok 2 kapandı.**',
           'Deney2 Kasım2026 yerel saat23:52de başladı,3 Kasım02:07de tamamlandı; dosya etiketi başlangıç tarihidir. Yeni performans turu yok.',
           '', '|Kapı|K kontrol|A kilit dışında|D sayfa4MiB|', '|---|---|---|---|']
    for label,key in [('Normal41: dört blokta her sorgu','normal41'),('Uzun tekli p95/p99/p99,9:20/50/100ms','long_commit'),('Havuz tekli commit:20/50/100ms','short_commit'),('Havuz görünürlük:20/50/100ms','visibility'),('Yazma korunum >=%75','throughput75'),('Korunum SQLite pooled üstü','throughput_above_sqlite'),('Motor-only fresh/warm idle <=15MiB','idle15')]:
        lines.append('|'+label+'|'+'|'.join('GEÇTİ' if a['gates'][arm][key] else 'KALDI' for arm in ['K','A','D'])+'|')
    lines += ['|Oracle/CRC/snapshot sıra-pin/eski okuyucu/kill-reopen|GEÇTİ|GEÇTİ|GEÇTİ|',
              '|Net gap/ilk-query kazanımı|Referans|KANITLANMADI|KANITLANMADI|',
              '', '**Kapı revizyonu yok.** A normal41 sonucu41/40/41/41; D40/41/41/40; K41/40/40/40; SQLite40/40/40/41. Tarihsel kullanıcı kabul notları yeni sonuçları GEÇTİ yapmaz. SQLite tekli max için kabul kapısı yok; yalnız referanstır.',
              '', '### Toplam gap p95 — aynı blok, ms','', '|Blok|K|A|A−K|D|D−K|SQLite|','|---|---:|---:|---:|---:|---:|---:|']
    for pair in range(4):
        v={arm:next(x['gap']['p95'] for x in s['pools'] if x['pair']==pair and x['arm']==arm) for arm in ['K','A','D','S']}
        lines.append(f"|{pair}|{v['K']:.3f}|{v['A']:.3f}|{v['A']-v['K']:+.3f}|{v['D']:.3f}|{v['D']-v['K']:+.3f}|{v['S']:.3f}|")
    lines += ['', 'A da D de2/4 blokta daha hızlı,2/4te daha yavaş. İlk-query p95leri gap ile aynı yönü izliyor. Bu nedenle mutex sayacındaki azalma veya daha büyük cache tek başına başarı sayılmadı.',
              '', '**Kilit etkisi:** A normal SEG05 miss read/CRC/decode\'unu guard dışına taşıdı; tek global decoder ve RAII rezervasyon var. Örneklerde cache-acquire beklemesi0,336–1,361ms iken yeni slot beklemesi1,043–5,641ms. Yeniden kontrol/rezervasyon ek mutex edinimleri yaratıyor. Profil örnekleri64/kol ve işlem tipleri eşleşmiyor; bunlardan saf CPU/tek-aşama nedenselliği çıkarılmaz. Toplam gapte tutarlı kazanım yok.',
              '', '**Kapasite etkisi:** D yalnız page allowance\'ını960KiB→4MiB yaptı; descriptor64KiB. Daha büyük cache, ölçülen toplam gapte tekrarlanabilir kazanım sağlamadı. Kapasite tahliyeleri ve descriptor tahliyeleri sürdü; sıfır oversized/legacy fallback ölçüldü. Global hit/miss toplamları bütün API/oracle sorgularını ve farklı sayıda background turunu kapsar; ilk marker başına oran diye sunulmaz.',
              '', f"**Yerleşim sınırı:** strict envanterde canlı run sayısı deney başında{run_counts['pre-A']}, normal bloklar sonrasında{run_counts['post-normal-A']}, havuz sonrasında{run_counts['final-K']}. K gap p95i12,338→32,385ms ilerledi. Aynı mantıksal50k/yazma havuzu, aynı fiziksel yerleşim demek değil. Sıra dengesi bu ilerlemeyi tamamen ayrıştırmaz; büyümenin bu turdaki kesin neden payları bilinmiyor. Yeni owner/tahliye koşullarıyla uyumlu gözlem, tek başına nedensellik kanıtı değildir. Eski profilli~30ms veya profilsiz~15ms ile birebir kazanç iddiası yok.",
              '', '### Yazma ve bellek bedeli','', '|Kol|Okuyucusuz ops/s|Yüklü pooled ops/s|Korunum %|Motor-only fresh idle MiB aralığı|Warm idle MiB aralığı|','|---|---:|---:|---:|---|---|']
    for arm in ['K','A','D','S']:
        v=a['throughput'][arm]
        r=a['ram'].get(arm)
        f=lambda k:f"{min(r[k]):.3f}–{max(r[k]):.3f}" if r else 'ölçülmedi'
        lines.append(f"|{arm}|{v['control_ops_s']:.2f}|{v['ops_s']:.2f}|{v['retention']*100:.3f}|{f('idle')}|{f('idle-warm')}|")
    lines += ['', 'A ve D mutlak ops/s olarak K\'ya yakın/SQLite\'ın üstünde; buna rağmen kendi okuyucusuz kontrollerine göre korunumları SQLite\'ın85,103% pooled oranını geçmiyor. Bu kapı KALDI; küçük farklar için eşik değiştirilmedi. Okuyucusuz kontrol mutlak tavan sayılmaz.',
              '', '4R+W phase peak private farkları, model/oracle/history DAHİL: A−K ' + ', '.join(f"{x-y:+.3f}" for x,y in zip(a['ram']['A']['loaded_model_included'],a['ram']['K']['loaded_model_included']))+'MiB; D−K '+', '.join(f"{x-y:+.3f}" for x,y in zip(a['ram']['D']['loaded_model_included'],a['ram']['K']['loaded_model_included']))+'MiB. Bu değerler motor-only maliyet diye yorumlanmaz; loaded motor-only ayrımı ölçülmedi. A için bütün süreç peak artmaması kanıtlanmadı.',
              '', 'A\'nın normal-page resident+in-flight rezervasyon sayacı üst sınırı983040B; sonunda in-flight0. Ham+decoded kapasite, deque kapasitesi ve sabit metadata payı kabul öncesi kontrol edilir. Metadata yeniden ayırmasında eski+yeni header payı da ön kontroldedir; sayaç allocator/RSS\'nin tam anlık tepesi değildir. Legacy/oversized seri istisnası bu korpusta0. D\'de WORKING_POOL baseline\'ı değişmedi;4MiB cache onun2MiB baseline\'ı içine sığmış gibi hard-cap iddiası yok.',
              '', 'CPU tablo değerleri parent+child bütün native threadlerin toplamıdır; opening/oracle/yazma ve farklı gerçek sorgu sayıları dahildir. Saf sorgu CPU payı veya scheduler/I/O/antivirüs nedeni ölçülmedi. Request CPU profilleri ayrı örneklerdir; tipler eşleşmediği için tek-aşama hızlanması diye genellenmez.',
              '', '### Doğrulama ve kanıt sınırı','',
              'Default ve K/A/D diagnostic testleri; fmt, default/diagnostic clippy -D warnings;15 tanıklı process-kill/reopen ve frozen eski okuyucu geçti. Kill tanıkları strict-open+oracle SONRASINDA; in-flight I/O/commit veya güç kesintisi iddiası yok. Yeni test owner ayrımı/eşzamanlı sayfa, CRC hata, dolu slotta cancel ve panic sonrasında RAII bırakmayı denetler; Condvar içine girme anı ayrıca tanıklanmadı. Strict CRC/kimlik/sıra kodu aynen kullanılır. Cancel OS read\'i zorla kesmez.',
              '', 'Bir büyük JSON satırını Get-Content ile okumaya çalışan yardımcı izleyici takılıp sonlandırıldı; native benchmark sonlandırılmadı. O izleme olayının zamanlamaya etkisi bilinmiyor; ilk SQLite kolu seçilerek tekrar edilmedi. Sonraki izleme1024B kuyrukla sınırlıydı. [Olay notu](benchmarks/'+P+'-monitor-note.json).',
              '', 'Ingest60MiB yeniden ölçülmedi; yeni ingest/reset/kopya yok. Test sonuçları yeni ingeste veya uzun snapshot/gerçek-soğuk koşula genellenmez. Kaynak/corpus/0.1 fixture koruma hashleri ve ölçülen ikili/source SHA\'ları sabit kaldı. Havuzlar temizlendi, ana50k korundu.',
              '', '[41 sorgunun bütün blok tabloları](benchmarks/'+P+'-normal-table.tsv) · [İşlem tipi tabloları](benchmarks/'+P+'-pool-types.tsv) · [Ayrı profiller/ortak-ordinal ek analiz](benchmarks/'+P+'-analysis.json). Ortak-ordinal kesit yalnız ek analizdir; ana kapı kohortu/estimatorü değiştirilmedi.',
              '', '**DURULDU.** A/D yalnız diagnostic opt-in olarak kalır; varsayılan K. B/C veya başka aday, ayar, tekrar turu yok;100lü/C/2,6sn/WAL/File::open/zx/reclaim-default/C ABI başlamadı.']
    lines += ['', '### Kalan normal41 sorgular — yuvarlanmamış değerle karar','',
              '|Blok|Kol|Sorgu|Idle p95 ms|Loaded p95 ms|Sınır ms|','|---|---|---|---:|---:|---:|']
    for x in a['normal_failures']:
        lines.append(f"|{x['pair']}|{x['arm']}|{x['text']}|{x['idle_p95']:.6f}|{x['loaded_p95']:.6f}|{x['limit']:.6f}|")
    lines += ['', '### Ayrı tanı örnekleri — ortalama ms /64 örnek','',
              '|Blok|Kol/tip|Cache acquire|Yeni slot wait|Read/CRC/decode|Posting setup|Cache acquire çağrı|Parent+child CPU|',
              '|---|---|---:|---:|---:|---:|---:|---:|']
    for x in a['profiles']:
        if x['arm']=='S':continue
        lines.append(f"|{x['pair']}|{x['arm']} / {','.join(x['kinds'])}|{x['cache_wait']:.3f}|{x['slot_wait']:.3f}|{x['decode']:.3f}|{x['posting']:.3f}|{x['cache_wait_calls']:.1f}|{x['parent_child_cpu_ms']:.3f}|")
    lines += ['', 'Tipler farklı, süreler nested; doğrudan eklenmez veya eşleşmiş CPU/A-B kanıtı sayılmaz. Cache guard hold\'un tüm LRU/descriptor işlerini kapsayan ayrı bir süre sayacı yok; decode süresi K/D\'de guard içinde, A\'da dışındadır. Bu sınır bilinmeyen hold payını tahminle doldurmaz.',
              '', '### Ek ortak-ordinal kesit — ana kapıyı değiştirmez','',
              '|Blok|Her kolda aynı N|A−K gap p95 ms|D−K gap p95 ms|','|---|---:|---:|---:|']
    for pair in range(4):
        v={x['arm']:x for x in a['matched_secondary'] if x['pair']==pair}
        lines.append(f"|{pair}|{v['K']['n']}|{v['A']['gap']['p95']-v['K']['gap']['p95']:+.4f}|{v['D']['gap']['p95']-v['K']['gap']['p95']:+.4f}|")
    lines += ['', 'Dört koldan herhangi birinde profillenmiş olan ordinal slot bütün kollardan çıkarılır;3840 ortak işlemin tamamı kullanılır. Sonuç yine2/4 yönlüdür. Bu ek analiz ana4032 örnek/kolu, gate\'i veya estimatorü sonradan değiştirmez; ham kayıtların tamamı saklanır.']
    audit=L/f'{P}-audit.json'
    if audit.exists():
        proof=read(audit)
        lines+=['', '### Tamamlanan doğrulama sayıları','',
                f"Core default{proof['tests']['test']}; diagnostic K{proof['tests']['diag-tests-K']}, D{proof['tests']['diag-tests-D']}, A{proof['tests'].get('post-tests-A',proof['tests']['tests-A'])}. Example/SQLite sayıları audit ve test loglarında ayrı tutulur. Root fmt/default ve diagnostic clippy exit0. SQLite comparator clippy exit0; no-default FDB dependency\'sindeki mevcut Pages::empty dead-code uyarısı logda korunur.",
                '', '[Son kanıt denetimi](benchmarks/'+P+'-audit.json).']
    (L/f'{P}-closing.md').write_text('\n'.join(lines)+'\n',encoding='utf-8')
    report.main()
    print(json.dumps(dict(gates=a['gates'],throughput=a['throughput'],manifest_live_runs=run_counts),ensure_ascii=False))
if __name__=='__main__':main()
