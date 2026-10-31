"""Read-only final evidence audit, including rejected intermediate candidates."""
import importlib,json,re
b=importlib.import_module('measure-churn')
def read(path):return json.loads(path.read_text(encoding='utf-8-sig'))
def main():
    protected=b.protected();evidence=[];designs=[]
    for path in b.LOGS.glob('small-commit*-20261026*design.json'):
        d=read(path)
        if not d.get('complete'):continue
        assert d.get('protected_before',d.get('protected'))==protected,path
        if 'protected_after' in d:assert d['protected_after']==protected,path
        if 'program' in d and 'sha256' in d:assert b.sha(b.ROOT/d['program'])==d['sha256']
        if 'programs' in d:
            for program in d['programs'].values():assert b.sha(b.ROOT/program['path'])==program['sha256']
        for process in d.get('completed',d.get('runs',[])):
            assert process['exit_code']==0
            log=b.LOGS/process['log'];assert read(log.with_suffix('.process.json'))==process
            evidence.append(log)
        designs.append(path.name);evidence.append(path)
    required=['small-commit-v6-10k-20261026-design.json','small-commit-final-v6-10k-20261026-design.json','small-commit-v7-10k-20261026-design.json']
    required += [f'small-commit-snapshot-v6-10k-20261026-fdb-{batch}-r{pair}-design.json' for batch in ['one','100'] for pair in [0,1]]
    assert all(name in designs for name in required)
    for prefix in ['small-commit-kill-10k-20261026','small-commit-kill-v4-10k-20261026','small-commit-kill-v6-10k-20261026','small-commit-kill-v7-10k-20261026']:
        path=b.LOGS/f'{prefix}-summary.json';d=read(path);assert d['protected_unchanged'] and not d['power_loss_claim'];assert d['old_reader']['exit_code']==0;assert len(d['results'])==7
        for result in d['results']:
            if 'mode' in result:assert result['verify_exit_code']==0 and result['oracle'] and result['marker_oracle']['all_passed']
            else:assert result['coverage_observed'] and result['reopen_full_payload_matrix_oracle_exit_code']==0 and result['pending_old_new_marker_oracle']['all_passed']
        evidence+=list(b.LOGS.glob(f'{prefix}-*.jsonl'));evidence.append(path)
    validation=read(b.LOGS/'snapshot-10k-20261026-validation-small-v7.json');assert len(validation)==8 and all(r['exit_code']==0 for r in validation)
    for row in validation:evidence.append(b.ROOT/row['log'])
    tests=(b.ROOT/next(r['log'] for r in validation if r['name']=='test')).read_text(encoding='utf-8-sig')
    rows=re.findall(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored',tests)
    counts=[sum(int(r[i]) for r in rows) for i in [1,2,3]];assert counts[1]==0
    slots=[p.name for p in (b.ROOT/'data/benchmark-work').iterdir() if p.is_dir()];assert sorted(slots)==['fdb','sqlite']
    result=dict(protected=protected,designs=sorted(designs),fixed_slots=slots,test_counts=dict(passed=counts[0],failed=counts[1],ignored=counts[2]),
                working_source_sha256={name:b.sha(b.ROOT/name) for name in ['Cargo.toml','src/engine.rs','src/engine/light.rs','src/engine/streamed.rs','src/engine/snapshot.rs','src/query.rs','src/format.rs']},
                balanced_performance_program=dict(path='output/small-commit-fdb-20261026-v6.exe',sha256=b.sha(b.ROOT/'output/small-commit-fdb-20261026-v6.exe')),
                final_correctness_guard='v7 per-entry dense/sparse shadow coverage, tested after cache miss',
                source_format_changed=False,power_loss_claim=False,final_program=dict(path='output/small-commit-fdb-20261026-v7.exe',sha256=b.sha(b.ROOT/'output/small-commit-fdb-20261026-v7.exe')),
                final_normal_kill_windows=7,final_small_wal_kill_boundaries=3,old_reader=True,
                evidence=[dict(path=str(p.relative_to(b.LOGS)),bytes=p.stat().st_size,sha256=b.sha(p)) for p in sorted(set(evidence))])
    b.write(b.LOGS/'small-commit-v6-10k-20261026-audit.json',result);print('Audit passed',counts,len(evidence),'files, protected bytes unchanged')
if __name__=='__main__':main()
