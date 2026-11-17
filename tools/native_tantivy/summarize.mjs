import { readFileSync, writeFileSync, existsSync, readdirSync } from "node:fs";
import { join } from "node:path";

const dir = process.argv[2];
if (!dir) throw new Error("Expected the benchmark log directory");

function rows(name) {
  const path = join(dir, name + ".jsonl");
  if (!existsSync(path)) return [];
  return readFileSync(path, "utf8").trim().split(/\r?\n/)
    .filter(Boolean).map(line => JSON.parse(line));
}

function stats(values) {
  if (values.length === 0) return null;
  const sorted = values.slice().sort((a, b) => a - b);
  const at = p => sorted[Math.ceil(p * sorted.length) - 1];
  return { n: sorted.length, p50: at(.5), p95: at(.95), max: sorted.at(-1) };
}

const completion = join(dir, "complete.json");
const summary = {
  complete: existsSync(completion),
  arms: {},
};

for (const arm of ["fdb", "tantivy"]) {
  const blocks = [0, 1].map(block => {
    const row = mode => rows(`${block}-${arm}-${mode}`);
    return {
      seed: row("seed")[0],
      verification: row("verify")[0],
      open: Array.from({length: 6}, (_, i) => row("open" + i)).flat(),
      idle: row("idle")[0],
      matrix: row("matrix"),
      visibility: readdirSync(dir)
        .filter(name => name.startsWith(`${block}-${arm}-visibility`) && name.endsWith(".jsonl"))
        .flatMap(name => rows(name.slice(0, -6))),
    };
  });
  const collected = field => blocks.flatMap(block => block[field] ?? []);
  const open = collected("open");
  const matrix = collected("matrix");
  const cases = new Map();
  for (const round of matrix) {
    for (const call of round.calls) {
      const id = call.case;
      if (!cases.has(id)) cases.set(id, { text: call.text, exact: [], capped: [] });
      const entry = cases.get(id);
      entry[call.capped ? "capped" : "exact"].push(call.ms);
      if (!call.capped) {
        entry.offset = call.offset;
        entry.limit = call.limit;
        entry.total = call.total;
      }
    }
  }
  const updates = collected("visibility");
  const visibility = updates.filter(row => row.mode === "visibility");
  summary.arms[arm] = {
    seeds: collected("seed"),
    verified: collected("verification").length,
    idle: collected("idle"),
    roundCountsByBlock: blocks.map(block => block.matrix.length),
    open: stats(open.map(row => row.open_ms)),
    first: stats(open.map(row => row.first_ms)),
    firstQuery: stats(open.map(row => row.first_ms - row.open_ms)),
    firstCpu: stats(open.map(row => row.cpu_ms)),
    caseLatency: Object.fromEntries(
      Array.from(cases.entries()).sort(([a], [b]) => a - b)
        .map(([id, value]) => [id, {
          ...value, exactStats: stats(value.exact), cappedStats: stats(value.capped),
        }]),
    ),
    matrixCpu: stats(matrix.map(row => row.cpu_ms)),
    matrixWall: stats(matrix.map(row => row.wall_ms)),
    visibility: {
      recoveredWithoutTiming: updates.filter(row => row.mode === "visibility_recovered"),
      commit: stats(visibility.map(row => row.commit_ms)),
      publishAfterCommit: stats(visibility.map(row => row.ready_ms - row.commit_ms)),
      visible: stats(visibility.map(row => row.visible_ms)),
    },
  };
}

writeFileSync(join(dir, "summary.json"), JSON.stringify(summary, null, 2) + "\n");

const fdb = summary.arms.fdb.caseLatency;
const tantivy = summary.arms.tantivy.caseLatency;
const sample = stats => stats ? `${stats.p50.toFixed(3)} / ${stats.p95.toFixed(3)}` : "—";
const lines = [
  "# Sorgu süreleri",
  "",
  "Hücreler p50 / p95 (ms). İlk 50 yolunda offset her zaman 0.",
  "",
  "| Case | Sorgu | Exact offset / limit | Toplam | FDB ilk 50 | Tantivy ilk 50 | FDB exact | Tantivy exact |",
  "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |",
];
for (const id of Object.keys(fdb)) {
  if (!tantivy[id]) continue;
  const a = fdb[id];
  const b = tantivy[id];
  if (a.text !== b.text || a.total !== b.total || a.offset !== b.offset || a.limit !== b.limit) {
    throw new Error(`Arm mismatch for case ${id}`);
  }
  const caseText = a.text.replaceAll("|", "\\|");
  lines.push(`| ${id} | ${caseText} | ${a.offset} / ${a.limit} | ${a.total} | ${sample(a.cappedStats)} | ${sample(b.cappedStats)} | ${sample(a.exactStats)} | ${sample(b.exactStats)} |`);
}
writeFileSync(join(dir, "query-matrix.md"), lines.join("\n") + "\n");
