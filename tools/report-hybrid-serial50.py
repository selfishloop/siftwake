import json,math,statistics,csv
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='hybrid-serial50-20261030'
def pct(v,p=.95):return sorted(v)[math.ceil(len(v)*p)-1]
def main():
    d=json.loads((L/f'{P}-design.json').read_text(encoding='utf-8'));assert d['stage']=='completed';cases=json.loads((L/'trwiki-cap200k-20261029-v3-catalog.json').read_text())['cases'];rows=[];tails=[];rates={};raw=[];queue_values={}
    for run in d['completed']:
        logs=list(map(json.loads,(L/run['log']).read_text(encoding='utf-8').splitlines()));load=next(x for x in logs if x['phase']=='snapshot_load');assert load['oracle'] and load['commits']==16384 and load['failure'] is None
        values=[x['commit_ms'] for x in load['writes']];tails.append(dict(role=run['role'],pair=run['pair'],p95=pct(values),p99=pct(values,.99),p999=pct(values,.999),max=max(values),over100=sum(x>100 for x in values),n=len(values),memory=load['memory'],stats=logs[0]['stats']))
        queue_values.setdefault((run['role'],'control' if load['control'] else 'loaded'),[]).extend(values)
        rates.setdefault((run['role'],'control' if load['control'] else 'loaded'),[]).append(dict(ops=load['mutations'],ms=load['writer_elapsed_ms']))
        if load['control']:continue
        phases={x['phase']:x for x in logs if x['phase'] in ['snapshot_idle_single','snapshot_idle_four','snapshot_load']};sets={}
        for phase,x in phases.items():
            assert x['oracle'];z={}
            for reader in x['results']:
                for q in reader['records']:
                    if phase=='snapshot_load' and not q['writer_active']:continue
                    z.setdefault(q['case_id'],[]).append(q['engine_ms'])
            sets[phase]=z
        for i in range(41):
            one=sets['snapshot_idle_single'][i];idle=sets['snapshot_idle_four'][i];active=sets['snapshot_load'][i];a=pct(idle);b=pct(active);limit=30 if a<=30 else a*2
            row=dict(pair=run['pair'],role=run['role'],case_id=i,text=cases[i]['text'],one_p95=pct(one),idle_p95=a,loaded_p95=b,limit=limit,passed=b<=limit,one_n=len(one),idle_n=len(idle),loaded_n=len(active));rows.append(row);raw.append(dict(**row,one_samples=one,idle_samples=idle,loaded_samples=active))
    throughput=[]
    for role in ['before','after','sqlite']:
        rate={arm:sum(x['ops'] for x in rates[role,arm])*1000/sum(x['ms'] for x in rates[role,arm]) for arm in ['control','loaded']};throughput.append(dict(role=role,**rate,ratio=rate['loaded']/rate['control']))
    decisions=[dict(pair=i,role=role,passed=sum(x['passed'] for x in rows if x['pair']==i and x['role']==role),failed=sum(not x['passed'] for x in rows if x['pair']==i and x['role']==role),failed_cases=[x['case_id'] for x in rows if x['pair']==i and x['role']==role and not x['passed']]) for i in range(4) for role in ['before','after','sqlite']]
    pooled_tails=[dict(role=role,phase=phase,n=len(v),p95=pct(v),p99=pct(v,.99),p999=pct(v,.999),max=max(v),over100=sum(x>100 for x in v)) for (role,phase),v in queue_values.items()]
    result=dict(rows=rows,raw=raw,tails=tails,pooled_tails=pooled_tails,throughput=throughput,decisions=decisions,A_after41=all(x['passed'] for x in rows if x['role']=='after'),other_gates='CPU/negative/engineRAM separate; no global pass from A alone')
    (L/f'{P}-summary.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf-8')
    with (L/f'{P}-cases.tsv').open('w',encoding='utf-8',newline='') as f:w=csv.DictWriter(f,fieldnames=list(rows[0]),delimiter='\t');w.writeheader();w.writerows(rows)
    lines=['# Hibrit seri devam önce/sonra kabul tablosu —30 Ekim2026','','Tek frozen FDB ikilisinde before=blocking,after=non-blocking try+aynı seri frontier. Dört matched çiftte before/SQLite/after ve ters sıra eşit. Her FDB rolü4 kez;SQLite ortak4 kez. Ek üç16k okuyucusuz kontrol throughput paydası. Aynı sabitDBler reset edilmeden,aynı50k/881756DA…corpus,41 sorgu,1R12round/4R3round ve16384tekli writer+oracle. Fiziksel yerleşim/sequence yazı/bakımla ilerler;bit-aynı yerleşim değildir. Idle N12 olduğundan p95=max. Derleme/test bu kollarla paralel çalışmadı;warm OS cache,cold eviction yok. No new thresholds.','','## Her çiftin sorgu kapısı','','|Çift|Rol|Geçti /kaldı|Kalan case ID|','|---|---|---|---|']
    for x in decisions:lines.append(f"|{x['pair']}|{x['role']}|{x['passed']} /{x['failed']}|{x['failed_cases']}|")
    lines+=['','## Yazma throughput (pooled)','','|Rol|Kontrol ops/s|4R ops/s|Korunum|','|---|---:|---:|---:|']
    for x in throughput:lines.append(f"|{x['role']}|{x['control']:.3f}|{x['loaded']:.3f}|{x['ratio']*100:.3f}%|")
    lines+=['','## Yazma kuyrukları','','|Çift/rol|p95 /p99 /p99,9 ms|Max ms|>100ms/N|Benchmark peak private MiB|','|---|---|---:|---|---:|']
    for x in tails:lines.append(f"|{x['pair']}/{x['role']}|{x['p95']:.3f}/{x['p99']:.3f}/{x['p999']:.3f}|{x['max']:.3f}|{x['over100']}/{x['n']}|{x['memory']['os_lifetime_peak_private_commit_bytes']/1048576:.3f}|")
    lines+=['','## Pooled kuyruk ve SQLite oranları','','Her oran aynı fazdaki SQLite yüzdeliğine bölünür; 1 altı FDB gecikmesinin daha düşük olduğunu gösterir. Loaded her rol için 65.536, control 16.384 commit. Örnekleri birleştirip yeniden yüzdelik hesaplandı; yüzdeliklerin ortalaması alınmadı.','','|Faz/rol|p95 / p99 / p99,9 / max ms|SQLite oranı p95 / p99 / p99,9 / max|>100 ms / N|','|---|---|---|---|']
    for x in pooled_tails:
        sql=next(v for v in pooled_tails if v['role']=='sqlite' and v['phase']==x['phase'])
        fields=['p95','p99','p999','max']
        lines.append(f"|{x['phase']}/{x['role']}|"+'/'.join(f'{x[k]:.3f}' for k in fields)+'|'+'/'.join(f'{x[k]/sql[k]:.3f}' for k in fields)+f"|{x['over100']}/{x['n']}|")
    old=json.loads((L/'idle-spread50-20261030-summary.json').read_text(encoding='utf-8'))
    lines+=['','## Idle yayılımı: eski rapor ve bu tur yan yana','','|Case|Önceki rapor min/medyan/max ms|Bu tur before|Bu tur after|','|---|---|---|---|']
    for i in [13,16,25]:
        prev=next(x for x in old['spread'] if x['engine']=='fdb' and x['case_id']==i);cols=[f"{prev['min']:.3f}/{prev['median']:.3f}/{prev['max']:.3f}"]
        for role in ['before','after']:
            v=[x['idle_p95'] for x in rows if x['role']==role and x['case_id']==i];cols.append(f'{min(v):.3f}/{statistics.median(v):.3f}/{max(v):.3f}')
        lines.append(f'|{i}:{cases[i]["text"]}|'+ '|'.join(cols)+'|')
    lines+=['','## Üreticisi/timmy/akarsu ham satırları','','|Çift/rol|Case|1R idle p95|4R idle p95|4R loaded p95|Sınır|Karar|','|---|---|---:|---:|---:|---:|---|']
    for x in rows:
        if x['case_id'] in [13,16,25]:lines.append(f"|{x['pair']}/{x['role']}|{x['text']}|{x['one_p95']:.3f}|{x['idle_p95']:.3f}|{x['loaded_p95']:.3f}|{x['limit']:.3f}|{'GEÇTİ' if x['passed'] else 'KALDI'}|")
    lines+=['','Bütün41 sorgu,35–38ve case34 dahil1R/idle/loadedbeforeafter [TSV](benchmarks/'+P+'-cases.tsv);herörnek [JSON](benchmarks/'+P+'-summary.json). [Frozen SHA/ham log/exit](benchmarks/'+P+'-design.json). Benchmark process RAM model/oracle/history/records içerir ve query hızına göre kayıt sayısı değişebilir; motorun seri devam ek maliyeti diye çıkarılmaz. Bunun için ayrı model/profil hariç4R engineRAM kolu vardır. CPU parent+child blokları ve loaded requestCPU ayrıdır. the zx user kararıyla ana41/kapı dışında ayrı metadata oracle/tarama/CPUda. Genel kapı kararı bütün bu ölçümler sonrası verilir;kötü veri atılmadı.']
    (R/'docs/HYBRID-SERIAL-ACCEPTANCE-50K-20261030.md').write_text('\n'.join(lines)+'\n',encoding='utf-8');print(json.dumps(dict(A_after41=result['A_after41'],decisions=decisions,throughput=throughput),ensure_ascii=False))
if __name__=='__main__':main()
