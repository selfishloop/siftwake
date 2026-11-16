"""Conservative FileObject/PID and DiskIO join for one bounded cold preflight."""
import collections
import csv
import ctypes
import datetime as dt
import hashlib
import json
import pathlib
import re
import sys

evidence = pathlib.Path(sys.argv[1])
baseline = pathlib.Path(sys.argv[2])
output = pathlib.Path(sys.argv[3])
epoch = 116444736000000000


def load(p):
    return json.loads(p.read_text(encoding="utf-8-sig"))


def iso_tick(text):
    delta = dt.datetime.fromisoformat(text) - dt.datetime(1970,1,1,tzinfo=dt.timezone.utc)
    extra = re.search(r"\.\d{6}(\d)", text)
    return epoch+(delta.days*86400+delta.seconds)*10**7+delta.microseconds*10+(int(extra[1]) if extra else 0)


def norm(path):
    return re.sub(r"\\+",r"\\",path).lower()


class Stream:
    size = 0
    def __init__(self):
        self.sha = hashlib.sha256()
    def __iter__(self):
        for line in sys.stdin.buffer:
            self.size += len(line)
            if self.size > 1024*1024**2:
                raise RuntimeError("CSV stream cap exceeded")
            self.sha.update(line)
            yield line.decode("utf-8",errors="replace").removeprefix("\ufeff")


