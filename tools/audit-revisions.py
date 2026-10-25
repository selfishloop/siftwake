"""Independent final checks against saved observations, with no DB writes."""
import csv
import importlib
import json
import hashlib
import subprocess

bench=importlib.import_module('measure-churn')
ROOT,LOGS=bench.ROOT,bench.LOGS
P='revisions-10k-20261025-v2'


def load(path):return json.loads(path.read_text(encoding='utf-8-sig'))


def main():
    design=load(LOGS/f'{P}-design.json');assert design['complete']
    assert bench.protected()==design['protected_before']==design['protected_after']
    for path,sha in design['programs'].items():assert bench.sha(ROOT/path)==sha
    assert all(x['exit_code']==0 for x in design['completed'])
    expected={'rounds':90,'queries':304,'queries-pooled':114,'single':1536}
    for suffix,count in expected.items():
        with (LOGS/f'{P}-{suffix}.csv').open(encoding='utf-8',newline='') as source:
            actual=list(csv.DictReader(source));assert len(actual)==count,(suffix,len(actual))
    summary=load(LOGS/f'{P}-summary.json')
    assert summary['gates']['disk_10'] and summary['gates']['oracle'] and summary['gates']['kill']
    assert len(summary['background_read']['samples'])==38*30
    assert summary['background_read']['oracle'] and summary['background_write']['oracle']
    kills=load(LOGS/'revisions-kill-10k-20261025-summary.json')
    assert len(kills['results'])==5
    for result in kills['results']:
        assert result['coverage_observed'] and result['reopen_full_payload_matrix_oracle_exit_code']==0
        assert result['pending_old_new_marker_oracle']['all_passed']
        if result['stage']=='cache':assert result['authoritative_sources_unchanged']
    checks=load(LOGS/'revisions-verification-resume-20261025.json')
    assert checks['status']=='complete' and all(x['exit_code']==0 for x in checks['steps'])
    changed=subprocess.check_output(['git','diff','3e4559d','--name-only','--','src','Cargo.toml','Cargo.lock'],cwd=ROOT,text=True).splitlines()
    allowed={'src/engine.rs','src/lib.rs','src/engine/revisions.rs','src/engine/revisions/tests.rs'}
    assert set(changed)<=allowed,changed
    policy=(ROOT/'src/engine.rs').read_text(encoding='utf-8')
    assert 'revision_reclamation: false,' in policy
    protected_formats=subprocess.check_output(['git','diff','3e4559d','--','src/format.rs','src/manifest.rs','src/payload.rs','src/segment.rs','Cargo.toml','Cargo.lock'],cwd=ROOT)
    assert not protected_formats
    evidence=list(LOGS.glob('revisions-*'))
    proof=dict(status='complete',baseline_commit='3e4559d',corpus_fixture_unchanged=True,
               frozen_programs_verified=True,default_reclamation_enabled=False,
               current_persistent_format_dependency_diff_bytes=len(protected_formats),
               csv_counts=expected,kills=5,background_queries=1140,gates=summary['gates'],
               retained_failed_observer_log='revisions-verification-20261025-kills.log',
               raw_evidence_files=len(evidence),raw_evidence_bytes=sum(p.stat().st_size for p in evidence if p.is_file()))
    bench.write(LOGS/'revisions-proof-20261025.json',proof)
    if '--staged' in __import__('sys').argv:
        blobs=subprocess.check_output(['git','ls-files','--stage','-z','--','docs/benchmarks/revisions-*'],cwd=ROOT)
        verified=[]
        for row in blobs.split(b'\0'):
            if not row:continue
            header,path=row.split(b'\t',1)
            content=(ROOT/path.decode('utf-8')).read_bytes()
            actual=hashlib.sha1(b'blob '+str(len(content)).encode()+b'\0'+content).hexdigest()
            assert actual==header.split()[1].decode(),path
            verified.append(dict(path=path.decode(),git_blob=actual,sha256=hashlib.sha256(content).hexdigest()))
        bench.write(LOGS/'revisions-staged-byte-proof-20261025.json',dict(verified=verified,count=len(verified),raw_bytes_preserved=True))
    print(json.dumps(proof,indent=2))


if __name__=='__main__':main()
