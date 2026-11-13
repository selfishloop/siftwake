// Authorized workspace repair, not a benchmark. Never invokes the failed harness.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
const {spawn}=require('node:child_process');const {isDeepStrictEqual}=require('node:util');
const R=path.resolve(__dirname,'..'),L=path.join(R,'docs/benchmarks'),P='block3a-repair-20261103';
const file=n=>path.join(L,`${P}-${n}`),read=p=>JSON.parse(fs.readFileSync(p,'utf8').replace(/^\uFEFF/,''));
const write=(p,x)=>fs.writeFileSync(p,JSON.stringify(x,null,2)+'\n');
const hash=p=>new Promise((ok,no)=>{const h=crypto.createHash('sha256');fs.createReadStream(p).on('data',b=>h.update(b)).on('error',no).on('end',()=>ok(h.digest('hex').toUpperCase()));});
const stable=x=>Array.isArray(x)?x.map(stable):x&&typeof x==='object'?Object.fromEntries(Object.keys(x).sort().map(k=>[k,stable(x[k])])):x;
const digest=x=>crypto.createHash('sha256').update(JSON.stringify(stable(x))).digest('hex').toUpperCase();
function assert(x,message){if(!x)throw new Error(message);}
const children=new Set(),eventLog=fs.createWriteStream(file('lifecycle.jsonl'),{flags:'wx'});
const event=(kind,data={})=>eventLog.write(JSON.stringify({utc:new Date().toISOString(),kind,...data})+'\n');
async function protectedHashes(){
 const out={};async function walk(dir){for(const e of fs.readdirSync(dir,{withFileTypes:true})){const p=path.join(dir,e.name);if(e.isDirectory())await walk(p);else out[path.relative(R,p)]=await hash(p);}}
 await walk(path.join(R,'src'));await walk(path.join(R,'tests/fixtures'));
 for(const p of ['tools/bench_support/block3a.rs','tools/measure-block3a.cjs'])out[p]=await hash(path.join(R,p));return out;
}
function run(engine,mode,config,label){return new Promise((resolve,reject)=>{
 const program=engine==='sqlite-old'?path.join(R,'output/estimator50-20261031-sqlite.exe'):engine==='sqlite'?path.join(R,'tools/native_sqlite/target/release/repair_block3a.exe'):path.join(R,'target/release/examples/repair_block3a.exe');
 const args=engine==='sqlite-old'?['scale-first-oracle','streamed',config]:engine==='sqlite'?[mode,config]:[engine,mode,config];
 const env={...process.env,FDB_BENCH_CORPUS_PATH:'D:/frunner-corpus/trwiki-100k-cap200k/corpus.payload-bench',FDB_BENCH_CORPUS_INDEX:path.join(L,'trwiki-cap200k-20261029-v3-catalog.json'),FDB_BENCH_ORIGINAL_BODIES:'1',FDB_SCALE_DOCUMENTS:'50000',FDB_BENCH_50K_WRITE:'1'};
 for(const k of Object.keys(env))if(k.startsWith('FDB_')&&!['FDB_BENCH_CORPUS_PATH','FDB_BENCH_CORPUS_INDEX','FDB_BENCH_ORIGINAL_BODIES','FDB_SCALE_DOCUMENTS','FDB_BENCH_50K_WRITE'].includes(k))delete env[k];
 const out=fs.createWriteStream(file(label+'.jsonl'),{flags:'wx'}),err=fs.openSync(file(label+'.stderr'),'wx');
 const child=spawn(program,args,{cwd:R,windowsHide:true,env,stdio:['ignore','pipe',err]});children.add(child);event('start',{engine,mode,label,pid:child.pid,program});
 let buffer='',killed=false,failure=null;const rows=[];
 const timer=setTimeout(()=>{failure=new Error(`verification watchdog: ${label}`);child.kill();},600000);
 child.stdout.setEncoding('utf8');child.stdout.on('data',chunk=>{out.write(chunk);buffer+=chunk;let i;while((i=buffer.indexOf('\n'))>=0){const line=buffer.slice(0,i);buffer=buffer.slice(i+1);if(!line)continue;try{
   const row=JSON.parse(line);rows.push(row);
   if(row.phase==='kill_ready') {assert(mode==='hold'&&!killed,'unexpected kill witness');killed=true;event('intentional_process_kill',{engine,pid:child.pid,witness:row});child.kill();}
 }catch(e){failure=e;child.kill();}}});
 child.on('error',e=>{failure=e;});
 child.on('close',(code,signal)=>{clearTimeout(timer);children.delete(child);out.end();fs.closeSync(err);event('exit',{engine,mode,label,pid:child.pid,code,signal,intentional_kill:killed});
   if(failure)return reject(failure);
   if(mode==='hold') {if(!killed||!rows.some(r=>r.phase==='verified'))return reject(new Error('missing verified kill witness'));}
   else if(code!==0)return reject(new Error(`${label} failed: ${code}/${signal}`));
   const cases=rows.filter(r=>r.phase==='case_digest');const caseDigest=cases.length?digest(cases):null;
   console.log(JSON.stringify({phase:'repair_stage',engine,mode,label,case_count:cases.length,case_sha256:caseDigest,sequence:rows.find(r=>r.phase==='verified'||r.phase==='deleted')?.sequence}));
   resolve({engine,mode,label,exit_code:code,signal,intentional_kill:killed,case_sha256:caseDigest,rows});
 });
});}
async function main(){
 assert(!fs.existsSync(file('result.json')),'never replay a started repair blindly');
 const ptrPath=path.join(L,'active-benchmark-slots.json'),ptr=read(ptrPath),initial=structuredClone(ptr);
 assert(Object.values(ptr.slots).every(s=>s.status==='blocked_partial_diagnostic'),'expected blocked owner slots');
 const original=read(path.join(L,'block3a-20261103-result.json'));
 const stages={},configs={},statesBefore={};
 const result={stage:'preparing',initial_slots:initial,programs:{},stages,maintenance_calls:0,new_measurements:0,harness_fix:false,engine_source_changes:false};
 result.protected_before=await protectedHashes();result.corpus_before=await hash(ptr.source);assert(result.corpus_before===ptr.sha256,'corpus changed');
 for(const engine of ['fdb','sqlite']){
   statesBefore[engine]=read(ptr.slots[engine].state);write(file(`${engine}-model-before.json`),statesBefore[engine]);
   const raw=fs.readFileSync(path.join(L,`block3a-20261103-${engine}.jsonl`),'utf8').trim().split('\n').map(JSON.parse);
   const setup=raw.find(r=>r.phase==='command_done'&&r.command==='setup').sequence;
   let versions=null;if(engine==='fdb'){versions=Array(16).fill(null);for(const r of raw.filter(r=>r.phase==='gap'))versions[r.ordinal%16]=r.sequence;assert(versions.every(v=>v!==null),'missing FDB acknowledged versions');}
   configs[engine]=file(`${engine}-config.json`);write(configs[engine],{engine,state:ptr.slots[engine].state,donors:path.join(L,'block3a-20261103-donors.json'),setup_sequence:setup,expected_versions:versions,before_delete:file(`${engine}-before-delete.json`)});
   ptr.slots[engine].status='repair_in_progress';
 }
 for(const program of ['target/release/examples/repair_block3a.exe','tools/native_sqlite/target/release/repair_block3a.exe','output/maintenance-candidate-cli-20261024.exe','output/estimator50-20261031-sqlite.exe'])result.programs[program]=await hash(path.join(R,program));
 write(ptrPath,ptr);write(file('result.json'),result);
 try{
   for(const engine of ['fdb','sqlite']){stages[`${engine}-delete`]=await run(engine,'delete',configs[engine],`${engine}-delete`);write(file('result.json'),result);}
   result.stage='deleted';
   for(const engine of ['fdb','sqlite']){
     stages[`${engine}-reopen-kill`]=await run(engine,'hold',configs[engine],`${engine}-reopen-kill`);write(file('result.json'),result);
     stages[`${engine}-recovered`]=await run(engine,'verify',configs[engine],`${engine}-recovered`);
     assert(stages[`${engine}-recovered`].case_sha256===stages[`${engine}-reopen-kill`].case_sha256,`${engine} digest changed after kill`);write(file('result.json'),result);
   }
   stages.legacy=await run('legacy','verify',configs.fdb,'legacy');assert(stages.legacy.case_sha256===stages['fdb-recovered'].case_sha256,'old reader digest differs');
   stages['sqlite-old']=await run('sqlite-old','verify',ptr.slots.sqlite.state,'sqlite-old');
   const oldCases=stages['sqlite-old'].rows.filter(r=>r.phase==='first_oracle');
   const currentCases=stages['sqlite-recovered'].rows.filter(r=>r.phase==='case_digest');
   assert(oldCases.length===41&&oldCases.every((r,i)=>r.oracle&&r.case_id===i&&r.exact_total===currentCases[i].oracle_df),'old SQLite 41-case oracle differs');
   result.protected_after=await protectedHashes();assert(isDeepStrictEqual(result.protected_before,result.protected_after),'engine/fixture/failed harness changed');
   result.corpus_after=await hash(ptr.source);assert(result.corpus_after===result.corpus_before,'corpus changed');
   for(const engine of ['fdb','sqlite']){
     const state=read(ptr.slots[engine].state),before=statesBefore[engine];
     assert(digest({rows:state.rows,dead:state.dead,next:state.next})===digest({rows:before.rows,dead:before.dead,next:before.next}),'main model mutated');
     const observed=stages[`${engine}-recovered`].rows.find(r=>r.phase==='verified').sequence;assert(state.sequence===observed,'model vs reopened sequence');
     ptr.slots[engine].status='ready';ptr.slots[engine].model_sha256=await hash(ptr.slots[engine].state);delete ptr.slots[engine].pending_diagnostic;
     ptr.slots[engine].last_repair={report:'docs/BLOCK3A-REPAIR-20261103.md',deleted_keys:16,sequence:observed,main_get_oracle:true,cases:41,kill_reopen:true};
   }
   write(ptrPath,ptr);result.stage='completed';result.final_slots=ptr;result.main_models_unchanged_except_sequence=true;
 }catch(e){result.stage='failed';result.error=String(e);for(const c of children)c.kill();for(const s of Object.values(ptr.slots))if(s.status!=='ready')s.status='blocked_repair_failed';write(ptrPath,ptr);throw e;}
 finally{write(file('result.json'),result);event('repair_finish',{stage:result.stage,error:result.error});eventLog.end();}
 console.log(JSON.stringify({phase:'repair_complete',sequences:Object.fromEntries(Object.entries(ptr.slots).map(([e,s])=>[e,s.last_repair.sequence])),new_measurements:0,maintenance_calls:0}));
}
main().catch(e=>{event('repair_error',{error:String(e)});eventLog.end();console.error(e);process.exitCode=1;});
