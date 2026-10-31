"""Stage1 alone: balanced16k controls and same-layout read-only probes."""
import argparse,importlib.util,json,os,subprocess
from pathlib import Path
R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='mask-stage1-20261030'
spec=importlib.util.spec_from_file_location('c',R/'tools/measure-revised50-profile.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);m=c.m;B=c.B

def main():
    global P
    ap=argparse.ArgumentParser();ap.add_argument('--same-binary',action='store_true');ap.add_argument('--read-only',action='store_true');args=ap.parse_args()
    if args.same_binary:P+='-same-read' if args.read_only else '-same'
    dest=L/f'{P}-design.json';assert not dest.exists()
    programs={'control':R/'output/mask-stage1-control-20261029.exe','candidate':R/'output/mask-stage1-candidate-20261030.exe'}
    if args.same_binary:programs={role:R/'output/mask-stage1-same-binary-20261030.exe' for role in programs}
    d=dict(stage='16k',documents=50000,programs={k:dict(path=str(p),sha256=B.sha(p)) for k,p in programs.items()},source_sha256=B.sha(m.CORPUS),protected_before=B.protected(),completed=[],change='replacement mask only; delta and WAL handles unchanged',cache='warm OS; same-layout read-only probes; no eviction',sequence=['control','candidate','candidate','control'])
    B.write(dest,d);life=L/f'{P}-lifecycle.jsonl';stream=life.open('x',encoding='utf-8')
    def event(kind,**v):stream.write(json.dumps(dict(utc=m.utc(),kind=kind,**v))+'\n');stream.flush();os.fsync(stream.fileno())
    stop=L/f'{P}-watch.stop';watcher=subprocess.Popen(['powershell.exe','-NoProfile','-ExecutionPolicy','Bypass','-File',str(R/'tools/watch-write50.ps1'),'-DriverId',str(os.getpid()),'-LogPath',str(L/f'{P}-events.jsonl'),'-StopPath',str(stop),'-LifecyclePath',str(life)],stdout=(L/f'{P}-watch.stdout').open('x'),stderr=(L/f'{P}-watch.stderr').open('x'),creationflags=subprocess.CREATE_NO_WINDOW)
    def run(program,mode,state,e,label,role):
        x=c.run_child(program,mode,state,e,P+'-'+label,event);x.update(role=role,label=label,state=str(state));d['completed'].append(x);B.write(dest,d);return x
    try:
        event('driver_start',pid=os.getpid());options=c.env();states=[]
        for i,role in enumerate([] if args.read_only else d['sequence']):
            B.reset('fdb');state=L/f'{P}-arm{i}-{role}-state.json';states.append(state)
            e=options.copy();e['FDB_SEED_IDLE_CLEANUP']='1';run(programs[role],'scale-seed',state,e,f'arm{i}-{role}-seed',role)
            e=options.copy();e.update(FDB_SEQUENCE_PROFILE='1',FDB_WRITE_PROFILE_CAPACITY='32768',FDB_THREAD_PROFILE='1')
            if args.same_binary and role=='control':e['FDB_SNAPSHOT_MASK_REFERENCE']='1'
            r=run(programs[role],'snapshot-control-1',state,e,f'arm{i}-{role}',role)
            x=next(x for x in map(json.loads,(L/r['log']).read_text().splitlines()) if x['phase']=='snapshot_load')
            assert x['oracle'] and x['commits']==16384 and x['write_profile']['dropped_frames']==0
            if i:assert B.sha(state)==B.sha(states[0])
            ptr=m.read(m.s.CURRENT);ptr['slots']['fdb'].update(documents=50000,status='ready',state=str(state),model_sha256=B.sha(state));B.write(m.s.CURRENT,ptr)
        d['stage']='same_layout_reads';B.write(dest,d)
        state=Path(m.read(m.s.CURRENT)['slots']['fdb']['state']) if args.read_only else states[-1]
        # No writes; all four probes see the identical final control layout/model.
        for i,role in enumerate(d['sequence']):
            e=options.copy();e.update(FDB_SNAPSHOT_IDLE_ONLY='1',FDB_SNAPSHOT_IDLE_READERS='4',FDB_SNAPSHOT_ROUNDS='5')
            if args.same_binary and role=='control':e['FDB_SNAPSHOT_MASK_REFERENCE']='1'
            r=run(programs[role],'snapshot-one',state,e,f'read{i}-{role}',role)
            x=next(x for x in map(json.loads,(L/r['log']).read_text().splitlines()) if x['phase']=='snapshot_idle_probe');assert x['oracle']
        d.update(stage='completed50k',state=str(state),protected_after=B.protected(),source_after=B.sha(m.CORPUS));assert d['protected_after']==d['protected_before'] and d['source_after']==d['source_sha256'];B.write(dest,d)
    finally:stop.write_text('normal stop');watcher.wait(timeout=20);event('driver_finish',stage=d['stage']);stream.close()

if __name__=='__main__':main()
