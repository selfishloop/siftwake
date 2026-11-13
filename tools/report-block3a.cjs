// Artifact-only audit/report. Does not open either database or run a query.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
const R=path.resolve(__dirname,'..'), L=path.join(R,'docs/benchmarks'), P='block3a-20261103-';
const p=n=>path.join(L,P+n),read=f=>JSON.parse(fs.readFileSync(f,'utf8').replace(/^\uFEFF/,''));
const write=(f,x)=>fs.writeFileSync(f,JSON.stringify(x,null,2)+'\n');
const hash=f=>new Promise((ok,no)=>{let h=crypto.createHash('sha256');fs.createReadStream(f).on('data',x=>h.update(x)).on('error',no).on('end',()=>ok(h.digest('hex').toUpperCase()));});
function stat(a){a.sort((a,b)=>a-b);return a.length?{n:a.length,min:a[0],p50:a[Math.ceil(.5*a.length)-1],p95:a[Math.ceil(.95*a.length)-1],max:a.at(-1)}:null;}
const num=n=>n==null?'ÖLÇÜLMEDİ':n.toFixed(3);
async function main(){
 const result=read(p('result.json')), summary={stage:result.stage,elapsed_seconds:result.elapsed_seconds,error:result.error,engines:{}};
 for(const engine of ['fdb','sqlite']){
  const rows=fs.readFileSync(p(engine+'.jsonl'),'utf8').trim().split('\n').map(JSON.parse);
  const scans={};for(const text of ['zx','the zx']){const selected=rows.filter(r=>r.phase==='scan'&&r.text===text&&!r.warmup&&!r.profiled);scans[text]={wall:stat(selected.map(r=>r.ms)),cpu_total_ms:selected.reduce((a,r)=>a+r.cpu_ms,0),cpu_mean_ms:selected.reduce((a,r)=>a+r.cpu_ms,0)/selected.length};}
  const gaps=rows.filter(r=>r.phase==='gap'&&!r.profiled);
  const profiles=rows.filter(r=>r.profile);const field=(r,k)=>r.profile?.counters?.[k]?.elapsed_ns/1e6||0, count=(r,k)=>r.profile?.counters?.[k]?.calls||0;
  summary.engines[engine]={scans,gap:stat(gaps.map(r=>r.gap_ms)),commit:stat(gaps.map(r=>r.commit_ms)),acquire:stat(gaps.map(r=>r.acquire_ms)),query:stat(gaps.map(r=>r.query_ms)),between:stat(gaps.map(r=>r.between_ms)),gap_all:rows.filter(r=>r.phase==='gap').length,normal_gap_active_readers:gaps.reduce((o,r)=>(o[r.active_readers]=(o[r.active_readers]||0)+1,o),{}),load_calls:rows.filter(r=>r.phase==='load').reduce((n,r)=>n+r.calls,0),initial:rows.find(r=>r.phase==='ready'),last_command:rows.filter(r=>r.phase==='command_done').at(-1),last_layout:rows.filter(r=>r.phase==='layout_after').at(-1),profiles:profiles.map(r=>({phase:r.phase,text:r.text,ordinal:r.ordinal,sequence:r.sequence,active_readers:r.active_readers,ms:r.ms,gap_ms:r.gap_ms,query_ms:r.query_ms,posting_ms:field(r,'first.posting_setup'),posting_calls:count(r,'first.posting_setup'),cache_wait_ms:field(r,'query.term_cache_mutex_wait'),contended_ms:field(r,'lock.cache_wait'),decode_ms:field(r,'query.term_page_decode'),delta_ms:field(r,'first.delta_filter'),read_ms:field(r,'query.payload_read'),crc_ms:field(r,'query.payload_crc'),schema_ms:field(r,'query.payload_validation'),text_ms:field(r,'query.verify_text'),verify_worker_sum_ms:field(r,'first.candidate_verify'),requests:count(r,'query.payload_read_requests'),bytes:count(r,'query.payload_requested_bytes'),ranges:count(r,'query.payload_ranges'),neighbors:count(r,'query.payload_neighbor_le42'),gap4k:count(r,'query.payload_gap_le4k'),gap64k:count(r,'query.payload_gap_le64k'),far:count(r,'query.payload_gap_far_reverse'),gap_bytes:count(r,'query.payload_gap_bytes'),owners:Object.keys(r.profile.owners||{}).length,profile_thread_cpu_ms:(r.profile.threads||[]).reduce((a,t)=>a+(t.cpu_100ns||0)/10000,0)}))};
  const columns=['phase','text','ordinal','sequence','active_readers','ms','gap_ms','query_ms','posting_ms','posting_calls','cache_wait_ms','contended_ms','decode_ms','delta_ms','read_ms','crc_ms','schema_ms','text_ms','verify_worker_sum_ms','requests','bytes','ranges','neighbors','gap4k','gap64k','far','gap_bytes','owners','profile_thread_cpu_ms'];
  fs.writeFileSync(p(engine+'-profiles.tsv'),[columns.join('\t'),...summary.engines[engine].profiles.map(r=>columns.map(k=>r[k]??'').join('\t'))].join('\n')+'\n');
 }
 const protect={sources_and_fixtures_unchanged:true,files:Object.keys(result.protected_before).length,corpus_sha256:await hash('D:/frunner-corpus/trwiki-100k-cap200k/corpus.payload-bench'),programs:{}};
 for(const [file,expected] of Object.entries(result.protected_before))if(await hash(path.join(R,file))!==expected)protect.sources_and_fixtures_unchanged=false;
 for(const e of ['fdb','sqlite'])protect.programs[e]={sha256:await hash(path.join(R,`output/block3a-20261103-${e}.exe`)),expected:result.programs[e]};
 summary.protected=protect;
 const testLogs=['tests-default.log','tests-diagnostic.log','counter-final.log','examples.log'];summary.tests={};
 for(const log of testLogs){const text=fs.readFileSync(p(log),'utf8');let passed=0,failed=0,ignored=0;for(const m of text.matchAll(/test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;/g)){passed+=+m[1];failed+=+m[2];ignored+=+m[3];}summary.tests[log]={passed,failed,ignored};}
 summary.build_test_seconds=(Date.parse(fs.readFileSync(p('build-end.txt'),'utf8').trim())-Date.parse(fs.readFileSync(p('build-start.txt'),'utf8').trim()))/1000;
 write(p('analysis.json'),summary);
 // Safety metadata only: stale-sequence models must never be advertised ready.
 const ptrPath=path.join(L,'active-benchmark-slots.json'),ptr=read(ptrPath);
 for(const e of ['fdb','sqlite']){ptr.slots[e].status='blocked_partial_diagnostic';ptr.slots[e].pending_diagnostic={report:'docs/BLOCK3A-DIAGNOSIS-20261103.md',collection:'keybench-numeric',ids:Array.from({length:16},(_,n)=>String(n).padStart(10,'0')),last_observed_sequence:summary.engines[e].last_command.sequence,model_sequence_stale:true,main_documents:50000,test_documents_at_last_observation:16,cleanup_performed:false,post_failure_reopen_performed:false};}
 write(ptrPath,ptr);
 console.log(JSON.stringify({elapsed:summary.elapsed_seconds,build_test_seconds:summary.build_test_seconds,tests:summary.tests,protected:protect,profiles:summary.engines.fdb.profiles,scans_sqlite:summary.engines.sqlite.scans,gap:summary.engines.fdb.gap,active:summary.engines.fdb.normal_gap_active_readers},null,2));
}
main().catch(e=>{console.error(e);process.exitCode=1;});
