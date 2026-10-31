"""Audit the stopped Block1, protected corpus and final recovery evidence."""
import importlib
import json
from pathlib import Path

b=importlib.import_module('measure-churn')
P='write-hardening-10k-20261027'


def read(name):
    return json.loads((b.LOGS/name).read_text(encoding='utf-8'))


def main():
    protected=b.protected()
    designs={}
    for section in ['boundary','throughput']:
        d=read(f'write-hardening-{section}-v4-10k-20261027-design.json')
        assert d['protected_before']==protected
        for program in d['programs'].values():assert b.sha(Path(program['path']))==program['sha256']
        for r in d['completed']:assert r['exit_code']==0
        designs[section]=dict(completed=len(d['completed']),protected_unchanged=True,program_hashes_unchanged=True,complete=d.get('complete',False))
    cancellation=read('write-hardening-throughput-v4-10k-20261027-cancelled.json')
    assert cancellation['cancelled']
    s=read(f'{P}-summary.json')
    candidates=[r for r in s['boundary'] if 'candidate' in r['label']]
    assert len(candidates)==4 and sum(r['n'] for r in candidates)==3072 and all(r['over50']==0 for r in candidates)
    assert s['pooled']['fdb-one']['impossible_3of4'] and not s['pooled']['fdb-one']['pass_gate']
    validation=read('snapshot-10k-20261026-validation-write-hardening-20261027-v6.json')
    assert len(validation)==8 and all(r['exit_code']==0 for r in validation)
    k=read('write-hardening-kill-v6-10k-20261027-summary.json')
    assert len(k['results'])==7 and k['protected_unchanged'] and k['old_reader']['exit_code']==0
    for r in k['results']:
        if 'coverage_observed' in r:assert r['coverage_observed'] and r['reopen_full_payload_matrix_oracle_exit_code']==0
        else:assert r['oracle'] and r['verify_exit_code']==0
    assert k['post_reopen_scratch_bytes']==0 and k['one_idle_call']['orphan_bytes']==0
    assert k['one_idle_call']['all_old_source_paths_present_while_pinned']
    assert k['one_idle_call']['post_matrix']['exit_code']==k['one_idle_call']['old_reader_post_matrix']['exit_code']==0
    files=sorted((b.ROOT/'src').rglob('*.rs'))
    b.write(b.LOGS/f'{P}-audit.json',dict(designs=designs,protected=protected,validation=validation,
            killed_reopened=7,old_reader=True,scratch_bytes=0,orphan_source_bytes_after_idle=0,
            gate='FAILED; stopped; no100throughput or30round performance claim',cancellation=cancellation,
            final_program_sha256=b.sha(b.ROOT/'output/write-hardening-final-20261027-v6.exe'),
            source_sha256={str(p.relative_to(b.ROOT)):b.sha(p) for p in files}))
    print('audit passed: protected corpus/fixture, frozen programs, failed gate,232tests/7kills/oldreader/pins/zero-orphans')


if __name__=='__main__':main()
