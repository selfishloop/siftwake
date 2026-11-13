"""Summarize the default-path run without opening a DB."""
import json
from pathlib import Path
from statistics import median
from math import ceil

OUT = Path(__file__).resolve().parent


def distribution(samples):
    values = sorted(samples)
    return {
        "n": len(values),
        "p50": median(values),
        **{label: values[ceil(len(values) * p) - 1]
           for label, p in (("p95", .95), ("p99", .99), ("p99.9", .999))},
        "max": values[-1],
    }


records = [json.loads(line) for line in (OUT / "disk-30.jsonl").read_text(encoding="utf-8").splitlines()]
baseline, = (row for row in records if row["phase"] == "live_disk50_baseline")
rounds = [row for row in records if row["phase"] == "live_disk50_round"]
commits = [dict(entry, round=row["round"]) for row in rounds for entry in row["commits"]]
reads = [row for row in records if row["phase"] == "live_disk50_read"]
write = sum(row["write_ms"] for row in rounds)
idle = sum(row["maintenance_ms"] for row in rounds)
initial = baseline["disk"]["retained_bytes"]
summary = dict(
    rounds=[
        dict(round=row["round"], retained_bytes=row["after"]["retained_bytes"],
             ratio=row["after"]["retained_bytes"] / initial, segments=row["stats"]["segments"],
             tombstones=sum(run["tombstones"] for run in row["liveness"]),
             dead_payload_bytes=sum(run["dead_payload_bytes"] for run in row["liveness"]))
        for row in rounds
    ],
    initial_bytes=initial,
    commit_ms=distribution(row["ms"] for row in commits),
    over_1s=[row for row in commits if row["ms"] > 1000],
    writing_s=write / 1000,
    quiet_s=idle / 1000,
    ops_per_write_s=len(commits) * 100_000 / write,
    ops_per_write_and_quiet_s=len(commits) * 100_000 / (write + idle),
    read_mix_ms={
        phase: distribution(row["wall_ms"] for row in reads if row["load_phase"] % 10 == tag)
        for phase, tag in (("writing", 1), ("quiet", 2))
    },
    os_peak_private_bytes=max(row["memory"]["os_lifetime_peak_private_commit_bytes"] for row in rounds),
    os_peak_working_set_bytes=max(row["memory"]["os_lifetime_peak_working_set_bytes"] for row in rounds),
)
(OUT / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8", newline="\n")
