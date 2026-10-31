"""Generate gate tables exclusively from immutable captured JSONL."""
import importlib,json,math
b=importlib.import_module('measure-churn')
P='small-commit-v2-10k-20261026'
SQLITE_PREFIX=None
def quantile(xs,q):
    xs=sorted(xs);return xs[max(0,math.ceil(q*len(xs))-1)]
def phases(label,engine,mode):
    prefix=SQLITE_PREFIX if engine=='sqlite' and SQLITE_PREFIX else P
    path=b.LOGS/f'{prefix}-{label}-{engine}-streamed-{mode}.jsonl'
    assert json.loads(path.with_suffix('.process.json').read_text(encoding='utf-8'))['exit_code']==0
    return {r['phase']:r for r in map(json.loads,path.read_text(encoding='utf-8').splitlines())}
def main():
    design=json.loads((b.LOGS/f'{P}-design.json').read_text(encoding='utf-8'));assert design['complete']
    result=dict(trials=[],pooled={},corpus=design['protected_before'],cache=design['cache'])
    text=['# Küçük streamed commit ve dirty500: ham sonuç tabloları','',
          '| Motor/çift/tür | n | Commit p50/p95/max ms | Yazı→doğrulanmış görünürlük p95/max ms |',
          '|---|---:|---:|---:|']
    for engine in ['fdb','sqlite']:
        pooled=[]
        for pair in [0,1]:
            label=f'r{pair}'
            r=phases(label,engine,'small-latency')['small-latency'];assert r['oracle'];pooled+=r['records']
            row=dict(engine=engine,pair=pair,kinds={},publication=r['publication'],memory=r['memory'])
            for kind in ['update','delete','insert']:
                xs=[v for v in r['records'] if v['kind']==kind];assert len(xs)==(48 if kind=='update' else 24)
                c=[v['commit_ms'] for v in xs];v=[v['write_to_verified_visibility_ms'] for v in xs]
                row['kinds'][kind]=dict(n=len(xs),p50=quantile(c,.5),p95=quantile(c,.95),max=max(c),visibility_p95=quantile(v,.95),visibility_max=max(v))
                z=row['kinds'][kind]
                text.append(f'| {engine}/r{pair}/{kind} | {len(xs)} | {z["p50"]:.4f} / {z["p95"]:.4f} / {z["max"]:.4f} | {z["visibility_p95"]:.4f} / {z["visibility_max"]:.4f} |')
            clean=phases(label,engine,'small-open');dirty=phases(label+'-dirty',engine,'small-open');seed=phases(label,engine,'seed');write=phases(label,engine,'small-dirty')['small-dirty']
            if engine=='fdb':assert write['stats']['delta_keys']==500 and dirty['small_idle']['stats']['delta_keys']==500
            row.update(clean_open_ms=clean['opened']['open_ms'],dirty_open_ms=dirty['opened']['open_ms'],
                       clean_private_mib=clean['small_idle']['memory']['private_bytes']/2**20,
                       dirty_private_mib=dirty['small_idle']['memory']['private_bytes']/2**20,
                       clean_ws_mib=clean['small_idle']['memory']['working_set_bytes']/2**20,
                       dirty_ws_mib=dirty['small_idle']['memory']['working_set_bytes']/2**20,
                       seed_peak_private_mib=seed['seeded']['memory']['os_lifetime_peak_private_commit_bytes']/2**20,
                       seed_peak_ws_mib=seed['seeded']['memory']['os_lifetime_peak_working_set_bytes']/2**20,
                       dirty_write_peak_private_mib=write['memory']['os_lifetime_peak_private_commit_bytes']/2**20,
                       dirty_open_peak_private_mib=dirty['small_idle']['memory']['os_lifetime_peak_private_commit_bytes']/2**20)
            result['trials'].append(row)
        result['pooled'][engine]={}
        for kind in ['update','delete','insert']:
            xs=[v for v in pooled if v['kind']==kind];c=[v['commit_ms'] for v in xs];v=[v['write_to_verified_visibility_ms'] for v in xs]
            result['pooled'][engine][kind]=dict(n=len(xs),p50=quantile(c,.5),p95=quantile(c,.95),max=max(c),visibility_p95=quantile(v,.95),visibility_max=max(v))
    text+=['','| Motor/çift | Temiz open ms | Dirty500 open ms | Temiz private/WS MiB | Dirty idle private/WS MiB | Ingest peak private/WS MiB | Dirty write peak private MiB |', '|---|---:|---:|---:|---:|---:|---:|']
    for z in result['trials']:
        text.append(f'| {z["engine"]}/r{z["pair"]} | {z["clean_open_ms"]:.3f} | {z["dirty_open_ms"]:.3f} | {z["clean_private_mib"]:.3f} / {z["clean_ws_mib"]:.3f} | {z["dirty_private_mib"]:.3f} / {z["dirty_ws_mib"]:.3f} | {z["seed_peak_private_mib"]:.3f} / {z["seed_peak_ws_mib"]:.3f} | {z["dirty_write_peak_private_mib"]:.3f} |')
    b.write(b.LOGS/f'{P}-summary.json',result);(b.LOGS/f'{P}-tables.md').write_text('\n'.join(text)+'\n',encoding='utf-8')
    for engine,kinds in result['pooled'].items():print(engine,kinds)
    for row in result['trials']:print(row['engine'],row['pair'],'dirty',row['dirty_open_ms'],row['dirty_private_mib'],'clean',row['clean_private_mib'],'peak',row['seed_peak_private_mib'])
if __name__=='__main__':main()
