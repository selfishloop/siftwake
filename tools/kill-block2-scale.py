"""50k FDB witnessed process kills and frozen old-reader oracle; no new DB slot."""
import hashlib
import importlib
import json
import os
import subprocess
import zlib
from pathlib import Path

k = importlib.import_module('measure-churn-kill')
b = k.bench
P = 'block2-kill-50k-20261028-v1'


def old_reader(state):
    model = json.loads(state.read_text(encoding='utf-8'))
    index = json.loads(Path(os.environ['FDB_BENCH_CORPUS_INDEX']).read_text(encoding='utf-8'))
    binary = b.ROOT / 'output/maintenance-candidate-cli-20261024.exe'
    before = {p.name:b.sha(p) for p in k.DB.iterdir() if p.suffix in ('.fdb','.ready')}
    digest = hashlib.sha256()
    full, deleted = 0, 0
    stderr = b.LOGS / f'{P}-old-reader-utf8-v2.stderr'
    assert not stderr.exists()
    with stderr.open('w',encoding='utf-8') as err:
        child = subprocess.Popen([str(binary),str(k.DB),'--stdio'],cwd=b.ROOT,
            stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=err,text=True,encoding='utf-8',
            creationflags=subprocess.CREATE_NO_WINDOW)
        def request(value):
            child.stdin.write(json.dumps(value)+'\n')
            child.stdin.flush()
            line = child.stdout.readline()
            digest.update(line.encode('utf-8'))
            response = json.loads(line)
            assert response['ok'],response
            return response['result']
        try:
            corpus_path=Path(os.environ.get('FDB_BENCH_CORPUS_PATH',str(b.ROOT/'data/rkyv-corpus-20261009/json.payload-bench')))
            original_bodies=bool(os.environ.get('FDB_BENCH_ORIGINAL_BODIES'))
            with corpus_path.open('rb') as corpus:
                def expected(ident,row):
                    entry = index['entries'][row['body']]
                    corpus.seek(entry['offset'])
                    payload = corpus.read(entry['size'])
                    assert zlib.crc32(payload)==entry['crc']
                    doc = json.loads(payload)['document']
                    doc['key'] = dict(scope='shared',collection='texts',id=f'{ident:010}')
                    if not original_bodies:
                        doc['text'] = doc['text'].encode('utf-8')[:51140].decode('utf-8',errors='ignore') + f" zqv{ident:010}r{row['revision']:010}zq"
                    elif row['revision'] and os.environ.get('FDB_BENCH_50K_WRITE'):
                        token=f"zqv{ident:010}r{row['revision']:010}zq"
                        doc['text']=doc['text'].encode('utf-8')[:200000-len(token)-1].decode('utf-8',errors='ignore')+' '+token
                    return dict(document=doc,version=row['version'])
                stats = request(dict(op='stats'))
                assert stats['documents']==len(model['rows']) and stats['sequence']==model['sequence']
                for ident,row in model['rows'].items():
                    assert request(dict(op='get',key=dict(scope='shared',collection='texts',id=f'{int(ident):010}')))==expected(int(ident),row)
                    full += 1
                for ident in model['dead']:
                    assert request(dict(op='get',key=dict(scope='shared',collection='texts',id=f'{int(ident):010}'))) is None
                    deleted += 1
                cases = []
                for i,case in enumerate(index['cases']):
                    ids = sorted(int(ident) for ident,row in model['rows'].items() if index['entries'][row['body']]['mask'] & (1<<case['base']))
                    got = request(dict(op='search',query=dict(scope='shared',collection='texts',text=case['text'],offset=case['offset'],limit=case['limit'])))
                    wanted = ids[case['offset']:case['offset']+case['limit']]
                    assert got['total']==len(ids) and got['sequence']==model['sequence']
                    assert got['hits']==[expected(ident,model['rows'][str(ident)]) for ident in wanted]
                    cases.append(dict(case_id=i,total=len(ids),page=len(wanted),oracle=True))
            child.stdin.close()
            assert child.wait(timeout=20)==0
        finally:
            if child.poll() is None:
                child.kill()
                child.wait(timeout=10)
    after = {p.name:b.sha(p) for p in k.DB.iterdir() if p.suffix in ('.fdb','.ready')}
    assert before==after
    result = dict(binary=str(binary.relative_to(b.ROOT)),binary_sha256=b.sha(binary),
                  full_payload_version_gets=full,deleted_gets=deleted,cases=cases,
                  response_stream_sha256=digest.hexdigest().upper(),authoritative_sources_unchanged=True,
                  oracle=True,payload_output_saved=False)
    b.write(b.LOGS/f'{P}-old-reader-utf8-v2.json',result)
    return result


