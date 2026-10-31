import json,math,argparse
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks'
def pct(xs,p=.95):return sorted(xs)[math.ceil(len(xs)*p)-1]
def records(x):return [r for reader in x['results'] for r in reader['records']]
def main():
    ap=argparse.ArgumentParser();ap.add_argument('--batch',type=int,required=True);a=ap.parse_args();P=f'delta-load50-20261030-b{a.batch}'
    d=json.loads((L/f'{P}-design.json').read_text(encoding='utf-8'));g=json.loads((L/f'{P}-gate.json').read_text(encoding='utf-8'));cases=json.loads((L/'trwiki-cap200k-20261029-v3-catalog.json').read_text())['cases'];loads={};slow=[];waits=[]
    for run in d['completed']:
        if not run['mode'].startswith('snapshot'):continue
        rows=list(map(json.loads,(L/run['log']).read_text(encoding='utf-8').splitlines()));x=next(x for x in rows if x['phase']=='snapshot_load');assert x['oracle'];loads[run['engine'],run['label']]=x
        for w in x['writes']:
            if w['commit_ms']>500:slow.append(dict(engine=run['engine'],label=run['label'],**w,stage_evidence='profile arm trace if same sequence; normal-arm cause unknown'))
        z=[r for r in records(x) if r['writer_active']]
        if z:waits.append(dict(engine=run['engine'],label=run['label'],reads=len(z),admission_contended=sum(r['admission']['contended'] for r in z),max_admission_wait_ms=max(r['admission']['wait_ns'] for r in z)/1e6,writer_wait_count=sum(r['writer_waited'] for r in z),max_writer_wait_ms=max(r['writer_wait_ns'] for r in z)/1e6))
    (L/f'{P}-slow-waits.json').write_text(json.dumps(dict(over500=slow,waits=waits),ensure_ascii=False,indent=2),encoding='utf-8')
    lines=[f'# 4R + {a.batch}li yazıcı — 30 Ekim 2026','',f"**Kapı: {'GEÇTİ' if g['passed'] else 'KALDI — sonraki maddelere geçilmedi'}.**",'',
    '881756DA… corpus ilk50k; akışlı API, hybrid200/N50, strict kaynak/indeks CRC, aynı FTS5 trigram SQLite ayarları. Sabit owner slotlar dışında DB yok. Her kol fresh50k aynı planla kuruldu; normal kabul sırası FDB kontrol → SQLite kontrol → SQLite4R → FDB4R. Tek dengeli çift; çoklu çift iddiası yok. Warm OS cache, eviction yok. Her idle/yüklü sorguda bağımsız commit-sequence/payload/model oracle. Oracle search zamanının dışında, ama CPU/I/O yükü olarak koşuda mevcut. Yeni p95 sınırı eklenmedi: idle≤30ms ise loaded≤30ms, diğerleri≤2×idle. Throughput≥%75 ve SQLite pooled üstü. Uzun tekli p95/p99/p99,9≤20/50/100ms; max bilgi.', '',
    '[Ham kollar/SHA/lifecycle](benchmarks/'+P+'-design.json) · [Tam kapı](benchmarks/'+P+'-gate.json) · [>500ms bütün olaylar ve bekleme](benchmarks/'+P+'-slow-waits.json).', '',
    '## Yazma ve kapılar','','|Motor|Kontrol ops/s|4R ops/s|Oran|4R p95 / p99 / p99,9 ms|Max ms|>100ms|','|---|---:|---:|---:|---|---:|---:|']
    for t in g['throughput']:
        e=t['engine'];w=g['writer_tail'][e];lines.append(f"|{e}|{t['rates']['control']:.2f}|{t['rates']['loaded']:.2f}|{t['loaded_control_ratio']*100:.2f}%|{w['p95_ms']:.3f} /{w['p99_ms']:.3f} /{w['p999_ms']:.3f}|{w['max_ms']:.3f}|{w['over100']} ({w['over100_ratio']*100:.5f}%)|")
    lines+=['','## 41 sorgu — her iki motorun bütün sonuçları','','|Case/sorgu|FDB idle / loaded p95 ms|Sınır|Karar|SQLite idle / loaded ms|','|---|---|---:|---|---|']
    for i in range(41):
        f=next(x for x in g['cases'] if x['engine']=='fdb' and x['case_id']==i);s=next(x for x in g['cases'] if x['engine']=='sqlite' and x['case_id']==i);lines.append(f"|{i}: {cases[i]['text']}|{f['idle_p95_ms']:.3f} /{f['loaded_p95_ms']:.3f}|{f['limit_ms']:.3f}|{'GEÇTİ' if f['passed'] else 'KALDI'}|{s['idle_p95_ms']:.3f} /{s['loaded_p95_ms']:.3f}|")
    lines+=['','## Maske etkisi: aynı capped ikilide gerçek yük','','Ana adaydan sonra ayrı maske-reference kolunda yalnız replacement reuse kapalıdır (full fallback), delta capped aynı. Kabul/SQLite throughput havuzuna katılmaz. Bu ilave tek yönlü karşılaştırma keşif niteliğindedir; dengeli tekrarlı neden kanıtı diye sunulmaz. Yeni yazmasız maske koşusu yapılmadı.','','|Case|Full maske loaded p95 ms|Artımlı maske loaded p95 ms|','|---|---:|---:|']
    normal=loads['fdb','pair0-loaded'];ref=loads['fdb','mask-reference']
    for i in range(41):
        x=[v['engine_ms'] for v in records(ref) if v['writer_active'] and v['case_id']==i];y=[v['engine_ms'] for v in records(normal) if v['writer_active'] and v['case_id']==i]
        if x and y:lines.append(f'|{i}|{pct(x):.3f}|{pct(y):.3f}|')
    lines+=['','## Sınırlar ve açık işler','','Process RAM; benchmark/model/oracle/history/katalog ve varsa trace içerir; motor-only D veya≤60/15MiB kapısı diye sunulmaz. Bütün normal/profile >500ms olaylar ham JSON listesinde. Eski2,6sn olayın nedeni bu farklı koşuyla geriye dönük kanıtlanmaz: BİLİNMİYOR. Antivirüs iddiası yok. Maske/Delta oracle, seçili WAL CRC,249 test/fmt/clippy ve3kill/reopen/eski okuyucu kanıtı [delta raporunda](DELTA-FIRST-PAGE-50K-20261030.md). Bu yükteki son modelin recovery ayrıca kaydedilecek.','','A kapısı KALDI ise100lü veya D/B/C/E/WAL düzeltmesine geçilmez. Motor RAM,Frunner anahtarı,500 worker drain,timmy41-vs4/cache/mutex; ayrıca arama regresyonları/case34, disk/bakım, signed-zero ve bilinmeyen eski uçlar açık. Reclaim default kapalı. Biçim/256KiB/ön filtre/Blok3/4/C ABI/gerçek-soğuk/uzun snapshot/100k yok.']
    (R/f'docs/CONCURRENT-DELTA-{a.batch}-50K-20261030.md').write_text('\n'.join(lines)+'\n',encoding='utf-8')
    print(json.dumps(dict(passed=g['passed'],failures=g['failures']),ensure_ascii=False))
if __name__=='__main__':main()
