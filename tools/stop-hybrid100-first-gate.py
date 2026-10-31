"""Stop after the first complete balanced100-batch gate fails; never hide extra attempts."""
import json
import math
from collections import defaultdict
from datetime import datetime,timezone
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='hybrid4r50-20261029-v1-b100'
read=lambda p:json.loads(p.read_text(encoding='utf-8-sig'))
pct=lambda a,p=.95:sorted(a)[math.ceil(p*len(a))-1]

def main():
    dest=L/f'{P}-gate.json';assert not dest.exists();d=read(L/f'{P}-design.json')
    idle=defaultdict(list);load=defaultdict(list);through={};tails={};accepted=[]
    for run in d['completed']:
        if not run['label'].startswith('pair0-') or not run['mode'].startswith('snapshot'):continue
        assert run['exit_code']==0;accepted.append(run['log'])
        for x in map(json.loads,(L/run['log']).read_text().splitlines()):
            if x['phase']=='snapshot_idle_four':
                for rr in x['results']:
                    for v in rr['records']:assert v['oracle'];idle[run['engine'],v['case_id']].append(v['engine_ms'])
            if x['phase']=='snapshot_load':
                assert x['oracle'] and x['commits']==1024 and x['failure'] is None
                arm='control' if x['control'] else 'loaded';through[run['engine'],arm]=dict(ops=x['mutations'],elapsed_ms=x['writer_elapsed_ms'],ops_s=x['ops_s']);a=[v['commit_ms'] for v in x['writes']]
                if arm=='loaded':
                    tails[run['engine']]=dict(p95_ms=pct(a),p99_ms=pct(a,.99),max_ms=max(a))
                    for rr in x['results']:
                        for v in rr['records']:
                            assert v['oracle']
                            if v['writer_active']:load[run['engine'],v['case_id']].append(v['engine_ms'])
    assert len(accepted)==4
    g=dict(batch=100,cases=[],throughput=[],writer_tail=tails,failures=[],passed=False,accepted_logs=accepted,balanced_pairs=1,scope='First full balanced pair; stop at its failed gate, any begun extra arm is retained as aborted, excluded from percentiles')
    for e in ['fdb','sqlite']:
        for i in range(41):
            a=idle[e,i];b=load[e,i];assert a and b
            aa=pct(a);bb=pct(b);limit=30 if aa<=30 else 2*aa
            row=dict(engine=e,case_id=i,idle_n=len(a),loaded_n=len(b),idle_p95_ms=aa,loaded_p95_ms=bb,loaded_max_ms=max(b),limit_ms=limit,passed=bb<=limit);g['cases'].append(row)
            if e=='fdb' and not row['passed']:g['failures'].append(dict(kind='query',**row))
        rates={arm:through[e,arm]['ops_s'] for arm in ['control','loaded']};g['throughput'].append(dict(engine=e,rates=rates,loaded_control_ratio=rates['loaded']/rates['control']))
    a,b=g['throughput'];assert a['loaded_control_ratio']>=.75 and a['loaded_control_ratio']>b['loaded_control_ratio']
    assert g['failures'],'Only invoke this stop path when the query gate genuinely failed'
    dest.write_text(json.dumps(g,ensure_ascii=False,indent=2),encoding='utf-8')
    lifecycle=[json.loads(v) for v in (L/f'{P}-lifecycle.jsonl').read_text().splitlines()]
    target=next(x for x in reversed(lifecycle) if x['kind']=='worker_start');assert target['label']=='pair1-loaded' and target['mode']=='snapshot-100'
    stop=dict(utc=datetime.now(timezone.utc).isoformat(),reason='User stop-on-failure rule: first complete balanced pair failed16/41, no further stage authorized',gate=dest.name,target_pid=target['pid'],expected_program=str((R/'output/hybrid4r50-20261029-v2-sqlite.exe').resolve()),expected_label=target['label'],not_durability_test=True,extra_arm_excluded=True)
    (L/f'{P}-intentional-gate-stop.json').write_text(json.dumps(stop,indent=2),encoding='utf-8');print(json.dumps(dict(failed=len(g['failures']),target=target['pid'],stop_intent_recorded=True)))

if __name__=='__main__':main()
