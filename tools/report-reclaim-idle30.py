"""Report every measured round; no hard gate is inferred from missing data."""
import importlib
import json

b=importlib.import_module('measure-churn')
P='reclaim-idle30-10k-20261027'


def main():
    result=dict(trials=[])
    text=['# Gerçek10k /30 tur: tur başına bakım ve2sn idle','',
          'Retained oranı bakım sonrası başlangıç diskine göredir. MB=10^6byte; MiB=2^20byte. Bellek OS lifetime peak olup harness/model dahil kümülatiftir.',
          'Tombstone sütunu bütün tutulan segmentlerdeki kayıt adedidir. shared/texts/10haneliID için kayıt başına SourceDelete38byte+SEG05record58byte=96byte türetilir; MET04 kopyaları/ortak framing dahil değildir. Source metadata üst sınırı diğer kaynak metadatasını da içerir.','',
          '| Kol/tur | Bakım öncesi /sonrası disk MB | Sonra oran | Ölü payload MB /kayıt | Tombstone | Source metadata üst sınır MB | İndeks MB | Commit toplam /bakım ms | Peak private /WS MiB |',
          '|---|---:|---:|---:|---:|---:|---:|---:|---:|']
    for path in sorted(b.LOGS.glob(f'{P}-*-write-churn-idle-30.jsonl')):
        rows=[]
        for line in path.read_text(encoding='utf-8').splitlines():
            try:rows.append(json.loads(line))
            except json.JSONDecodeError:continue
        baseline=next((r for r in rows if r['phase']=='write_churn_baseline'),None)
        if baseline is None:continue
        initial=baseline['disk']['retained_bytes']
        label=path.name[len(P)+1:].split('-streamed-')[0]
        rounds=[]
        for raw in [r for r in rows if r['phase']=='write_churn_round']:
            live=raw['liveness_after']
            r=dict(round=raw['round'],before_bytes=raw['before']['retained_bytes'],after_bytes=raw['after']['retained_bytes'],
                   ratio=raw['after']['retained_bytes']/initial,commit_ms=sum(raw['commits_ms']),commit_max_ms=max(raw['commits_ms']),
                   maintenance_ms=raw['maintenance_ms'],idle_ms=raw['idle_ms'],memory=raw['memory'],stats=raw['stats'])
            if live is not None:
                r.update(dead_payload_bytes=sum(s['dead_payload_bytes'] for s in live),dead_records=sum(s['dead_records'] for s in live),
                         tombstones=sum(s['tombstones'] for s in live),index_bytes=sum(s['index_bytes'] for s in live),
                         source_metadata_upper_bound_bytes=sum(s['source_bytes']-s['payload_bytes'] for s in live))
                r['tombstone_explicit_source_entry_bytes']=38*r['tombstones']
                r['tombstone_explicit_seg05_record_bytes']=58*r['tombstones']
            rounds.append(r)
            dead=f'{r["dead_payload_bytes"]/1e6:.3f} /{r["dead_records"]}' if live is not None else 'N/A'
            tomb=str(r['tombstones']) if live is not None else 'N/A'
            meta=f'{r["source_metadata_upper_bound_bytes"]/1e6:.3f}' if live is not None else 'N/A'
            index=f'{r["index_bytes"]/1e6:.3f}' if live is not None else 'N/A'
            m=r['memory']
            text.append(f'| {label}/{r["round"]} | {r["before_bytes"]/1e6:.3f} /{r["after_bytes"]/1e6:.3f} | {r["ratio"]:.4f} | {dead} | {tomb} | {meta} | {index} | {r["commit_ms"]:.1f} /{r["maintenance_ms"]:.1f} | {m["os_lifetime_peak_private_commit_bytes"]/2**20:.2f} /{m["os_lifetime_peak_working_set_bytes"]/2**20:.2f} |')
        marker=next((r for r in rows if r['phase']=='write_churn_marker_oracle'),None)
        complete=any(r['phase']=='write_churn_verified' and r['oracle'] for r in rows)
        trial=dict(label=label,initial_bytes=initial,rounds=rounds,complete=complete,marker_oracle=marker,
                   failed_disk_rounds=[r['round'] for r in rounds if r['ratio']>1.3])
        # The runner label precedes the backend name in the raw mode filename.
        sample_path=b.LOGS/f'{P}-{label.rsplit("-",1)[0]}-disk-samples.jsonl'
        if sample_path.exists():
            samples=[json.loads(s) for s in sample_path.read_text(encoding='utf-8').splitlines()]
            trial['sampled_peak_bytes']=max(s['bytes'] for s in samples)
            trial['sampled_peak_ratio']=trial['sampled_peak_bytes']/initial
        if rounds and 'tombstones' in rounds[-1]:
            tail=rounds[-10:]
            trial['last10']=dict(tombstones=[r['tombstones'] for r in tail],dead_payload_bytes=[r['dead_payload_bytes'] for r in tail],
                                 index_bytes=[r['index_bytes'] for r in tail],ratios=[r['ratio'] for r in tail])
        result['trials'].append(trial)
    b.write(b.LOGS/f'{P}-summary.json',result)
    (b.LOGS/f'{P}-tables.md').write_text('\n'.join(text)+'\n',encoding='utf-8')
    for t in result['trials']:
        print(json.dumps(dict(label=t['label'],rounds=len(t['rounds']),complete=t['complete'],failed=t['failed_disk_rounds'],
                   final_ratio=t['rounds'][-1]['ratio'] if t['rounds'] else None,last10=t.get('last10'))),flush=True)


if __name__=='__main__':main()
