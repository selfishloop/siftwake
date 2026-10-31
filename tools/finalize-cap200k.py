"""Check source immutability and publish current docs after the measured gates."""
import hashlib
import json
import re
from pathlib import Path

R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='cap200k-scale-20261029-v1'


def read(path):return json.loads(path.read_text(encoding='utf-8-sig'))
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest().upper()


def main():
    d=read(L/f'{P}-design.json');a=read(L/f'{P}-analysis.json')
    assert d['protected_unchanged'] and d['provided_source_unchanged'] and a['all_recorded_oracles']
    checks=read(L/'snapshot-10k-20261026-validation-cap200k-20261029-v1.json')
    assert len(checks)==8 and all(x['exit_code']==0 for x in checks)
    now={str(p.relative_to(R)):sha(p) for p in sorted((R/'src').rglob('*.rs'))}
    old=read(L/'posting-pages-50k-20261029-v3-final-audit.json')
    assert now==d['implementation_source_sha256']==old['source_sha256']
    for binary in d['programs'].values():assert sha(R/binary['path'])==binary['sha256']
    accepted=read(L/'cpu-gate-acceptance-20261029-cap200k.json')
    assert sha(R/'docs/POSTING-PAGES-50K-20261029.md')==accepted['original_raw_report_sha256']
    log=(L/'snapshot-10k-20261026-validation-cap200k-20261029-v1-test.log').read_text(encoding='utf-8-sig')
    testcounts=[int(v) for v in re.findall(r'test result: ok\. (\d+) passed',log)]
    assert sum(testcounts)==243,testcounts
    complete=d['gate_failed_at'] is None
    status='GEÇTİ' if complete else f"KALDI ({d['gate_failed_at']} aşaması)"
    link='TRWIKI-CAP200K-SCALE-20261029.md'
    text=f'29 Ekim cap200k yeni taban: **{status}**. '+(
        '50k/100k melez N50 arama, strict-open, temiz idle ve ingest tepe kapıları; oracle/Pending kill/eski okuyucu geçti.200KB uzun-belge iptali ölçüldü. '
        if complete else 'Bağımlı FDB arama/iptal aşamaları durdu; bağımsız ingest/disk/RAM/SQLite tamamlandı. Kötü sonuçlar raporda. ')
    text+=('Düzeltilmiş kullanıcı corpus SHA881756DA… korundu;private/ okunmadı. '+
        'Eski pozitif CPU kapısı yeni mutlak/+%5 kararla geçti; eski ham rapor değişmedi. '+
        'Bu tur motor/biçim/256KiB sınırı değişmedi; exact/serial default,melez200/cancel opt-in,reclaim kapalı. ')
    # Keep earlier decisions/history below a clearly authoritative current entry.
    for name in ['README.md','docs/ARCHITECTURE.md','docs/OPEN-WORK.md']:
        path=R/name;parts=path.read_text(encoding='utf-8').split('\n\n',2)
        target='docs/'+link if name=='README.md' else link
        parts[1]=(text+f'[Tam rapor]({target}). '+
            '50k yazma profili (tekli/dirty500/4R+yazıcı/100lü tail ve RAM kök nedeni),uzun snapshot,gerçek soğuk cache ayrı onay bekler. '+
            'Eski case34 (ishmael /130/1),the zx parçalı okuma,4R uç sorgular/throughput ve signed-zero açık. Ön filtre/Blok3/4/C ABI yok. '+
            'Aşağıdaki önceki sonuçlar ve eski kapı durumları tarihseldir.')
        path.write_text('\n\n'.join(parts),encoding='utf-8')
    report=R/'docs'/link;links=[]
    for target in re.findall(r'\]\(([^)]+)\)',report.read_text(encoding='utf-8')):
        if '://' in target or target.startswith('#'):continue
        path=(report.parent/target.split('#')[0]).resolve();assert path.is_file(),target;links.append(target)
    result=dict(checks=checks,parallel_tests=sum(testcounts),diagnostic_tests='149 accepted prior run; diagnostic engine unchanged; not repeated this measurement-only turn',
        source_sha256=now,engine_source_unchanged=True,frozen_binaries_unchanged=True,old_raw_report_unchanged=True,
        entry_limit_changed=False,persistent_format_changed=False,default_search_changed=False,
        subagents_used=False,corpus_generated=False,private_directory_read=False,
        protected_unchanged=True,provided_source_unchanged=True,gate_failed_at=d['gate_failed_at'],
        report_sha256=sha(report),audit_report_sha256=sha(R/'docs/TRWIKI-CAP200K-AUDIT-20261029.md'),
        links_verified=links,gate_authority='Direct human decisions; primary hybrid200 N50')
    (L/f'{P}-final-audit.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps(dict(status=status,source_files_unchanged=len(now),parallel_tests=sum(testcounts),links=len(links)),ensure_ascii=False))


if __name__=='__main__':main()
