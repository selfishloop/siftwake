"""Fixed-slot cost diagnosis and normal-build acceptance; no corpus preparation."""
import argparse
import importlib
import json
import os
from pathlib import Path

runner=importlib.import_module('measure-revisions')
bench=runner.bench
ROOT,LOGS=bench.ROOT,bench.LOGS
PREFIX='revisions-cost-10k-20261025'


def configure(label,phase):
    runner.PREFIX=PREFIX+'-'+('compare-v2' if phase=='compare' else 'compare-fixed-v2' if phase=='compare-fixed' else phase)
    runner.FDB=ROOT/f'output/revision-cost-{label}-20261025.exe'
    runner.SQLITE=ROOT/'output/revisions-sqlite-20261025-v2.exe'
    os.environ.pop('FDB_COST_LEGACY',None)
    if label in ['before-profile','reference']:os.environ['FDB_COST_LEGACY']='1'


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--phase',required=True,choices=['before-profile','after-profile','pilot','compare','compare-fixed','long'])
    parser.add_argument('--resume',action='store_true')
    args=parser.parse_args()
    manifest=LOGS/f'{PREFIX}-{"compare-v2" if args.phase=="compare" else "compare-fixed-v2" if args.phase=="compare-fixed" else args.phase}-design.json'
    assert not manifest.exists() or args.resume,'Never overwrite evidence'
    protected=bench.protected()
    design=dict(phase=args.phase,baseline_commit='f7788ca',protected_before=protected,
                corpus='same first10000, original churn donor/revision rules; fixed FDB/SQLite slots',
                cache='warm OS; no background/power isolation',completed=[])
    if args.resume:
        design=json.loads(manifest.read_text(encoding='utf-8'))
        assert design['protected_before']==protected
        design['reference_load_limit']='Only legacy writer32 retained, interrupted reference read calibration at saved model/manifest commit boundary; required candidate read/idle probe remains full38x10 per pair.'
    bench.write(manifest,design)
    orders=([['reference','candidate','sqlite'],['sqlite','candidate','reference']] if args.phase=='compare'
            else [['reference','candidate-fixed'],['candidate-fixed','reference']] if args.phase=='compare-fixed'
            else [[args.phase]])
    if args.phase=='compare-fixed':
        design['sqlite_reference']='Fresh two SQLite churn10/verify runs in compare-v2, unchanged frozen executable. No SQLite database is regenerated for a Rust-only construction-flag correction.'
        design['superseded_candidate']='compare-v2/v4 indexed terms before posting reuse; compare-fixed/v5 changed the wrong bool at SourceLease. Both preserved and excluded. Fixed v6 uses named immutable and records-only constructors, with an output write-denial regression test.'
    for pair,order in enumerate(orders):
        for label in order:
            configure(label,args.phase)
            if args.phase in ['pilot','compare','compare-fixed','long']:
                runner.FDB=ROOT/('output/revision-cost-candidate-20261025-v6.exe' if args.phase in ['compare-fixed','long'] else 'output/revision-cost-candidate-20261025-v4.exe')
            engine='sqlite' if label=='sqlite' else 'fdb'
            design.setdefault('programs',{})[label]=dict(path=str((runner.SQLITE if engine=='sqlite' else runner.FDB).relative_to(ROOT)),
                                                      sha256=bench.sha(runner.SQLITE if engine=='sqlite' else runner.FDB))
            state=LOGS/f'{runner.PREFIX}-r{pair}-{label}-state.json'
            if args.phase.endswith('profile') or args.phase=='pilot':
                modes=['seed','churn','verify']
            elif args.phase=='long':modes=['seed','revisions-30','verify']
            else:modes=(['seed','churn','verify'] if label=='sqlite' else ['seed','revisions-10','verify'] if label=='reference' else ['seed','revisions-10','cost-load','verify'])
            records=[LOGS/f'{runner.PREFIX}-r{pair}-{label}-streamed-{mode}.process.json' for mode in modes]
            if all(p.exists() and json.loads(p.read_text())['exit_code']==0 for p in records):continue
            if records[0].exists():
                pointer=json.loads((LOGS/f'{PREFIX}-slot-{engine}.json').read_text())
                assert args.resume and pointer['label']==label and pointer['pair']==pair
            else:
                bench.reset(engine)
                bench.write(LOGS/f'{PREFIX}-slot-{engine}.json',dict(state=str(state),label=label,phase=args.phase,pair=pair))
            for mode in modes:
                completed=LOGS/f'{runner.PREFIX}-r{pair}-{label}-streamed-{mode}.process.json'
                if completed.exists() and json.loads(completed.read_text())['exit_code']==0:continue
                design['completed'].append(runner.run(label,mode,'streamed',state,pair))
                bench.write(manifest,design)
    design['protected_after']=bench.protected()
    assert design['protected_after']==protected
    design['complete']=True
    bench.write(manifest,design)


if __name__=='__main__':main()
