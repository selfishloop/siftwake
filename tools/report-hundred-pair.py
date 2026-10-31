"""All100-pair latency/memory/query outcomes, including failed old read bounds."""
import importlib
import json
import math

b=importlib.import_module('measure-churn');P='hundred-pair-10k-20261027'


def pct(xs,p=.95):
    return sorted(xs)[math.ceil(len(xs)*p)-1]


def records(p):return [r for t in p['results'] for r in t['records']]


def main():
    result=dict(arms=[],matrices={})
    cases=json.loads((b.LOGS/'churn-10k-20261024-corpus-index.json').read_text(encoding='utf-8'))['cases']
    lines=['# Tek100-işlem çifti: tüm sonuçlar','',
           '| Motor/kol | İşlem/sn |100-işlem commit p50/p95/max ms | OS peak private/WS MiB |','|---|---:|---:|---:|']
    for engine in ['fdb','sqlite']:
        for arm in ['control','load']:
            mode='snapshot-100' if arm=='load' else 'snapshot-control-100'
            path=b.LOGS/f'{P}-{engine}-{arm}-{engine}-streamed-{mode}.jsonl'
            assert json.loads(path.with_suffix('.process.json').read_text(encoding='utf-8'))['exit_code']==0
            phases={r['phase']:r for r in map(json.loads,path.read_text(encoding='utf-8').splitlines())}
            load=phases['snapshot_load'];assert load['oracle'] and load['commits']==128
            xs=[w['commit_ms'] for w in load['writes']]
            row=dict(engine=engine,arm=arm,ops_s=load['ops_s'],commit_p50_ms=pct(xs,.5),commit_p95_ms=pct(xs),commit_max_ms=max(xs),memory=load['memory'],maintenance=load['maintenance'],publication=load['publication'])
            result['arms'].append(row)
            m=row['memory'];lines.append(f'| {engine}/{arm} | {row["ops_s"]:.3f} | {row["commit_p50_ms"]:.3f}/{row["commit_p95_ms"]:.3f}/{row["commit_max_ms"]:.3f} | {m["os_lifetime_peak_private_commit_bytes"]/2**20:.2f}/{m["os_lifetime_peak_working_set_bytes"]/2**20:.2f} |')
            if arm=='load':
                a=records(phases['snapshot_idle_four']);z=[r for r in records(load) if r['writer_active']]
                matrix=[]
                for cid in range(38):
                    idle=[r['engine_ms'] for r in a if r['case_id']==cid];active=[r['engine_ms'] for r in z if r['case_id']==cid]
                    r=dict(case_id=cid,case=cases[cid],idle_p95_ms=pct(idle),active_n=len(active),loaded_p95_ms=pct(active) if active else None,
                           loaded_max_ms=max(active) if active else None)
                    r['pass_old_bound']=(r['loaded_p95_ms']-r['idle_p95_ms']<=2 if r['idle_p95_ms']<1 else r['loaded_p95_ms']<=2*r['idle_p95_ms']) if active else None
                    matrix.append(r)
                result['matrices'][engine]=matrix
                result.setdefault('waits',{})[engine]=dict(writer_waiting_reads=sum(r['writer_waited'] for r in z),longest_single_writer_wait_ms=max(r['max_writer_wait_ns'] for r in z)/1e6,
                       max_query_sum_writer_wait_ms=max(r['writer_wait_ns'] for r in z)/1e6,active_reads=len(z),
                       failed_cases=[r['case_id'] for r in matrix if r['pass_old_bound'] is False])
    for engine,matrix in result['matrices'].items():
        lines+=['',f'## {engine}: idle4R vs writer-active queries','',
                '| Case /offset/limit | Yük n | Boşta p95 /yük p95 /max ms | Fark ms | Eski2×/kısa+2ms |',
                '|---|---:|---:|---:|---|']
        for r in matrix:
            c=r['case'];name=c['text'] if len(c['text'])<40 else c['text'][:32]+'…'
            if r['active_n']==0:lines.append(f'| {r["case_id"]}: {name} /{c["offset"]}/{c["limit"]} |0|{r["idle_p95_ms"]:.4f}/ölçülmedi||ÖLÇÜLMEDİ|');continue
            lines.append(f'| {r["case_id"]}: {name} /{c["offset"]}/{c["limit"]} |{r["active_n"]}|{r["idle_p95_ms"]:.4f}/{r["loaded_p95_ms"]:.4f}/{r["loaded_max_ms"]:.4f}|{r["loaded_p95_ms"]-r["idle_p95_ms"]:+.4f}|{"GEÇTİ" if r["pass_old_bound"] else "KALDI"}|')
    b.write(b.LOGS/f'{P}-summary.json',result)
    (b.LOGS/f'{P}-tables.md').write_text('\n'.join(lines)+'\n',encoding='utf-8')
    print(json.dumps(result['waits']),flush=True)


if __name__=='__main__':main()