arm = load(evidence/"ready.json")
engine = arm["engine"]
files = load(baseline)["files"][engine]
admin = load(evidence/"admin-result.json")
reader = load(evidence/"reader-result.json")
cold = load(evidence/"cold.jsonl")
pid = cold["pid"]
first, last = (epoch + int(cold[k])//100 for k in ("open_start_unix_ns","first_result_unix_ns"))
purge_end = iso_tick(admin["et_utc"])
required = set(files["required"])
assert len(required) == arm["expected_files"]
assert len(files["entries"]) == arm["files"]
query_device = ctypes.WinDLL("kernel32",use_last_error=True).QueryDosDeviceW
query_device.argtypes = (ctypes.c_wchar_p,ctypes.c_wchar_p,ctypes.c_uint32)
query_device.restype = ctypes.c_uint32
volumes = {}
paths = {}
for entry in files["entries"]:
    path = entry["path"]
    drive = path[:2]
    if drive not in volumes:
        buffer = ctypes.create_unicode_buffer(4096)
        if not query_device(drive,buffer,len(buffer)):
            raise ctypes.WinError(ctypes.get_last_error())
        volumes[drive] = buffer.value
    paths[norm(path)] = entry["name"]
    paths[norm(volumes[drive]+path[2:])] = entry["name"]

history = {}
seen = {}
last_reads = {}
read_counts = collections.Counter()
unknown_reads = 0
seen_creates = {}
key_names = collections.defaultdict(set)
init = collections.defaultdict(list)
disk = []
foreign = []
headers = []
row_counts = collections.Counter()
protected_keys = []
stream = Stream()
events = 0

for number, row in enumerate(csv.reader(stream,skipinitialspace=True),1):
    if len(row)<20 or not row[16].strip().isdigit():
        continue
    events += 1
    kind, op, tick = row[0].strip(), row[1].strip(), int(row[16])
    row_pid = int(row[9],16)
    data = [x.strip() for x in row[19:]]
    row_counts[(kind,op,row[2].strip(),row[3].strip())] += 1
    if kind=="FileIo" and op in ("Create","Read","OperationEnd") and purge_end <= tick <= last:
        protected_keys.append(f"{int(row[6]):03d}\t{tick:019d}\t{row_pid:010d}\t{int(row[10],16):010d}\n")
    meta = dict(row=number,filetime=tick,pid=row_pid,tid=row[10].strip())
    within = first <= tick <= last
    pre = purge_end <= tick < first
    if kind=="EventTrace" and op=="Header":
        headers.append([tick,int(data[4])])
    elif kind=="FileIo" and op=="Create" and len(data)>=7:
        obj=data[1]
        history.pop(obj,None)
        name=paths.get(norm(data[6]))
        if name is not None:
            create={**meta,"path":data[6],"file_object":obj}
            history[obj]=(name,create)
            if within and row_pid==pid:
                seen_creates.setdefault(name,create)
            if pre and row_pid!=pid:
                foreign.append({**meta,"op":op,"name":name})
    elif kind=="FileIo" and op=="Close" and len(data)>=2:
        history.pop(data[1],None)
    elif kind=="FileIo" and op=="Read" and len(data)>=6:
        obj,key=data[2:4]
        mapping=history.get(obj)
        if not mapping:
            if within and row_pid==pid:
                unknown_reads+=1
            continue
        name,create=mapping
        if pre and row_pid!=pid:
            foreign.append({**meta,"op":op,"name":name})
        if within and row_pid==pid and first<=create["filetime"]<=tick and create["pid"]==pid:
            read={**meta,"file_object":obj,"file_key":key,"requested_bytes":int(data[5]),"create":create}
            seen.setdefault(name,read)
            last_reads[name]=read
            read_counts[name]+=1
            key_names[key].add(name)
    elif kind=="DiskIo" and op=="ReadInit" and within and len(data)>=2:
        init[data[0]].append((tick,row_pid,data[1],number))
    elif kind=="DiskIo" and op=="Read" and within and len(data)>=9:
        disk.append((tick,data,number))

storage=collections.Counter()
witnesses={}
ambiguous=[]
for tick, data, number in disk:
    names=key_names.get(data[5])
    if not names:
        continue
    candidates=[x for x in init[data[6]] if x[0]<=tick]
    if len(names)!=1 or not candidates:
        ambiguous.append(number)
        continue
    at, issuer, tid, init_number=max(candidates)
    if tid!=data[8]:
        ambiguous.append(number)
    elif issuer==pid:
        name=next(iter(names))
        size=int(data[2])
        storage[name]+=size
        witnesses.setdefault(name,dict(row=number,init_row=init_number,filetime=tick,
                                       init_filetime=at,file_key=data[5],bytes=size))

# tracerpt writes its summary before closing the dump on this host. The
# parent verifies decoder and consumer exit separately.
summary_path=output/"tracerpt-summary.txt"
if not summary_path.is_file():
    # Do not promote a trace when the decoder did not produce its summary.
    summary=""
else:
    summary=summary_path.read_text(encoding="utf-8-sig",errors="replace")
lost=re.search(r"Total Events\s+Lost\s+(\d+)",summary)
processed=re.search(r"Total Events\s+Processed\s+(\d+)",summary)
lost=int(lost[1]) if lost else None
processed=int(processed[1]) if processed else None

# CSV section accounting with exact protected-window identities.
summary_counts=collections.Counter()
csv_counts=collections.Counter()
aliases={"{90cbdc39-4a3e-11d1-84f4-0000f80464e3}":"FileIo",
         "{01853a65-418f-4f36-aefc-dc0f1d2fd235}":"SystemConfig"}
for line in summary.splitlines():
    if not re.match(r"^\|\s+\d+\s+",line): continue
    m=re.fullmatch(r"\|\s*(\d+)\s+(.*?)\s+(\d+)\s+(\{[^}]+\})\|\s*",line)
    if not m: raise RuntimeError("unparsed summary row")
    n,middle,ver,guid=m.groups()
    fields=re.fullmatch(r"(.*?)\s*\b(0)\s+(.+?)",middle)
    if not fields: raise RuntimeError("unparsed summary task")
    provider,task,op=fields.groups()
    provider=provider.strip() or guid.lower()
    if provider=="EventTrace" and op=="PartitionInfoExtension" and ver=="2":
        op="PartitionInfoExtensionV2"
    summary_counts[(provider,op,task,ver)]+=int(n)
for (provider,op,task,ver),n in row_counts.items():
    csv_counts[(aliases.get(provider,provider),op,task,ver)]+=n
deltas=sorted([(*k,summary_counts[k]-csv_counts[k]) for k in summary_counts.keys()|csv_counts.keys()
               if summary_counts[k]!=csv_counts[k]])
protected_sha=hashlib.sha256("".join(sorted(protected_keys)).encode("ascii")).hexdigest()

errors=[]
if any(admin.get(k)!=0 for k in ("start_exit","es_exit","et_exit","stop_exit")) or admin["failure"] or not admin["stop_status_idle"]:
    errors.append("administrator lifecycle / collector")
if not reader["oracle"] or not reader["source_and_model_unchanged"] or not cold["oracle"]:
    errors.append("oracle or final inventory")
if lost!=0 or processed!=events:
    errors.append("merged loss or full event count")
if not any(t0<=purge_end<=first<=last<=t1 for t0,t1 in headers):
    errors.append("trace timestamps")
if required-seen.keys():
    errors.append("expected same-window path / PID reads incomplete")
extensions=(".fdb", ".idx") if engine == "fdb" else (".db",)
if not sum(size for name,size in storage.items() if name.endswith(extensions))>0:
    errors.append("no attributed target data DiskIO")
if ambiguous or foreign:
    errors.append("ambiguous DiskIO or foreign pre-open access")
result=dict(technical_pass=not errors,errors=errors,merged_lost=lost,decoded_events=events, engine=engine,
            count_differences=deltas,protected_fileio_sha=protected_sha,protected_fileio_rows=len(protected_keys),
            summary_events=processed,loss_scope="collector bazı ölçülemedi",
            stream_bytes=stream.size,stream_sha256=stream.sha.hexdigest(),target_pid=pid,
            trace_headers=headers,purge_end_filetime=purge_end,cold_window_filetime=[first,last],
            expected_files=len(required),matched_files=len(required&seen.keys()),
            missing=sorted(required-seen.keys()),unknown_cold_read_rows=unknown_reads,
            total_attributed_disk_bytes=sum(storage.values()),disk_bytes_by_name=dict(storage),
            foreign_count=len(foreign),foreign_first=foreign[:32],ambiguous_count=len(ambiguous),
            ambiguous_rows=ambiguous[:32],cold_open_ms=cold["open_ms"],
            cold_first_result_ms=cold["open_start_to_first_ms"],
            per_required_file={n:dict(create=seen_creates.get(n),first_read=seen.get(n),
                                  last_read=last_reads.get(n),count=read_counts[n],
                                  storage_bytes=storage[n],disk_witness=witnesses.get(n)) for n in sorted(required)})
(output/"identity.json").write_text(json.dumps(result,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
print(json.dumps({k:result[k] for k in ("technical_pass","errors","matched_files","missing","merged_lost",
    "decoded_events","stream_bytes","total_attributed_disk_bytes","foreign_count","ambiguous_count","cold_first_result_ms")}))

