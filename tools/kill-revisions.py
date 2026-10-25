"""Reuse observed kill windows on the candidate; never rewrites old evidence."""
import importlib
import json
import sys
from pathlib import Path

kill = importlib.import_module('measure-churn-kill')
bench = kill.bench
PREFIX = 'revisions-kill-10k-20261025'


def main():
    design = json.loads((bench.LOGS/'revisions-10k-20261025-v2-design.json').read_text(encoding='utf-8'))
    assert design.get('complete')
    bench.PREFIX = PREFIX
    kill.PREFIX = PREFIX
    bench.PROGRAMS['fdb'] = bench.ROOT/'output/revisions-fdb-20261025-v2.exe'
    state = Path(json.loads((bench.LOGS/'revisions-10k-20261025-v2-slot-fdb.json').read_text())['state'])
    protection = bench.protected()
    resume='--resume' in sys.argv
    results = [json.loads((bench.LOGS/f'{PREFIX}-{stage}-attempt.json').read_text(encoding='utf-8'))
               for stage in ['iterator','flush','merge']] if resume else []
    if resume:
        bench.run('fdb','verify','owned',state,'after-harness-interruption')
        kill.pending_marker_oracle(state,'after-harness-interruption')
    for stage in ['iterator','flush','merge','revisions','cache']:
        if resume and stage in ['iterator','flush','merge']:continue
        kill.PREFIX = bench.PREFIX = f'{PREFIX}-retry1' if resume and stage=='revisions' else PREFIX
        result = kill.kill_stage(stage,state)
        assert result['coverage_observed'], result
        results.append(result)
        print(json.dumps({k:result[k] for k in ['stage','coverage_observed','reopen_full_payload_matrix_oracle_exit_code']}),flush=True)
    assert bench.protected() == protection
    bench.write(bench.LOGS/f'{PREFIX}-summary.json',dict(results=results,protected_unchanged=True,power_loss_claim=False))


if __name__ == '__main__':
    main()