def main():
    if (b.LOGS/f'{P}-summary.json').exists():
        raise RuntimeError('Existing immutable kill evidence')
    design = json.loads((b.LOGS/'block2-scale-20261028-v1-design.json').read_text(encoding='utf-8'))
    assert design['protected_unchanged'] and len(design['completed'])==24
    slot = json.loads((b.LOGS/'block2-scale-20261028-v1-slot-fdb.json').read_text(encoding='utf-8'))
    assert slot['count']==50000
    state = Path(slot['state'])
    os.environ.update(FDB_SCALE_DOCUMENTS='50000',FDB_STREAMED_WAL_LAYER='1',FDB_RECLAMATION_CONTROL='1',
        FDB_BENCH_CORPUS_INDEX=str(b.LOGS/'block2-scale-20261028-v1-corpus-index.json'))
    b.PROGRAMS['fdb'] = b.ROOT/'output/block2-fdb-20261028-v1.exe'
    b.PREFIX = k.PREFIX = P
    protected = b.protected()
    results=[]
    missed=[]
    for stage in ('iterator','flush','merge','cache'):
        recorded=b.LOGS/f'{P}-{stage}-attempt.json'
        if recorded.exists():
            result=json.loads(recorded.read_text(encoding='utf-8'))
            if result['coverage_observed'] and result['reopen_full_payload_matrix_oracle_exit_code']==0:
                results.append(result)
                print(json.dumps(dict(stage=stage,oracle=True,coverage_observed=True,resumed_existing=True)),flush=True)
                continue
            missed.append(dict(stage=stage,attempt=recorded.name,coverage_observed=False,
                oracle_exit_code=result.get('reopen_full_payload_matrix_oracle_exit_code')))
            assert stage=='merge', 'Investigate a new failed recovery window'
            # The legacy helper's extra100 streamed entries stay in the small
            # active WAL. start_compaction() itself does not flush them, so a
            # clean50k layout has no second similarly-sized small immutable run.
            # Retry with the product-default WAL-layer flag, same streamed API,
            # solely for this kill fixture. Preserve the missed attempt intact.
            retry=P+'-merge-retry1'
            retry_record=b.LOGS/f'{retry}-{stage}-attempt.json'
            if retry_record.exists():
                result=json.loads(retry_record.read_text(encoding='utf-8'))
                assert result['coverage_observed'] and result['reopen_full_payload_matrix_oracle_exit_code']==0
                results.append(result)
                print(json.dumps(dict(stage=stage,oracle=True,coverage_observed=True,resumed_retry=True)),flush=True)
                continue
            b.PREFIX=k.PREFIX=retry
            os.environ.pop('FDB_STREAMED_WAL_LAYER',None)
            try:
                result=k.kill_stage(stage,state)
            finally:
                os.environ['FDB_STREAMED_WAL_LAYER']='1'
                b.PREFIX=k.PREFIX=P
            b.write(b.LOGS/f'{retry}-scope.json',dict(
                reason='Legacy kill fixture did not flush the additional small active-WAL transaction; no merge was scheduled',
                missed_attempt=recorded.name,small_active_wal_layer=False,
                persistent_wal_sync_unchanged=True,engine_modified=False))
        else:
            result=k.kill_stage(stage,state)
        assert result['coverage_observed'] and result['reopen_full_payload_matrix_oracle_exit_code']==0
        results.append(result)
        print(json.dumps(dict(stage=stage,oracle=True,coverage_observed=True)),flush=True)
    compat=old_reader(state)
    assert b.protected()==protected
    b.write(b.LOGS/f'{P}-summary.json',dict(results=results,missed_attempts=missed,old_reader=compat,protected_unchanged=True,
        power_loss_claim=False,sqlite_scale_kill_not_measured=True))
    print('50k old-reader full oracle passed',flush=True)


if __name__=='__main__':
    main()
