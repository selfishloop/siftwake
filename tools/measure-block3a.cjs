// One bounded diagnostic. Metadata/log files only; never reset/copy a database.
const fs=require('node:fs'), path=require('node:path'), crypto=require('node:crypto');
const {spawn}=require('node:child_process');
const {isDeepStrictEqual}=require('node:util');
const R=path.resolve(__dirname,'..'), L=path.join(R,'docs/benchmarks'), P='block3a-20261103';
const ptrPath=path.join(L,'active-benchmark-slots.json');
const read=p=>JSON.parse(fs.readFileSync(p,'utf8').replace(/^\uFEFF/,''));
const write=(p,v)=>fs.writeFileSync(p,JSON.stringify(v,null,2)+'\n');
const file=n=>path.join(L,`${P}-${n}`);
const hash=p=>new Promise((ok,no)=>{const h=crypto.createHash('sha256');fs.createReadStream(p).on('data',b=>h.update(b)).on('error',no).on('end',()=>ok(h.digest('hex').toUpperCase()));});
const children=new Set(); let deadline=0, timedOut=false;
async function protectedHashes() {
  const out={};
  async function walk(dir) {
    if(!fs.existsSync(dir))return;
    for(const e of fs.readdirSync(dir,{withFileTypes:true})) {
      const p=path.join(dir,e.name);if(e.isDirectory())await walk(p);else out[path.relative(R,p)]=await hash(p);
    }
  }
  await walk(path.join(R,'src'));await walk(path.join(R,'tests/fixtures'));return out;
}
const events=fs.createWriteStream(file('lifecycle.jsonl'),{flags:'wx'});
function event(kind,data={}) {events.write(JSON.stringify({utc:new Date().toISOString(),kind,...data})+'\n');}
function assert(v,msg) {if(!v) throw new Error(msg);}
function percentile(a,p) {const b=a.slice().sort((x,y)=>x-y);return b[Math.ceil(p*b.length)-1];}
function stats(a) {return a.length?{n:a.length,min:Math.min(...a),p50:percentile(a,.5),p95:percentile(a,.95),max:Math.max(...a)}:null;}
function summarize(rows) {
  const out={};
  for(const text of ['zx','the zx']) {
    const selected=rows.filter(x=>x.phase==='scan'&&x.text===text&&!x.profiled&&!x.warmup);
    out[text]={wall_ms:stats(selected.map(x=>x.ms)),cpu_ms:stats(selected.map(x=>x.cpu_ms)),total_cpu_ms:selected.reduce((a,x)=>a+x.cpu_ms,0)};
  }
  const gap=rows.filter(x=>x.phase==='gap'&&!x.profiled);
  out.gap={wall_ms:stats(gap.map(x=>x.gap_ms)),query_ms:stats(gap.map(x=>x.query_ms)),commit_ms:stats(gap.map(x=>x.commit_ms)),acquire_ms:stats(gap.map(x=>x.acquire_ms))};
  return out;
}
class Worker {
  constructor(engine,state,donors,suffix='') {
    this.engine=engine;this.rows=[];this.pending=[];this.closed=false;
    const program=path.join(R,`output/${P}-${engine}.exe`);
    this.log=fs.createWriteStream(file(`${engine}${suffix}.jsonl`),{flags:'wx'});
    this.err=fs.openSync(file(`${engine}${suffix}.stderr`),'wx');
    const env={...process.env,
      FDB_BENCH_CORPUS_PATH:'D:/frunner-corpus/trwiki-100k-cap200k/corpus.payload-bench',
      FDB_BENCH_CORPUS_INDEX:path.join(L,'trwiki-cap200k-20261029-v3-catalog.json'),
      FDB_BENCH_ORIGINAL_BODIES:'1',FDB_SCALE_DOCUMENTS:'50000',FDB_BENCH_50K_WRITE:'1',
      FDB_TERM_CACHE_ARM:'K',FDB_TIMING_DISABLED:'1'};
    for(const key of ['FDB_HYBRID_WAIT_REFERENCE','FDB_QUERY_PROFILE','FDB_SEQUENCE_PROFILE','POOL_VISIBILITY_PROFILE','FDB_HYBRID_WORKERS','FDB_SNAPSHOT_MASK_REFERENCE'])delete env[key];
    this.child=spawn(program,['block3a',state,donors],{cwd:R,windowsHide:true,stdio:['pipe','pipe',this.err],env});
    children.add(this.child);event('start',{engine,pid:this.child.pid,program});
    this.exit=new Promise((ok)=>this.child.on('exit',(code,signal)=>{children.delete(this.child);this.closed=true;event('exit',{engine,pid:this.child.pid,code,signal});for(const p of this.pending)p.reject(new Error(`${engine} exited ${code}/${signal}`));this.pending=[];ok({code,signal});}));
    let buffer='';this.child.stdout.on('data',chunk=>{
      this.log.write(chunk);buffer+=chunk.toString();let index;
      while((index=buffer.indexOf('\n'))>=0) {const line=buffer.slice(0,index);buffer=buffer.slice(index+1);if(!line)continue;
        let row;try{row=JSON.parse(line);}catch(e){this.fail=e;continue;}
        this.rows.push(row);
        for(let i=0;i<this.pending.length;i++)if(this.pending[i].predicate(row)){const [p]=this.pending.splice(i,1);p.resolve(row);break;}
      }
    });
  }
  wait(predicate) {const found=this.rows.find(predicate);if(found)return Promise.resolve(found);if(this.closed)return Promise.reject(new Error('worker already closed'));return new Promise((resolve,reject)=>this.pending.push({predicate,resolve,reject}));}
  async command(command) {
    assert(!timedOut&&Date.now()<deadline, 'deadline: no more DB work');
    const from=this.rows.length; event('command',{engine:this.engine,command});
    const done=new Promise((resolve,reject)=>this.pending.push({predicate:r=>r.phase==='command_done'&&r.command===command,resolve,reject}));
    this.child.stdin.write(command+'\n');await done;return this.rows.slice(from);
  }
  async quit() {if(this.closed)return;this.child.stdin.end('quit\n');const result=await this.exit;assert(result.code===0,'nonzero worker shutdown');this.log.end();fs.closeSync(this.err);}
}
const sig=layout=>layout.runs?.map(r=>[r.source_id,r.source_bytes,r.index_bytes]);
async function legacy(state) {
  const model=read(state), catalog=read(path.join(L,'trwiki-cap200k-20261029-v3-catalog.json'));
  const fd=fs.openSync('D:/frunner-corpus/trwiki-100k-cap200k/corpus.payload-bench','r');
  const program=path.join(R,'output/maintenance-candidate-cli-20261024.exe');
  const err=fs.openSync(file('legacy.stderr'),'wx');
  const child=spawn(program,[path.join(R,'data/benchmark-work/fdb'),'--stdio'],{cwd:R,windowsHide:true,stdio:['pipe','pipe',err]});children.add(child);event('legacy_start',{pid:child.pid});
  const queue=[];let buffer='';child.stdout.on('data',c=>{buffer+=c.toString();let i;while((i=buffer.indexOf('\n'))>=0){const s=buffer.slice(0,i);buffer=buffer.slice(i+1);queue.shift()?.resolve(JSON.parse(s));}});
  const exited=new Promise(ok=>child.on('exit',(code,signal)=>{for(const pending of queue)pending.reject(new Error(`legacy exited ${code}/${signal}`));ok({code,signal});}));
  const request=v=>new Promise((resolve,reject)=>{queue.push({resolve,reject});child.stdin.write(JSON.stringify(v)+'\n');});
  for(const [id,row] of Object.entries(model.rows).slice(0,50)) {
    const e=catalog.entries[row.body],bytes=Buffer.alloc(e.size);fs.readSync(fd,bytes,0,bytes.length,e.offset);
    const stored=JSON.parse(bytes);stored.document.key={scope:'shared',collection:'texts',id:id.padStart(10,'0')};
    if(row.revision) {const token=`zqv${id.padStart(10,'0')}r${String(row.revision).padStart(10,'0')}zq`;let text=Buffer.from(stored.document.text);text=text.subarray(0,200000-Buffer.byteLength(token)-1);while(text.length&&text.toString('utf8').endsWith('\uFFFD'))text=text.subarray(0,text.length-1);stored.document.text=text.toString()+' '+token;}
    stored.version=row.version;stored.document.title??='';stored.document.fields??={};stored.document.links??=[];
    const result=await request({op:'get',key:stored.document.key});assert(result.ok&&isDeepStrictEqual(result.result,stored),'legacy GET mismatch');
  }
  child.stdin.end();const result=await exited;children.delete(child);assert(result.code===0,'legacy exit');fs.closeSync(fd);fs.closeSync(err);
  return {oracle:true,gets:50,program_sha256:await hash(program),...result};
}
async function main() {
  assert(!fs.existsSync(file('result.json')),'never replay a started experiment');
  const ptr=read(ptrPath),initial=structuredClone(ptr);
  assert(Object.values(ptr.slots).every(s=>s.status==='ready'&&s.documents===50000),'owner slots not ready');
  assert(ptr.sha256==='881756DA400952500BA871F6C7BA8FF8D17E5348E5975420BC5CD90952A7CF12','corpus identity');
  const entries=read(path.join(L,'frunner-key50-20261102-design.json')).donors.filter(e=>e.text_bytes>=2048&&e.text_bytes<=4000).slice(0,16);
  assert(entries.length===16,'16 small original donors required');const donors=file('donors.json');write(donors,entries);
  const states={};for(const engine of ['fdb','sqlite']){states[engine]=file(`${engine}-state.json`);fs.copyFileSync(ptr.slots[engine].state,states[engine]);}
  const d={stage:'starting',initial_slots:initial,plan:'block3a-20261103-plan.md',cells:[],limit_seconds:1200,logical_processors:require('node:os').cpus().length,setup:{},reset:false,ingest:false,programs:{},notes:[]};
  for(const e of ['fdb','sqlite'])d.programs[e]=await hash(path.join(R,`output/${P}-${e}.exe`));
  d.harness_sha256={driver:await hash(__filename),helper:await hash(path.join(R,'tools/bench_support/block3a.rs'))};
  d.corpus_sha256=await hash('D:/frunner-corpus/trwiki-100k-cap200k/corpus.payload-bench');assert(d.corpus_sha256===ptr.sha256,'source hash mismatch');d.protected_before=await protectedHashes();
  write(file('result.json'),d);
  const start=Date.now();deadline=start+1200000;
  const timer=setTimeout(()=>{timedOut=true;event('deadline',{limit_seconds:1200});for(const child of children)child.kill();},1200000);
  const workers={};let failed=null;
  try {
    for(const engine of ['fdb','sqlite']) {
      ptr.slots[engine].status='block3a_diagnostic';ptr.slots[engine].state=states[engine];write(ptrPath,ptr);
      const w=workers[engine]=new Worker(engine,states[engine],donors);await w.wait(r=>r.phase==='ready');
      await w.command('audit');await w.command('setup');d.setup[engine]={ready:w.rows.find(r=>r.phase==='ready'),setup:w.rows.filter(r=>r.phase==='command_done'&&r.command==='setup')};
    }
    let before;
    for(const [index,label] of ['L0-A','L0-Aprime','L1-A','L1-Aprime'].entries()) {
      if(index===2) {
        // Publish A/A before performing the only maintenance call.
        d.aa=d.cells.filter(c=>c.label.startsWith('L0'));write(file('aa.json'),d.aa);write(file('result.json'),d);
        console.log(JSON.stringify({phase:'AA_COMPLETE',rows:d.aa.map(c=>({engine:c.engine,label:c.label,summary:c.summary}))}));
        assert(deadline-Date.now()>45000,'insufficient remaining budget for maintenance; stop');
        const w=workers.fdb;const prior=w.rows.filter(r=>r.phase==='layout_after').at(-1).layout;
        const rows=await w.command('maintain');const post=rows.find(r=>r.phase==='command_done').layout;
        d.maintenance={before:prior,after:post,elapsed_ms:rows.find(r=>r.phase==='command_done').elapsed_ms};
        await w.command('audit');
        if(!(post.stats.segments<prior.stats.segments)) {d.notes.push('run count did not decrease: L1 omitted, no retry');break;}
      }
      const order=index===0||index===3?['fdb','sqlite']:['sqlite','fdb'];
      for(const engine of order) {
        assert(deadline-Date.now()>45000,'insufficient remaining budget for next cell; stop');
        const rows=await workers[engine].command(label);
        const first=rows.find(r=>r.phase==='layout_before').layout,last=rows.find(r=>r.phase==='layout_after').layout;
        const cell={engine,label,layout_before:first,layout_after:last,summary:summarize(rows)};d.cells.push(cell);write(file('result.json'),d);
        if(engine==='fdb') {
          assert(isDeepStrictEqual(sig(first),sig(last)),`${label}: immutable layout changed within cell; stop`);
          if(label==='L0-A')before=sig(first);
          if(label==='L0-Aprime')assert(isDeepStrictEqual(before,sig(first)),'A/A immutable layout changed; stop');
        }
        console.log(JSON.stringify({phase:'cell_complete',engine,label,summary:cell.summary}));
      }
    }
    d.stage='measured';
  } catch(e) {failed=e;d.stage='partial';d.error=String(e);event('failure',{error:String(e)});}
  try {
    const unexpectedExit=Object.values(workers).some(w=>w.closed);
    if(unexpectedExit)d.notes.push('unexpected native exit: stop immediately; no post-failure DB cleanup');
    for(const engine of ['sqlite','fdb'])if(!unexpectedExit&&workers[engine]&&!workers[engine].closed&&!timedOut&&deadline-Date.now()>10000) {
      await workers[engine].command('cleanup');
      ptr.slots[engine].model_sha256=await hash(states[engine]);ptr.slots[engine].status='ready';write(ptrPath,ptr);
    }
    if(!failed&&!timedOut) {
      await workers.sqlite.command('audit');await workers.sqlite.quit();
      const w=workers.fdb;const witness=w.wait(r=>r.phase==='kill_witness');w.child.stdin.write('hold\n');await witness;
      event('intentional_kill',{pid:w.child.pid,witness:await witness});w.child.kill();d.kill=await w.exit;w.log.end();fs.closeSync(w.err);
      const recovered=new Worker('fdb',states.fdb,donors,'-reopen');workers.fdb=recovered;await recovered.wait(r=>r.phase==='ready');await recovered.command('audit');await recovered.quit();
      d.legacy=await legacy(states.fdb);d.stage='completed';
      d.protected_after=await protectedHashes();assert(isDeepStrictEqual(d.protected_before,d.protected_after),'source/fixture changed');
      d.corpus_after=await hash('D:/frunner-corpus/trwiki-100k-cap200k/corpus.payload-bench');assert(d.corpus_after===d.corpus_sha256,'corpus changed');
    }
  } catch(e) {d.stage='partial';d.recovery_error=String(e);event('recovery_failure',{error:String(e)});}
  finally {
    for(const w of Object.values(workers))if(!w.closed){w.child.kill();await w.exit;}
    clearTimeout(timer);d.timed_out=timedOut;d.elapsed_seconds=(Date.now()-start)/1000;d.final_slots=read(ptrPath);d.finished_utc=new Date().toISOString();write(file('result.json'),d);event('driver_exit',{stage:d.stage,elapsed_seconds:d.elapsed_seconds});events.end();
  }
  console.log(JSON.stringify({phase:'FINISHED',stage:d.stage,elapsed_seconds:d.elapsed_seconds,error:d.error,recovery_error:d.recovery_error}));
}
main().catch(e=>{event('driver_failure',{error:String(e)});for(const c of children)c.kill();events.end();console.error(e);process.exitCode=1;});
