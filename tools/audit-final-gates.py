"""Final user-revised gates; verify immutable raw programs/protected inputs."""
import importlib
import json
import re
from pathlib import Path

b=importlib.import_module('measure-churn')


def read(name):return json.loads((b.LOGS/name).read_text(encoding='utf-8'))


def main():
    protected=b.protected()
    churn=read('reclaim-idle30-10k-20261027-design.json')
    assert churn['measurement_complete'] and churn['protected_before']==churn['protected_after']==protected
    assert read('reclaim-idle30-10k-20261027-audit.json')['gate_pass']
    assert all(not r['failed_rounds'] for r in churn['disk_results'].values())
    for r in churn['programs'].values():assert b.sha(Path(r['path']))==r['sha256']
    profiles=[]
    for n in [1,2,3]:
        d=read(f'wal-sequence-profile-10k-20261027-a{n}-design.json')
        assert d['complete'] and d['protected_before']==d['protected_after']==protected and not d['slow_commits']
        assert b.sha(Path(d['program']))==d['sha256']
        profiles.append(dict(max_ms=d['worst']['commit_ms'],wal_open_count=len(d['wal_opens'])))
    qualified=read('wal-qualified-10k-20261027-design.json')
    assert qualified['complete'] and qualified['gate_pass'] and qualified['protected_after']==protected
    hundred=read('hundred-pair-10k-20261027-design.json')
    assert hundred['complete'] and hundred['gate_pass'] and hundred['protected_before']==hundred['protected_after']==protected
    for r in hundred['programs'].values():assert b.sha(Path(r['path']))==r['sha256']
    recovery=[]
    for prefix in ['reclaim-idle30-kill','wal-sequence-kill','hundred-pair-kill']:
        d=read(f'{prefix}-10k-20261027-summary.json')
        assert len(d['results'])==7 and d['protected_unchanged'] and d['old_reader']['exit_code']==0
        for result in d['results']:
            if 'coverage_observed' in result:assert result['coverage_observed'] and result['reopen_full_payload_matrix_oracle_exit_code']==0
            else:assert result['oracle'] and result['verify_exit_code']==0
        assert d['post_reopen_scratch_bytes']==0 and d['one_idle_call']['orphan_bytes']==0
        assert d['one_idle_call']['all_old_source_paths_present_while_pinned'] and d['one_idle_call']['old_reader_post_matrix']['exit_code']==0
        recovery.append(dict(step=prefix,kills=7,old_reader=True,scratch_bytes=0,orphan_bytes=0))
    validation=read('snapshot-10k-20261026-validation-sequence-profile-20261027-v1.json')
    assert len(validation)==8 and all(r['exit_code']==0 for r in validation)
    log=(b.LOGS/'snapshot-10k-20261026-validation-sequence-profile-20261027-v1-test.log').read_text(encoding='utf-8')
    rows=re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',log)
    counts=[sum(int(r[i]) for r in rows) for i in range(3)];assert counts==[233,0,14]
    observations=read('hundred-pair-10k-20261027-summary.json')
    assert len(observations['waits']['fdb']['failed_cases'])==20
    b.write(b.LOGS/'final-gates-10k-20261027-audit.json',dict(protected=protected,churn=True,
            profiles=profiles,qualified=qualified['trials'],historic419_cause='NOT_REPRODUCED_UNPROVEN;no handle optimization applied',
            hundred=hundred['results'],new_hundred_gate_pass=True,
            broader_read_ram_gate='OPEN;20of38read bounds exceeded;processpeak77.94MiB;100commitp95~1004ms',
            recoveries=recovery,tests=counts,source_sha256={str(p.relative_to(b.ROOT)):b.sha(p) for p in sorted((b.ROOT/'src').rglob('*.rs'))},
            no_later_blocks_or_c_abi=True))
    print('Final audit passed: revised gates,233tests,21kills,old-reader/pins;broader read/RAM and419cause remain explicitly open')


if __name__=='__main__':main()
