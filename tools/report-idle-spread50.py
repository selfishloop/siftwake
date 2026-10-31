import json,math,statistics
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='idle-spread50-20261030'
def pct(v):return sorted(v)[math.ceil(len(v)*.95)-1]
def main():
    d=json.loads((L/f'{P}-design.json').read_text(encoding='utf-8'));assert d['stage']=='completed';cases=json.loads((L/'trwiki-cap200k-20261029-v3-catalog.json').read_text())['cases'];rows=[];raw=[]
    for run in d['completed']:
        log=[json.loads(x) for x in (L/run['log']).read_text(encoding='utf-8').splitlines()];idle=next(x for x in log if x['phase']=='snapshot_idle_four');loaded=next(x for x in log if x['phase']=='snapshot_load');assert idle['oracle'] and loaded['oracle'];a={};b={}
        for x,dest in [(idle,a),(loaded,b)]:
            for t in x['results']:
                for q in t['records']:
                    if x is loaded and not q['writer_active']:continue
                    dest.setdefault(q['case_id'],[]).append(q['engine_ms'])
        failures=[]
        for i in range(41):
            initial=pct(a[i]);observed=pct(b[i]);limit=30 if initial<=30 else initial*2
            row=dict(pair=run['pair'],engine=run['engine'],case_id=i,text=cases[i]['text'],idle_n=len(a[i]),loaded_n=len(b[i]),idle_p95=initial,loaded_p95=observed,limit=limit,passed=observed<=limit)
            rows.append(row);raw.append(dict(**row,idle_samples=a[i],loaded_samples=b[i]))
            if not row['passed']:failures.append(i)
        run['query_result']=dict(passed=41-len(failures),failed=len(failures),failure_cases=failures)
    spread=[]
    for e in ['fdb','sqlite']:
        for i in [13,16,25]:
            v=[x['idle_p95'] for x in rows if x['engine']==e and x['case_id']==i];spread.append(dict(engine=e,case_id=i,text=cases[i]['text'],min=min(v),max=max(v),median=statistics.median(v),values=v))
    result=dict(pairs=[dict(pair=x['pair'],engine=x['engine'],**x['query_result']) for x in d['completed']],rows=rows,spread=spread,raw=raw,all_pairs_fdb41=all(x['query_result']['failed']==0 for x in d['completed'] if x['engine']=='fdb'))
    (L/f'{P}-summary.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf-8')
    lines=['# Idle tabanı ve4R+yazıcı — 30 Ekim2026','','Dört çift; sırayla FDB→SQLite,SQLite→FDB,FDB→SQLite,SQLite→FDB. Aynı frozen ikili ve aynı sabit50k DB slotları yeniden kurulmadan kullanıldı. Her child kendi normal kabul düzeninde1R idle,4R idle,16384 tekli writer+4R ve oracle çalıştırdı. Mantıksal sorgu kümeleri aynı; revision token/version ilerler. Normal başlangıç bakımı ve yazmalar fiziksel yerleşimi değiştirir; bu yüzden yayılımın yalnız işletim sistemi gürültüsünden kaynaklandığı iddiası yok. No-reset, sequence/segment sayıları her opened logunda. Aynı anda başka benchmark/build/test çalıştırılmadı. Warm OS cache; cold eviction yok.','','P95 nearest rank. Idle4R/case N12 olduğundan p95 o kolun maksimum örneğidir; loaded p95 kendi writer-active örneklerinden hesaplandı. Yeni eşik yok: idle≤30ms ise loaded≤30ms, diğerleri≤2×idle. SQLite aynı formülle bilgi amaçlı gösterilir; gate FDB içindir. Request edilen minimum3 yerine4 çiftle iki sıra eşit tekrarlandı.','','## Çift kararları','','|Çift|Motor|Geçti / kaldı|Kalan case ID|','|---|---|---|---|']
    for x in result['pairs']:lines.append(f"|{x['pair']}|{x['engine']}|{x['passed']} /{x['failed']}|{x['failure_cases']}|")
    lines+=['','## Idle p95 yayılımı (çiftlerin p95 değerleri)','','|Motor/case|Min ms|Max ms|Medyan ms|Dört değer ms|','|---|---:|---:|---:|---|']
    for x in spread:lines.append(f"|{x['engine']}/{x['case_id']}:{x['text']}|{x['min']:.3f}|{x['max']:.3f}|{x['median']:.3f}|"+', '.join(f'{v:.3f}' for v in x['values'])+'|')
    lines+=['','## Her çiftte odak sorgular','','|Çift/motor|Case|Idle p95 ms|Loaded p95 ms|Sınır ms|Karar|','|---|---|---:|---:|---:|---|']
    for x in rows:
        if x['case_id'] in [13,16,25]:lines.append(f"|{x['pair']}/{x['engine']}|{x['text']}|{x['idle_p95']:.3f}|{x['loaded_p95']:.3f}|{x['limit']:.3f}|{'GEÇTİ' if x['passed'] else 'KALDI'}|")
    lines+=['','Bütün41 case, ham örnekler ve sayımlar [JSON tabloda](benchmarks/'+P+'-summary.json); [program SHA/exit ve ham log dizini](benchmarks/'+P+'-design.json). Hiçbir kötü kol çıkarılmadı. Oynaklık kaynak atfı değildir; cache,yerleşim,worker bekleme,CPU/I/O ayrımı sonraki ayrı diagnostics raporundadır. Process RAM motor-only diye sunulmaz; D ayrıdır. Throughput için bu tekrarlar yeni okuyucusuz control çalıştırmadı; önceki%91,4/%71,1 sonucu yeni ölçülmüş oran sayılmaz.','','100lü/B/C/2,6sn ancak A41/41 sonrası; herhangi bir çiftte kalan sorgu sessizce havuzla kapatılmaz. Kullanıcıya bütün kararlar sunulur. Biçim/256KiB/ön filtre/Blok3/4/C ABI/gerçek-soğuk/uzun snapshot/100k/WAL düzeltmesi yok.']
    (R/'docs/IDLE-SPREAD-50K-20261030.md').write_text('\n'.join(lines)+'\n',encoding='utf-8');print(json.dumps(dict(pairs=result['pairs'],spread=spread),ensure_ascii=False))
if __name__=='__main__':main()
