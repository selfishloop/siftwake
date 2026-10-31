"""User-revised1c/1d first: existing10k, every-round idle, no engine changes."""
import argparse
import importlib
import json
import os
import threading
import time

b=importlib.import_module('measure-churn')
P=b.PREFIX='reclaim-idle30-10k-20261027'
b.PROGRAMS={'fdb':b.ROOT/'output/reclaim-idle30-fdb-20261027-v1.exe',
            'sqlite':b.ROOT/'output/reclaim-idle30-sqlite-20261027-v1.exe'}


def observed_run(engine,state,label):
    trace=b.LOGS/f'{P}-{label}-disk-samples.jsonl'
    assert not trace.exists()
    done=threading.Event()
    def monitor():
        start=time.monotonic()
        with trace.open('w',encoding='utf-8') as stream:
            while not done.is_set():
                files=[]
                for path in (b.ROOT/f'data/benchmark-work/{engine}').iterdir():
                    try:
                        if path.is_file():files.append((path.name,path.stat().st_size))
                    except FileNotFoundError:pass
                stream.write(json.dumps(dict(ms=(time.monotonic()-start)*1000,
                    bytes=sum(n for _,n in files),files=len(files),
                    scratch_bytes=sum(n for name,n in files if name.startswith('input-') or '.build-' in name)))+'\n')
                done.wait(.05)
    worker=threading.Thread(target=monitor)
    worker.start()
    try:return b.run(engine,'write-churn-idle-30','streamed',state,label)
    finally:done.set();worker.join()


def main():
    parser=argparse.ArgumentParser();parser.add_argument('--resume',action='store_true');args=parser.parse_args()
    target=b.LOGS/f'{P}-design.json'
    before=b.protected()
    prior=json.loads((b.LOGS/'write-hardening-10k-20261027-audit.json').read_text(encoding='utf-8'))
    current={str(p.relative_to(b.ROOT)):b.sha(p) for p in sorted((b.ROOT/'src').rglob('*.rs'))}
    assert current==prior['source_sha256'],'This first step measures the unchanged engine'
    programs={k:dict(path=str(p),sha256=b.sha(p)) for k,p in b.PROGRAMS.items()}
    if target.exists():
        assert args.resume,'Never overwrite evidence'
        d=json.loads(target.read_text(encoding='utf-8'))
        assert d['programs']==programs and d['protected_before']==before
    else:
        d=dict(programs=programs,protected_before=before,engine_source_sha256=current,completed=[],orders=[],
            policy='30rounds;1000update500delete500insert each;20x100streamed uninterrupted inside each round;one maintain_idle then2s;finalall45000retired and10000current markers plus38cases',
            order='SQLite,FDB,FDB,SQLite balanced if gates pass; stop on failed FDB disk gate',
            settings='FDB small-WAL+reclaim explicit opt-in; product defaults unchangedOFF; SQLiteFTS5trigram detailnone FULLWAL cache2MiB mmap0 tempFILE',
            cache='warm OS; no eviction/backgroundCPU/power isolation',
            disk_gate='after every maintenance <=1.3x initial; before/50ms sampled transient peaks separate; tombstone/dead-payload trends reviewed, not extrapolated to infinite time',
            memory='OSprivate/working-set lifetime and sampled counters include harness; finalmarker oracle is separate',
            samples='50ms directory sampling may miss brief peaks; source CRC/validation not disabled')
        b.write(target,d)
    for flag in ['FDB_RECLAMATION_CONTROL','FDB_BENCH_SNAPSHOTS','FDB_COST_LEGACY','FDB_QUERY_VISIBILITY_PRUNING','FDB_SNAPSHOT_FIXED_COMMITS','FDB_SMALL_OPERATIONS']:os.environ.pop(flag,None)
    os.environ['FDB_STREAMED_WAL_LAYER']='1'
    for ordinal,engine in enumerate(['sqlite','fdb','fdb','sqlite']):
        label=f'r{ordinal//2}-{engine}'
        state=b.LOGS/f'{P}-{label}-state.json'
        final=b.LOGS/f'{P}-{label}-{engine}-streamed-write-churn-idle-30.process.json'
        if final.exists() and json.loads(final.read_text(encoding='utf-8'))['exit_code']==0:continue
        seed=b.LOGS/f'{P}-{label}-{engine}-streamed-seed.process.json'
        pointer=b.LOGS/f'{P}-slot-{engine}.json'
        if seed.exists():
            assert args.resume and json.loads(pointer.read_text(encoding='utf-8'))['state']==str(state)
        else:
            b.reset(engine);b.write(pointer,dict(state=str(state),label=label))
        d['orders'].append(label)
        d['completed'].append(b.run(engine,'seed','streamed',state,label));b.write(target,d)
        d['completed'].append(observed_run(engine,state,label));b.write(target,d)
        rows=[json.loads(s) for s in (b.LOGS/f'{P}-{label}-{engine}-streamed-write-churn-idle-30.jsonl').read_text(encoding='utf-8').splitlines()]
        base=next(r['disk']['retained_bytes'] for r in rows if r['phase']=='write_churn_baseline')
        rounds=[r for r in rows if r['phase']=='write_churn_round']
        assert len(rounds)==30 and any(r['phase']=='write_churn_verified' and r['oracle'] for r in rows)
        bad=[dict(round=r['round'],ratio=r['after']['retained_bytes']/base) for r in rounds if r['after']['retained_bytes']>1.3*base]
        d.setdefault('disk_results',{})[label]=dict(initial_bytes=base,final_ratio=rounds[-1]['after']['retained_bytes']/base,failed_rounds=bad)
        b.write(target,d)
        if engine=='fdb' and bad:
            d['stopped']='FAILED1c disk gate; no repair/throughput/profile or later blocks';break
    d['protected_after']=b.protected();assert d['protected_before']==d['protected_after']
    d['measurement_complete']=True;b.write(target,d)
    print(json.dumps(d.get('disk_results',{})),flush=True)


if __name__=='__main__':main()
