"""Offline historical evidence only, after the larger-sample matrix completes."""
import hashlib
import json
import math
from pathlib import Path

R = Path(__file__).resolve().parent.parent
L = R / 'docs/benchmarks'
P = 'estimator50-20261031'


def main():
    assert json.loads((L / f'{P}-design.json').read_text())['stage'] == 'completed'
    source = L / 'hybrid-serial-extra50-20261030-requests.json'
    requests = json.loads(source.read_text(encoding='utf-8'))
    rows = []
    for case in [10, 13, 16, 25, 34, 38]:
        before = [x for x in requests if x['phase'] == 'snapshot_load' and x['role'] == 'before' and x['case_id'] == case]
        waits = []
        for x in before:
            c = x['query_profile']['counters']
            if c.get('first.permit_condvar_wait', {}).get('calls', 0):
                waits.append(c['first.permit_acquire']['elapsed_ns'] / 1e6)
        after = [x for x in requests if x['phase'] == 'snapshot_load' and x['role'] == 'after' and x['case_id'] == case]
        rows.append(dict(case_id=case, old_reference_requests=len(before), old_contended=len(waits),
                         old_acquired_within_2ms=sum(w <= 2 for w in waits),
                         old_contended_within_2ms_percent=100 * sum(w <= 2 for w in waits) / len(waits) if waits else None,
                         old_acquire_p95_ms=sorted(waits)[math.ceil(.95 * len(waits)) - 1] if waits else None,
                         old_candidate_requests=len(after),
                         old_candidate_handoffs=sum(x['query_profile']['counters'].get('first.hybrid_handoffs', {}).get('calls', 0) for x in after),
                         old_candidate_fallbacks=sum(x['fallback'] for x in after)))
    with source.open('rb') as f:
        digest = hashlib.file_digest(f, 'sha256').hexdigest().upper()
    evidence = dict(source=source.name, source_sha256=digest,
                    scope='30Oct historical profiles; not new N60 profiling or a timed2ms prototype. Resource schedule under a different policy cannot be inferred.',
                    requested_wait_ms=2, implemented=False, rows=rows)
    (L / f'{P}-wait-evidence.json').write_text(json.dumps(evidence, indent=2), encoding='utf-8')
    print(json.dumps(evidence))


if __name__ == '__main__':
    main()
