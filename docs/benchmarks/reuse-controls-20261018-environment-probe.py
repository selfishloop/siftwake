"""Read-only scheduling calibration and same-executable reuse replay.
Temporary process AboveNormal priority, verified for both conditions; no global
power/affinity/app changes. Ordinary and AboveNormal baseline blocks balanced
within each process. Thread CPU/cycle observations outside query wall clocks.
"""
import ctypes, importlib.util, json, statistics, sys, time
from pathlib import Path
ROOT=Path.cwd()
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,ROOT/path)
 value=importlib.util.module_from_spec(spec);spec.loader.exec_module(value);return value
old=module('old','tools/measure-interleaved-reads.py')
api=ctypes.WinDLL('kernel32',use_last_error=True)
api.SetPriorityClass.argtypes=[ctypes.c_void_p,ctypes.c_uint32];api.SetPriorityClass.restype=ctypes.c_int
api.GetPriorityClass.argtypes=[ctypes.c_void_p];api.GetPriorityClass.restype=ctypes.c_uint32
def priority(worker,value):
 handle=int(worker.process._handle)
 if not api.SetPriorityClass(handle,value):raise ctypes.WinError(ctypes.get_last_error())
 assert api.GetPriorityClass(handle)==value
 worker.metadata['priority_class_verified']=value
prefix=ROOT/'docs/benchmarks/reuse-controls-scheduling-10k-20261018'
report=lambda suffix:Path(f'{prefix}-{suffix}')
assert not report('design.json').exists()
exe=ROOT/'output/payload-isolated-20261017-control-measure_interleaved_reads-normal.exe'
classes=[0x20,0x8000]; records={name:[] for name in ['control','fdb']}
design=dict(executable=str(exe.relative_to(ROOT)),sha256=old.sha(exe),cases=[0,11,20,31],
 labels={'control':'Normal priority 0x20','fdb':'AboveNormal priority 0x8000'},processes=2,
 blocks_per_condition_per_process=2,repeats=10,affinity_mask=4,
 cache='warm OS cache; strict open + all oracle checks/warmup outside clocks; no eviction',
 observation='QueryThreadCycleTime/GetThreadTimes before/after query clock. CPU tick quantization makes micro-query CPU ratios unsuitable.',
 source='same existing first10000, no database reset/copy/corpus creation',orders=[],workers=[])
old.write(report('design.json'),design);before=old.snapshot();old.write(report('before.json'),before)
for process in range(2):
 worker=old.Worker(exe,report(f'worker-{process}.jsonl'),4,False,True,[0,11,20,31])
 for value in records.values():value.append([])
 try:
  labels=[0,1,1,0] if process==0 else [1,0,0,1]
  for block,label in enumerate(labels):
   priority(worker,classes[label]); command=dict(id=process*10+block,seed=process*100+block//2,repeats=10)
   began=time.monotonic();row=worker.block(command)
   records[['control','fdb'][label]][process].append(row)
   design['orders'].append(dict(process=process,block=block,priority_class=classes[label],command=command))
   print('scheduling',process,block,label,round(time.monotonic()-began,3),flush=True)
  design['workers'].append(worker.metadata)
 finally:worker.close()
after=old.snapshot();assert before==after;old.write(report('after.json'),after);old.write(report('design.json'),design)
summary=old.summarize(records,design)
summary['interpretation']='Same executable/process; only child priority changes. ABBA/BAAB, API CPU/cycle observations outside wall clock. Not a code speedup, cold-cache or concurrency result; cumulative thread CPU has OS tick quantization.'
summary['wall_cpu_samples']=[]
for n,case in enumerate(records['control'][0][0]['queries']):
 row={k:case[k] for k in ['text','offset','limit']}
 for name,processes in records.items():
  triples=[dict(wall_ms=wall,cpu_ms=cpu,cycles=cycles) for blocks in processes for block in blocks for wall,cpu,cycles in zip(block['queries'][n]['timing']['samples'],block['queries'][n]['thread_observation']['cpu_ms'],block['queries'][n]['thread_observation']['cycles'],strict=True)]
  row[name]=dict(samples=triples,wall_median_ms=statistics.median(t['wall_ms'] for t in triples),cpu_median_ms=statistics.median(t['cpu_ms'] for t in triples),max_wall_minus_cpu_ms=max(t['wall_ms']-t['cpu_ms'] for t in triples),median_wall_minus_cpu_ms=statistics.median(t['wall_ms']-t['cpu_ms'] for t in triples))
 summary['wall_cpu_samples'].append(row)
old.write(report('summary.json'),summary)
# Same archived normal probe, all three labels at equal verified child priority.
reuse=module('reuse','tools/measure-reuse-controls.py');base=reuse.Worker
class PriorityWorker(base):
 def __init__(self,*args,**kwargs):
  super().__init__(*args,**kwargs)
  try:priority(self,0x8000)
  except BaseException:
   self.close();raise
reuse.Worker=PriorityWorker
sys.argv=['measure-reuse-controls.py','--prefix','docs/benchmarks/reuse-controls-priority-10k-20261018',
 '--executable','output/reuse-controls-20261018-normal.exe','--case-indices','0,11,18,20,28,31,34,37',
 '--pattern','mixed','--processes','4','--repeats','5']
reuse.main()
