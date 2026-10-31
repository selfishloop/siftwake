import json,math,csv,statistics
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='hybrid-serial-extra50-20261030'
def pct(v):return sorted(v)[math.ceil(len(v)*.95)-1]
def main():
    d=json.loads((L/f'{P}-design.json').read_text(encoding='utf-8'));assert d['stage']=='completed';cpu={};aux={};requests=[]
    for run in d['completed']:
        rows=list(map(json.loads,(L/run['log']).read_text(encoding='utf-8').splitlines()))
        for x in rows:
            if x['phase']=='cpu_block':cpu.setdefault((run['role'],x['case_id']),[]).append(x)
            if x['phase'] in ['snapshot_idle_probe','snapshot_idle_single','snapshot_idle_four','snapshot_load']:
                for t in x['results']:
                    for q in t['records']:
                        if x['phase']=='snapshot_load' and not q['writer_active']:continue
                        if run['label'].startswith('aux-'):aux.setdefault((run['role'],run['label'].split('-')[1],q['case_id']),[]).append(q['engine_ms'])
                        elif run['label'].startswith('loaded-profile'):
                            assert q['oracle'] and q['internal_wait_measured'];p=q['query_profile'];c=p['counters'];fallback=c.get('first.hybrid_serial_fallbacks',{}).get('calls',0);tries=c.get('first.permit_try',{}).get('calls',0)
                            assert tries<=1 and fallback<=1
                            if fallback:assert len(p['threads'])==1 and c.get('first.permits_granted',{}).get('calls',0)==0
                            requests.append(dict(role=run['role'],label=run['label'],phase=x['phase'],**q,cpu_ms=sum(t['cpu_100ns'] or 0 for t in p['threads'])/10000,cycles=sum(t['cycles'] or 0 for t in p['threads']),fallback=fallback,tries=tries))
    names=[x['text'] for x in json.loads((L/'trwiki-cap200k-20261029-v3-catalog.json').read_text())['cases']]+['the zx'];cpus=[]
    for i in range(42):
        vals={role:sum(x['cpu_ms'] for x in cpu[role,i])/sum(x['queries'] for x in cpu[role,i]) for role in ['before','after']};cpus.append(dict(case_id=i,text=names[i],before=vals['before'],after=vals['after'],difference=vals['after']-vals['before'],gate_member=i!=41,blocks={role:cpu[role,i] for role in ['before','after']}))
    auxrows=[dict(role=role,readers=readers,case_id=i,text=names[i],n=len(v),p95=pct(v),samples=v,gate_member=i!=41) for (role,readers,i),v in sorted(aux.items())]
    loaded=[]
    for phase in ['snapshot_idle_single','snapshot_idle_four','snapshot_load']:
        for i in range(41):
            for role in ['before','after']:
                v=[x for x in requests if x['phase']==phase and x['case_id']==i and x['role']==role];assert v
                loaded.append(dict(phase=phase,role=role,case_id=i,n=len(v),wall_p95=pct([x['search_ms'] for x in v]),cpu_mean=statistics.mean(x['cpu_ms'] for x in v),cpu_p95=pct([x['cpu_ms'] for x in v]),fallbacks=sum(x['fallback'] for x in v),threads_max=max(len(x['query_profile']['threads']) for x in v),condvar_p95=pct([x['query_profile']['counters'].get('first.permit_condvar_wait',{}).get('elapsed_ns',0)/1e6 for x in v])))
    assert all(x['fallbacks']==0 for x in loaded if x['phase']=='snapshot_idle_single')
    result=dict(cpu=cpus,aux=auxrows,loaded=loaded,one_reader_no_fallback=True,all_try_at_most_once=True,fallback_no_worker_or_lease=True,aux_the_zx_not_in_gate=True)
    (L/f'{P}-summary.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf-8');(L/f'{P}-requests.json').write_text(json.dumps(requests,ensure_ascii=False),encoding='utf-8')
    lines=['# Seri fallback CPU ve ek tarama —30 Ekim2026','','Ana kabul41-case düzeni değiştirilmedi;bu ölçümler ayrıca. the zx yalnız metadata oracle ile42.ekcase,kapı dışında;eski ishmael dışarıda. Auxsource corpus CRC ile ilk50k okunup sadece query bit41 üretildi;corpus dosyası değiştirilmedi. Same live source/WAL/manifest SHA read-onlyaux+CPU boyunca eşit. 1R/4R aux ABBA,CPU ABBA. Program SHA/loglar tasarımda.','','## Parent+child process CPU blokları','','Her query için5blok;fast200/slow5 query/blok. GetProcessTimes parent+temporaryqueryworkers toplamı,oracle veGET doğrulaması blok dışında;helper sampler CPU dahil. Sonuç/droplama/loop overhead dahil. CPU ms/query=ΣblokCPU/Σquery;tekqueryCPU yüzdeliği değildir. Yeni eşik/tolerans eklenmedi;bütün sayısal artışlar da görünür.','','|Case/sorgu|Before CPU ms/query|After|Fark ms|','|---|---:|---:|---:|']
    for x in cpus:lines.append(f"|{x['case_id']}:{x['text']}|{x['before']:.6f}|{x['after']:.6f}|{x['difference']:+.6f}|")
    lines+=['','## Negatif/kısa ek taramalar:1R ve4R p95','','|Okuyucu|Case|Before p95/N|After p95/N|','|---|---|---|---|']
    for readers in ['1R','4R']:
        for i in [34,35,36,37,38,41]:
            a=next(x for x in auxrows if x['role']=='before' and x['readers']==readers and x['case_id']==i);b=next(x for x in auxrows if x['role']=='after' and x['readers']==readers and x['case_id']==i);lines.append(f"|{readers}|{names[i]}|{a['p95']:.3f}/{a['n']}|{b['p95']:.3f}/{b['n']}|")
    lines+=['','## Loaded request CPU,1R davranış ve tektry kanıtı','','Diagnostic ikili ayrı;profil overheadi nedeniyle kabul p95 yerine kullanılamaz. GetThreadTimes parent+child nicemlenebilir;processCPUblokları bunu tamamlar. Her fallback requestte try≤1,backgroundworker0,grant0:doğrulandı.1Rbefore/after bütün41 case fallback0:doğrulandı.','','|Faz/case|Role|N|Wall p95|Mean CPU|CPU p95|Fallback adedi|Max request thread|Condvar p95|','|---|---|---:|---:|---:|---:|---:|---:|---:|']
    for x in loaded:
        lines.append(f"|{x['phase']}/{x['case_id']}|{x['role']}|{x['n']}|{x['wall_p95']:.3f}|{x['cpu_mean']:.3f}|{x['cpu_p95']:.3f}|{x['fallbacks']}|{x['threads_max']}|{x['condvar_p95']:.3f}|")
    lines+=['','Kapanış: KALDI. Ek 4R zx p95 894,895→994,314 ms; profilli loaded zx parent+child ortalama CPU 472,458→522,559 ms arttı. İzin beklemesinin kalkması tek başına kabul değildir. 1R kod yolu/fallback sıfır kanıtı, bütün süre ve CPU değerlerinin aynı olacağı anlamına gelmez; sayısal farklar yukarıda korunur. [Bütün kapılar ve duruş](HYBRID-SERIAL-50K-20261030.md).','','[Bütün CPU blokları/auxhamörnekleri](benchmarks/'+P+'-summary.json) · [Her sequence/thread ve kaynak sayaçları](benchmarks/'+P+'-requests.json) · [SHA/exit/readonlysource ve bütün loglar](benchmarks/'+P+'-design.json). Profil4Rroller DBdeilerleyen yazıları kullanır;fiziksel yerleşim aynı diye sunulmaz. Kötü sonuçlar çıkarılmadı;the zx gate dışında bilgi olarak kalır.']
    (R/'docs/HYBRID-SERIAL-CPU-50K-20261030.md').write_text('\n'.join(lines)+'\n',encoding='utf-8');print(json.dumps({k:v for k,v in result.items() if k not in ['cpu','aux','loaded']}))
if __name__=='__main__':main()
