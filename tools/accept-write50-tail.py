"""Apply the direct human gate change to immutable single-write samples."""
import hashlib
import json
import math
from collections import defaultdict
from pathlib import Path

R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='write50-20261029-v1'
def q(xs,p):return sorted(xs)[math.ceil(p*len(xs))-1]
def main():
    out=L/'write50-tail-acceptance-20261029.json';assert not out.exists()
    report=R/'docs/WRITE-PROFILE-50K-20261029.md';old=report.read_bytes();groups=defaultdict(list)
    for engine in ['fdb','sqlite']:
        for pair in range(2):
            path=L/f'{P}-single-pair{pair}-{engine}-write50-single.jsonl'
            for line in path.read_text(encoding='utf-8').splitlines():
                row=json.loads(line)
                if row['phase']=='write50_commit':groups[engine,row['kind']].append(row)
    results=[]
    for (engine,kind),xs in sorted(groups.items()):
        assert len(xs)==512
        metrics={key:dict(p95_ms=q([x[key] for x in xs],.95),p99_ms=q([x[key] for x in xs],.99),max_ms=max(x[key] for x in xs)) for key in ['commit_ms','visibility_ms']}
        old_pass=metrics['commit_ms']['p95_ms']<=10 and metrics['commit_ms']['max_ms']<=50 and metrics['visibility_ms']['max_ms']<=50
        new_pass=all(v['p95_ms']<=20 and v['p99_ms']<=50 and v['max_ms']<=100 for v in metrics.values())
        results.append(dict(engine=engine,kind=kind,n=512,metrics=metrics,old_pass=old_pass,new_pass=new_pass if engine=='fdb' else None))
    assert all(x['new_pass'] for x in results if x['engine']=='fdb')
    value=dict(authority='Direct user approval: p95<=20ms,p99<=50ms,max<=100ms for commit and visibility; SQLite has no gate',
        percentile='nearest rank ceil(p*n)-1, pooled512/type from two balanced arms',results=results,
        raw_logs_unchanged=True,old_gate_file_unchanged=True,report_before_sha256=hashlib.sha256(old).hexdigest().upper(),
        unknown_74ms_not_resolved=True,mtime_visibility_fdb_max_ms=76.6328,mtime_visibility_sqlite_max_ms=65.7963)
    text=old.decode('utf-8').replace('\r\n','\n');head,body=text.split('\n\n',1)
    labels={'delete':'Sil','insert':'Ekle','update':'Güncelle','mtime_delete_insert':'mtime: sil+ekle'}
    amendment=['## Kullanıcı kapısı güncellemesi — 29 Ekim 2026','',
        '**Güncel karar: tekli yazma GEÇTİ.** Ham örnekler yeniden çalıştırılmadan p99 hesaplandı. '+
        'Commit ve görünürlük için yeni kapı p95≤20ms,p99≤50ms,max≤100ms; bütün dört FDB tipi geçti. '+
        'SQLite max şartı yoktur. mtime sil+ekle görünürlük max FDB76,633ms,SQLite65,796ms: '+
        'FDB bu uçta10,837ms daha yüksek. Commit max karşılığı74,840/57,106ms. '+
        '74,840ms ucun nedeni hâlâ bilinmiyor; çözülmüş veya gürültü diye kapatılmadı.','',
        '| İşlem | Eski kapı | Yeni kapı |','|---|---|---|']
    for x in results:
        if x['engine']=='fdb':amendment.append(f"| {labels[x['kind']]} | {'GEÇTİ' if x['old_pass'] else 'KALDI'} | GEÇTİ |")
    amendment+=['','Eski kapı: commit p95≤10ms/max≤50ms ve görünürlük max≤50ms. '+
        'Yeni kapı: her iki metrikte p95≤20ms,p99≤50ms,max≤100ms. Yüzdelik tanımı nearest-rank; '+
        'n=512/tip, iki dengeli kol pooled. Aşağıdaki ilk durma kararı tarihsel olarak korunur.','']
    start=body.index('| Motor | İşlem | n | Commit p95/max ms |');end=body.index('\n\nBütün kötü FDB örnekleri:',start)
    table=['| Motor | İşlem | n | Commit p95/p99/max ms | Görünürlük p95/p99/max ms | Current private max MiB | Process peak private MiB | Peak WS MiB |','|---|---|---:|---|---|---:|---:|---:|']
    for x in results:
        xs=groups[x['engine'],x['kind']];c=x['metrics']['commit_ms'];v=x['metrics']['visibility_ms']
        fmt=lambda a:'/'.join(f'{a[k]:.3f}' for k in ['p95_ms','p99_ms','max_ms'])
        mem=lambda k:max(r['memory'][k] for r in xs)/2**20
        table.append(f"| {x['engine']} | {labels[x['kind']]} |512| {fmt(c)} | {fmt(v)} | {mem('private_bytes'):.3f} | {mem('os_lifetime_peak_private_commit_bytes'):.3f} | {mem('os_lifetime_peak_working_set_bytes'):.3f} |")
    body=body[:start]+'\n'.join(table)+body[end:]
    report.write_text(head+'\n\n'+'\n'.join(amendment)+'\n'+body,encoding='utf-8')
    value['report_after_sha256']=hashlib.sha256(report.read_bytes()).hexdigest().upper();out.write_text(json.dumps(value,ensure_ascii=False,indent=2),encoding='utf-8')
    print('User revised single gate: PASS, all4 types; p99 recorded; historical failures retained')
if __name__=='__main__':main()
