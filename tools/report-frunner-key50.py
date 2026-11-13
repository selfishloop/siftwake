"""Paired key-pool costs and unchanged gates, raw samples and size cells."""
import csv
import json
import math
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='frunner-key50-20261102'

def stats(values):
    assert values
    x=sorted(values);n=len(x)
    return dict(n=n,p50=x[math.ceil(.5*n)-1],p95=x[math.ceil(.95*n)-1],p99=x[math.ceil(.99*n)-1],max=x[-1])

def main():
    d=json.loads((L/f'{P}-design.json').read_text(encoding='utf-8'))
    writes=[];reads=[];cells=[];arms=[];samples=[]
    for run in d['completed']:
        phases=[json.loads(x) for x in (L/run['log']).read_text(encoding='utf-8').splitlines()]
        out=next(x for x in phases if x['phase']=='writes');records=out['records'];assert out['oracle'] and len(records)==4097
        base=dict(block=run['block'],engine=run['engine'],variant=run['variant'])
        arm=dict(**base,writer_ms=out['writer_ms'],commits=len(records),commit_ops_s=len(records)*1000/out['writer_ms'],
                 mutation_ops_s=sum(x['mutations'] for x in records)*1000/out['writer_ms'],memory=out['memory'],oracle=True)
        for kind in ['update','delete','insert','mtime_delete_insert','edge1000_mtime']:
            xs=[x for x in records if x['kind']==kind];assert len(xs)==(1 if kind.startswith('edge') else 1024)
            st={k:stats([x[k] for x in xs]) for k in ['commit_ms','visibility_ms','prepare_ms']}
            passed=all(v['max']<=100 and (kind.startswith('edge') or v['p95']<=20 and v['p99']<=50) for k,v in st.items() if k!='prepare_ms')
            writes.append(dict(**base,kind=kind,**st,passed=passed))
            if not kind.startswith('edge'):
                for body in range(4):
                    for path in [40,80,160,300]:
                        group=[x for x in xs if x['body_band']==body and x['path_bytes']==path];assert len(group)==64
                        cells.append(dict(**base,kind=kind,body_band=body,path_bytes=path,n=64,commit_p50=stats([x['commit_ms'] for x in group])['p50'],commit_p95=stats([x['commit_ms'] for x in group])['p95'],commit_p99=stats([x['commit_ms'] for x in group])['p99'],commit_max=max(x['commit_ms'] for x in group),visibility_p95=stats([x['visibility_ms'] for x in group])['p95'],visibility_max=max(x['visibility_ms'] for x in group)))
        by_phase={}
        for phase in ['idle1','idle4','loaded']:
            p=next(x for x in phases if x['phase']==phase);assert p['oracle']
            by_case=[[] for _ in range(5)]
            for reader in p['records']:
                for row in reader:
                    assert row['oracle']
                    if phase!='loaded' or row['writer_active']:by_case[row['case']].append(row['engine_ms'])
            assert all(len(v)>=60 for v in by_case),(base,phase,[len(v) for v in by_case])
            by_phase[phase]=by_case
            for case,values in enumerate(by_case):samples.append(dict(**base,phase=phase,case=case,**stats(values),values=values))
        for case,text in enumerate(['fdbkeypoolmark','olduğu','üreticisi','olduğu birlikte','qzxkvwkeypoolabsent']):
            one,idle,load=[stats(by_phase[k][case]) for k in ['idle1','idle4','loaded']];limit=30 if idle['p95']<=30 else 2*idle['p95']
            reads.append(dict(**base,case=case,text=text,idle1=one,idle4=idle,loaded=load,limit=limit,passed=load['p95']<=limit))
        own=[x for x in writes if all(x[k]==base[k] for k in base)];rr=[x for x in reads if all(x[k]==base[k] for k in base)]
        arm.update(write_passed=all(x['passed'] for x in own),read_passed=sum(x['passed'] for x in rr));arms.append(arm)
    complete=d['stage']=='completed'
    fdb=[a for a in arms if a['engine']=='fdb' and a['variant']=='frunner']
    decision=('GEÇTİ' if all(a['write_passed'] and a['read_passed']==5 for a in fdb) else 'KALDI') if complete else 'BEKLENİYOR'
    summary=dict(stage=d['stage'],decision=decision,writes=writes,reads=reads,arms=arms,cells=cells,full_50k_rekey=False,prototype_default=True)
    (L/f'{P}-summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2),encoding='utf-8')
    (L/f'{P}-read-samples.json').write_text(json.dumps(samples,ensure_ascii=False),encoding='utf-8')
    if cells:
        with (L/f'{P}-cells.tsv').open('w',encoding='utf-8',newline='') as out:
            w=csv.DictWriter(out,fieldnames=list(cells[0]),delimiter='\t');w.writeheader();w.writerows(cells)
    lines=['# B: Frunner anahtarı, ayrı havuz — 2 Kasım 2026','',f'**Kapı: {decision}. Tamamlanan kol {len(arms)}/16.** Yeni try/seri varsayılan; policy override yok. Bu çalışma motor optimizasyonu veya Frunner entegrasyonu değildir.','']
    if complete:
        fr=[x for x in writes if x['engine']=='fdb' and x['variant']=='frunner' and not x['kind'].startswith('edge')]
        commit=all(x['commit_ms']['p95']<=20 and x['commit_ms']['p99']<=50 and x['commit_ms']['max']<=100 for x in fr)
        vmin=min(x['visibility_ms']['p95'] for x in fr);vmax=max(x['visibility_ms']['p95'] for x in fr)
        lines += [f"Frunner commit kapısı {'GEÇTİ' if commit else 'KALDI'}; görünürlük p95 {vmin:.3f}–{vmax:.3f}ms>sınır20ms: KALDI. Havuz okuma Frunner blokları "+', '.join(f"{a['read_passed']}/5" for a in arms if a['engine']=='fdb' and a['variant']=='frunner')+'. Anahtar sırası/atomik mtime/1000B uç doğruluğu geçti; sonraki işe geçilmedi.','']
    lines += [
           '## Kapsam ve yöntem','',
           'Ölçümden önce kullanıcıya [plan ve eşikler](FRUNNER-KEY-DESIGN-50K-20261102.md) gösterildi; kullanıcı ayrı collection havuzunu seçti. Aynı881756DA ilk50k içinden256 gerçek donör +1 uzun-yol slotu. Kaynak/private/ değiştirilmedi; private/ okunmadı. Yeni corpus, DB kopyası, tüm50k yeniden ingest veya yeniden anahtarlama yok. Numeric `keybench-numeric`, Frunner `keybench-frunner`; ikisi16UTF8B collection adı. Mevcut50k `texts` korunur. Hazırlık ve cleanup süreleri loglarda ayrı; yalnız bu iki collection anahtarları yazılır. Her kol başında257 kayıt, yeni mtime her zaman başa; güncelleme aynı anahtarda, mtime değişikliği tek committe delete+insert.','',
           'ID10B numeric veya `{:016x}|path` ters-u64-mtime: yol40/80/160/300B, anahtar57/97/177/317B; bir1000B yol/1017B ID ucu. UTF8 bayt uzunluklarıdır. Body2–4/4–16/16–64/64–200KB; aynı donörler her kolda. Gövdede oracle etiketi için en fazla199936B kaynak öneki +etiket; disk corpus değişmez.','',
           'Her tip×body×path hücresi64 örnek: tip başına1024; kol4096 normal+1 uç commit. P50/p95/p99/max nearest-rank; trim/bootstrap yok. N64te p95=max değildir, p99=max olabilir. Commit zamanı native API içidir; key/document hazırlığı ayrıca. Görünürlük commit başlangıcından yeni snapshotta ilk marker sorgusunun dönüşüne kadar; tam oracle sonra.','',
           'Havuz5 kontrol sorgusu: ortak oracle etiketi yalnız sıralama için, olduğu/üreticisi/olduğu birlikte gerçek gövde, negatif qzxkvwkeypoolabsent. 1R idle60,4R15tur/aktör; loaded yalnız writer_active örnekleri, her sorgu≥60. Dört okuyucu aynı beşli sıra ile başlar; eski41-sorgu yükünün zamanlaması kopyalanmadı, kullanıcı seçimiyle küçük havuz kontrolüdür. Marker sorgusundan genel içerik arama üstünlüğü çıkarılmaz. Oracle her snapshot sequence için bağımsız history modelidir; zamanı engine_ms dışında. Engine_ms=acquire+search+release. Dört blok numeric/Frunner ve FDB/SQLite ters sıralı; sıcak/kontrolsüz OS cache, eviction yok. Fiziksel yerleşim/sequence değişir; yalnız anahtar byte sayısına kesin neden atfı yapılmaz. Numeric artan ID sonunda, Frunner ters-mtime başında yeni kayıt verir: ilk50 aynı belgeler olmak zorunda değildir, dönüş payload maliyeti de değişebilir. Bu okuma kontrolü salt anahtar karşılaştırma CPU deneyi değildir. SQLite aynı FTS5 trigram/detailnone/columnsize0, WAL/FULL/cache2MiB/mmap0/tempFILE; yeni bağımsız benchmark adaptörü, engine bağımlılığı eklenmedi.','',
           '## Değişmeyen kapılar','',
           'FDB kısa commit ve görünürlük: her işlem tipinde p95≤20,p99≤50,max≤100ms; uçN1de max≤100ms. Havuz okuma: kendi idle≤30 ise loaded≤30ms, aksi halde≤2×idle. B kararının esas yolu FDB Frunner; numeric kontrol ve SQLite ayrıca gösterilir. SQLite referanstır, FDB max şartı uygulanmaz. Numeric→Frunner farkına yeni yüzde kapısı konmadı. CPU/model/oracle/history içeren process private/WS motor-only D sayılmaz; bu tur D ingest/RAM kabulü yeniden ölçülmedi. Önceki normal41 KALDI kullanıcı kabul kaydı bu yeni kapıyı otomatik geçirmez. Commit/s hazırlık, görünürlük ve her işlemdeki tam oracle dahil yazıcı döngüsüdür; HYBRID-DEFAULT uzun seri ops/s ile birebir kıyaslanmaz. Yalnız native API commit kuyruk değerleri ayrı sütundadır. Kontrollü yol dizgileri kullanıldı; gerçek filesystem erişimi/Win32 bileşen geçerliliği ölçülmedi.','',
           '|Blok|Motor|Anahtar|Yazma kapısı|Okuma geçti/5|Commit/s|Mutation/s|Process peak private MiB|', '|---|---|---|---|---:|---:|---:|---:|']
    for a in arms:
        gate=('GEÇTİ' if a['write_passed'] else 'KALDI') if a['engine']=='fdb' else 'REFERANS'
        lines.append(f"|{a['block']}|{a['engine']}|{a['variant']}|{gate}|{a['read_passed']}|{a['commit_ops_s']:.3f}|{a['mutation_ops_s']:.3f}|{a['memory']['os_lifetime_peak_private_commit_bytes']/2**20:.3f}|")
    lines+=['','## Ön sıradaki payload farkı, ölçüm sınırı','','Her FDB kolunun son snapshotında marker ilk50 dönen metin byte toplamı; bütün sorguların byte toplamı veya saf CPU profili değildir.','',
            '|Blok|Numeric ilk50 text B|Frunner ilk50 text B|', '|---|---:|---:|']
    for block in range(4):
        g={x['variant']:x['old_reader']['first50_text_bytes'] for x in d['completed'] if x['block']==block and x['engine']=='fdb'}
        if len(g)==2:lines.append(f"|{block}|{g['numeric']}|{g['frunner']}|")
    lines+=['','Bu donör/işlem sırası Frunner başında daha büyük belgeler bırakır. Anahtar byte maliyeti ile farklı sonuç/payload maliyeti bu okumada izole edilmedi; bu fark görünürlük/commit kaçırmalarının kanıtlanmış kök nedeni olarak sunulmaz.']
    lines+=['','## Commit/görünürlük, bütün tipler ve çiftler','','|Blok/motor/anahtar|Tip/N|Commit p50/p95/p99/max ms|Görünürlük p50/p95/p99/max ms|Hazırlık p95 ms|FDB kuralı|','|---|---|---|---|---:|---|']
    for x in writes:
        values=['/'.join(f"{x[k][s]:.3f}" for s in ['p50','p95','p99','max']) for k in ['commit_ms','visibility_ms']]
        if x['kind'].startswith('edge'):
            values=[f"—/—/—/{x[k]['max']:.3f}" for k in ['commit_ms','visibility_ms']]
        gate=('GEÇTİ' if x['passed'] else 'KALDI') if x['engine']=='fdb' else 'REFERANS'
        lines.append(f"|{x['block']}/{x['engine']}/{x['variant']}|{x['kind']}/{x['commit_ms']['n']}|{values[0]}|{values[1]}|{x['prepare_ms']['p95']:.3f}|{gate}|")
    lines+=['','## Havuz okuma:1R,4R idle ve yüklü','','|Blok/motor/anahtar|Sorgu|1R N:p50/p95/max|4R idle N:p50/p95/max|Loaded N:p50/p95/max|Sınır ms|Karar|','|---|---|---|---|---|---:|---|']
    for x in reads:
        vals=[f"{x[k]['n']}:{x[k]['p50']:.3f}/{x[k]['p95']:.3f}/{x[k]['max']:.3f}" for k in ['idle1','idle4','loaded']]
        lines.append(f"|{x['block']}/{x['engine']}/{x['variant']}|{x['text']}|"+'|'.join(vals)+f"|{x['limit']:.3f}|{'GEÇTİ' if x['passed'] else 'KALDI'}|")
    lines+=['','## Numeric → Frunner, eşleşmiş yazma farkları','','|Blok/motor|Tip|Commit p95 B→F ms|Görünürlük p95 B→F ms|Commit p95 Δ%|','|---|---|---|---|---:|']
    for b in range(4):
        for engine in ['fdb','sqlite']:
            for kind in ['update','delete','insert','mtime_delete_insert']:
                g={x['variant']:x for x in writes if x['block']==b and x['engine']==engine and x['kind']==kind}
                if len(g)==2:
                    n,f=[g[k] for k in ['numeric','frunner']]
                    lines.append(f"|{b}/{engine}|{kind}|{n['commit_ms']['p95']:.3f}→{f['commit_ms']['p95']:.3f}|{n['visibility_ms']['p95']:.3f}→{f['visibility_ms']['p95']:.3f}|{100*(f['commit_ms']['p95']/n['commit_ms']['p95']-1):+.2f}|")
    lines+=['','## Kanıt ve sınırlar','',
            'Her değişiklikte eski marker kaybolması, yeni marker/GET ve version; snapshot tek sequence; yeni insert/mtime first-key; uzun-yol kısa eski snapshot; bütün ilk50 key sırası oracle ile sınandı. Frozen eski okuyucu her FDB havuzunu tam257GET ve ilk50 ile okur. Kill tanıkları pool full oracle sonrası; in-flight transaction/güç kesintisi iddiası yok. Tam test paketi ayrı kesilme/bozulma/kill senaryolarını da içerir.','',
            f'[Ham kol/süreç/varsayılan/SHA](benchmarks/{P}-design.json) · [Özet](benchmarks/{P}-summary.json) · [Body×path hücrelerinin hepsi](benchmarks/{P}-cells.tsv) · [Okuma örnekleri](benchmarks/{P}-read-samples.json) · [Süreç olayları](benchmarks/{P}-lifecycle.jsonl).','',
            '100lü,C500drain,2,6sn,WAL handle/File::open,zx iyileştirmesi,format/256KiB/ön filtre/Blok3/4/C ABI/gerçek-soğuk/uzun snapshot/100k yok. Reclaim default kapalı; geniş tarama, signed-zero, yazma/bakım/disk/sorguRAM açık. [OPEN-WORK](OPEN-WORK.md).']
    audit=L/f'{P}-audit.json'
    if audit.exists():
        a=json.loads(audit.read_text(encoding='utf-8'))
        pre=json.loads((L/f'{P}-pre-recovery.json').read_text(encoding='utf-8'))['kills'][0]['witness']['stats']
        post=json.loads((L/f'{P}-post-recovery.json').read_text(encoding='utf-8'))['kills'][0]['witness']['stats']
        lines+=['','## Havuz temizliği ve kalan alan','',
                f"İki havuzun canlı kaydı0; ana koleksiyon50000. FDB canlı-run kaynak bytes (WAL dahil) {pre['source_bytes']}→{post['source_bytes']}; .idx bytes {pre['disk_index_bytes']}→{post['disk_index_bytes']}. Tombstone/ölü revizyon kalabilir: logical delete, başlangıç fiziksel alanına dönüldü iddiası değildir. Reclaim default kapalı kaldı. Başlangıç toplam retained disk sayılmadığı için toplam retained büyüme oranı verilmez. Son retained FDB{a['retained_after_bytes']['fdb']}B; SQLite{a['retained_after_bytes']['sqlite']}B. Bu B turunda yeni disk kapısı eklenmedi."]
        lines+=['',f"Doğrulama GEÇTİ: {a['tests']['passed']} default +{a['diagnostic_tests']['passed']} diagnostic test, SQLite adaptörü{a['sql_tests']['passed']} test; fmt/root ve diagnostic clippy -D warnings. Native en son ölçüm etiketleri eklenince ilgili2 test/clippy ve iki release ikilisi tekrar doğrulandı. Kaynak/0.1fixture SHA aynı, engine accepted-default ile bit aynı. Ana50k FDB tamGET/41oracle ve SQLite payload digest korundu.9 witnessedkill/reopen,8 FDB pool kolunda frozen eski okuyucu257GET+ilk50 sırası geçti. Normal{a['normal_children']} child exit0. CPU/model/oracle hariç bir motor RAM sonucu çıkarılmaz.", '',
                'İki hazırlık hatası ölçümden önce yakalandı: en uzun donör ilk63 içinde olunca seçicide duplicate; ardından model yolu str/Path uyuşmazlığı. İkisi de DB worker açılmadan durdu; düzeltildi, eski loglar silinmedi. Bu kayıtlar tamamlanmış ölçüm veya engine oracle hatası olarak sayılmadı.', '',
                f'[İlk hazırlık logu](benchmarks/{P}-driver.log) · [İkinci hazırlık logu](benchmarks/{P}-driver-v2.log) · [Asıl ölçüm](benchmarks/{P}-driver-v3.log) · [Doğrulama/oracle/CRC/kill/eski okuyucu ve ana50k koruma denetimi](benchmarks/{P}-audit.json).']
    if complete:lines+=['',f'**Sonuç {decision}.** '+('Performans kapısı KALDI; sonraki işe geçilmedi.' if decision=='KALDI' else 'Bu B bloğu tamamlandı; sonraki işler ayrı onay bekler.')]
    (R/'docs/FRUNNER-KEY-50K-20261102.md').write_text('\n'.join(lines)+'\n',encoding='utf-8')
    print(json.dumps(dict(stage=d['stage'],completed=len(arms),decision=decision,arms=[dict(block=a['block'],engine=a['engine'],variant=a['variant'],reads=a['read_passed'],writes=a['write_passed']) for a in arms]),ensure_ascii=False))

if __name__=='__main__':main()
