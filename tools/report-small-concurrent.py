"""Report all tracked4R query/throughput outcomes after the small-WAL change."""
import importlib,json
b=importlib.import_module('measure-churn');s=importlib.import_module('summarize-snapshots')
P='small-commit-snapshot-10k-20261026'
def main():
    result=dict(trials=[],matrices={})
    text=['# Küçük WAL katmanı:4 okuyucu+1 yazıcı','',
          '| Grup/çift | Kontrol/yük ops/s | Oran | Yük/kontrol commit | Geçti mi ≥%80 |',
          '|---|---:|---:|---:|---|']
    for batch in ['one','100']:
        pooled={phase:[] for phase in ['snapshot_load','snapshot_idle_four','snapshot_idle_single']}
        for pair in [0,1]:
            parts={}
            for arm in ['load','control']:
                mode=f'snapshot-{batch}' if arm=='load' else f'snapshot-control-{batch}'
                p=b.LOGS/f'{P}-fdb-{batch}-r{pair}-{arm}-fdb-streamed-{mode}.jsonl'
                assert json.loads(p.with_suffix('.process.json').read_text(encoding='utf-8'))['exit_code']==0
                phases={r['phase']:r for r in map(json.loads,p.read_text(encoding='utf-8').splitlines())}
                parts[arm]=phases['snapshot_load'];assert parts[arm]['oracle'] and parts[arm]['failure'] is None
                if arm=='load':
                    loaded_phases=phases
                    for phase in pooled:pooled[phase]+=s.records(phases[phase])
            load,control=parts['load'],parts['control'];recs=s.records(load);ratio=load['ops_s']/control['ops_s']
            row=dict(batch=batch,pair=pair,ratio=ratio,load_ops_s=load['ops_s'],control_ops_s=control['ops_s'],load_commits=load['commits'],control_commits=control['commits'],memory=load['memory'],maintenance=load['maintenance'],publication=load['publication'],writer_waited=sum(r['writer_waited'] for r in recs),max_writer_wait_ms=max(r['max_writer_wait_ns'] for r in recs)/1e6)
            details=load['maintenance']['details'];assert details['normal_merges_completed']>0 and details['revision_rewrites_completed']>0
            start,end=load['maintenance']['start_ms'],load['maintenance']['end_ms']
            assert all(any(r['started_ms']<end and r['finished_ms']>start for r in recs if r['thread']==thread) for thread in range(4))
            row['read_fails']={}
            for baseline in ['four','single']:
                base=s.records(loaded_phases['snapshot_idle_'+baseline]);failed=[]
                for case in range(38):
                    a=s.percentile([r['engine_ms'] for r in base if r['case_id']==case]);z=s.percentile([r['engine_ms'] for r in recs if r['case_id']==case])
                    if not(z-a<=2 if a<1 else z<=a*2):failed.append(dict(case_id=case,idle_ms=a,loaded_ms=z,delta_ms=z-a))
                row['read_fails'][baseline]=failed
            result['trials'].append(row)
            text.append(f'| {batch}/r{pair} | {control["ops_s"]:.3f} / {load["ops_s"]:.3f} | {ratio*100:.2f}% | {load["commits"]}/{control["commits"]} | {"GEÇTİ" if ratio>=.8 else "KALDI"} |')
        matrix=[]
        for case in range(38):
            sample={phase:[r['engine_ms'] for r in recs if r['case_id']==case] for phase,recs in pooled.items()}
            assert all(len(xs)==40 for xs in sample.values())
            row=dict(case_id=case,p95=s.percentile(sample['snapshot_load']),max=max(sample['snapshot_load']),p50=s.percentile(sample['snapshot_load'],.5))
            for baseline in ['four','single']:
                a=s.percentile(sample['snapshot_idle_'+baseline]);row[baseline+'_idle']=a;row[baseline+'_delta']=row['p95']-a;row[baseline+'_pass']=row['p95']-a<=2 if a<1 else row['p95']<=2*a
            matrix.append(row)
        result['matrices'][batch]=matrix
        text+=['',f'## {batch}:n40/sorgu, ms','', '| Case | Boşta1R /4R p95 | Yük p50 /p95 /max | Fark4R | 4R kapısı |','|---|---:|---:|---:|---|']
        for r in matrix:text.append(f'| {r["case_id"]} | {r["single_idle"]:.4f} / {r["four_idle"]:.4f} | {r["p50"]:.4f} / {r["p95"]:.4f} / {r["max"]:.4f} | {r["four_delta"]:+.4f} | {"GEÇTİ" if r["four_pass"] else "KALDI"} |')
        print(batch,'4R failed',[r['case_id'] for r in matrix if not r['four_pass']])
    text+=['','## Her çiftte ayrı kapılar','', '| Grup/çift | 4R tabanında aşan case | 1R tabanında aşan case |','|---|---|---|']
    for row in result['trials']:
        bad={base:', '.join(str(r['case_id']) for r in row['read_fails'][base]) or 'yok' for base in ['four','single']}
        text.append(f'| {row["batch"]}/r{row["pair"]} | {bad["four"]} | {bad["single"]} |')
    b.write(b.LOGS/f'{P}-summary.json',result);(b.LOGS/f'{P}-tables.md').write_text('\n'.join(text)+'\n',encoding='utf-8')
    print([(r['batch'],r['pair'],r['ratio']) for r in result['trials']])
if __name__=='__main__':main()
