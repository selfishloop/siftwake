"""Summarize saved churn evidence only. Never touches a database or corpus."""
import json
import math
import statistics
import re
from pathlib import Path

ROOT=Path(__file__).resolve().parent.parent
LOGS=ROOT/'docs/benchmarks'
P='churn-concurrent-10k-20261024'


def rows(path):
    return [json.loads(x) for x in path.read_text(encoding='utf-8-sig').splitlines()]


def quantile(values):
    v=sorted(values)
    if not v:
        return {'n':0}
    return dict(n=len(v),p50=statistics.median(v),p95=v[math.ceil(len(v)*.95)-1],maximum=v[-1])


def main():
    files=sorted(LOGS.glob(f'{P}-*.jsonl'))
    parsed={p.name:rows(p) for p in files if p.with_suffix('.process.json').exists()
            and json.loads(p.with_suffix('.process.json').read_text(encoding='utf-8-sig'))['exit_code']==0}
    latency={}
    matrices=[]
    churn=[]
    churn_baselines=[]
    burst=[]
    concurrent=[]
    idle=[]
    opening=[]
    failures=[]
    for name,records in parsed.items():
        engine='sqlite' if '-sqlite-' in name else 'fdb'
        api='owned' if '-owned-' in name else 'streamed'
        match=re.search(r'-r(\d+)-',name)
        run=int(match.group(1)) if match else -1
        context=dict(engine=engine,api=api,run=run,file=name)
        for row in records:
            phase=row.get('phase')
            if phase=='opened':
                opening.append(dict(context,**row))
            if phase=='idle':
                idle.append(dict(context,**row))
            if phase=='latency':
                key=f'{engine}/{api}/{row["kind"]}/{row["batch"]}'
                group=latency.setdefault(key,dict(engine=engine,api=api,kind=row['kind'],batch=row['batch'],commit_samples_ms=[],visible_upper_bound_samples_ms=[],errors=[],runs=[]))
                group['runs'].append(dict(run=run,file=name,
                    commit=quantile([s['commit_ms'] for s in row['samples'] if s['ok']]),
                    visibility_upper_bound=quantile([s['visible_end_to_end_ms'] for s in row['samples'] if s['ok']]),
                    errors=[s['error'] for s in row['samples'] if not s['ok']] ))
                for sample in row['samples']:
                    if sample['ok']:
                        group['commit_samples_ms'].append(sample['commit_ms'])
                        group['visible_upper_bound_samples_ms'].append(sample['visible_end_to_end_ms'])
                    else:
                        group['errors'].append(sample['error'])
            if phase=='matrix':
                matrices.append(dict(context,**row))
            if phase=='churn_round':
                commit_ms=sum(v['commit_ms'] for v in row['attempts'] if v['ok'])
                errors=[v.get('error') for v in row['attempts'] if not v['ok']]
                churn.append(dict(context,round=row['round'],commit_sum_ms=commit_ms,
                                  write_including_oracle_ms=row['write_including_oracle_ms'],
                                  maintenance_ms=row['maintenance_ms'],before=row['before'],after=row['after'],
                                  stats=row['stats'],memory=row['memory'],errors=errors))
            if phase=='churn_baseline':
                churn_baselines.append(dict(context,**row))
            if phase.startswith('burst'):
                burst.append(dict(context,**row))
            if phase=='concurrent':
                baseline=next(r for r in records if r['phase']=='matrix' and r['label']=='concurrent_idle')
                writer_baseline=next(r for r in records if r['phase']=='writer_baseline')
                by_case=[]
                for n,case in enumerate(baseline['records']):
                    active=[r for r in row['read_records'] if r['case_id']==n and not r['writer_finished']]
                    all_read=[r for r in row['read_records'] if r['case_id']==n]
                    q=quantile([r['total_ms'] for r in active])
                    idle_q=quantile(case['samples_ms'])
                    all_q=quantile([r['total_ms'] for r in all_read])
                    ratio=all_q.get('p95',0)/idle_q['p95'] if all_read else None
                    by_case.append(dict(case=case['case'],active=q,all_reads=quantile([r['total_ms'] for r in all_read]),
                                        idle=idle_q,p95_ratio=ratio,
                                        active_finished_flag_p95_ratio=q.get('p95',0)/idle_q['p95'] if active else None,
                                        gate_uses_all_reads_conservatively=True,
                                        read_wait=quantile([r['wait_ms'] for r in all_read]),
                                        engine_only=quantile([r['engine_ms'] for r in active])))
                writes=row['write']
                concurrent.append(dict(context,batch=row['batch'],cases=by_case,
                    writer= dict(commits=writes['commits'],mutations=writes['mutations'],
                    throughput_ops_s=writes['mutations']*1000/writes['commits_elapsed_ms'],
                    solo_throughput_ops_s=writer_baseline['throughput_ops_s'],
                    commit=quantile([r['commit_ms'] for r in writes['rows']]),
                    wait=quantile([r['wait_ms'] for r in writes['rows']]),
                    previous_jobs_drained=writes['previous_jobs_drained_by_streamed_commits'],
                    running_observations=writes['running_job_observations_after_commits'],
                    final_maintenance_ms=writes['final_maintenance_ms']),
                    maximum_read_ms=max(r['total_ms'] for r in row['read_records']),
                    oracle_admission=quantile([r['oracle_admission_ms'] for r in row['read_records']]),
                    active_flag_limit='writer_finished sampled after query; cannot classify a request queued while writer stopped, so gate includes all reads',
                    all_cases_active=all(c['active']['n']>0 for c in by_case),memory=row['memory']))
    for group in latency.values():
        group['commit']=quantile(group['commit_samples_ms'])
        group['visibility_upper_bound']=quantile(group['visible_upper_bound_samples_ms'])
    delta=[]
    for active in [r for r in matrices if r['label']=='active']:
        before=next(r for r in matrices if r['file']==active['file'] and r['label']=='idle')
        delta.append(dict(engine=active['engine'],api=active['api'],run=active['run'],stats=active['stats'],
            cases=[dict(case=a['case'],idle=quantile(b['samples_ms']),active=quantile(a['samples_ms']),
                        p95_ratio=quantile(a['samples_ms'])['p95']/quantile(b['samples_ms'])['p95'])
                   for a,b in zip(active['records'],before['records'])]))
    for p in LOGS.glob(f'{P}-*.process.json'):
        result=json.loads(p.read_text(encoding='utf-8-sig'))
        if result['exit_code']:
            failures.append(result)
    summary=dict(latency=list(latency.values()),delta=delta,churn=churn,churn_baselines=churn_baselines,burst=burst,
                 concurrent=concurrent,idle=idle,opening=opening,failures=failures,
                 input_files=list(parsed),scope='saved JSONL arithmetic only; p95 low sample sizes retained')
    (LOGS/f'{P}-summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps(dict(files=len(files),latency_groups=len(latency),churn_rounds=len(churn),concurrent_runs=len(concurrent),failed_processes=len(failures))))


if __name__=='__main__':
    main()
