$ErrorActionPreference='Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$prefix='docs/benchmarks/block-open-10k-20261012'
$programs=@{control='output/block-check-open-control.exe';fdb='target/release/examples/measure_content_reads.exe'}
if (Test-Path -LiteralPath "$prefix-before.json") {throw 'Preserve existing report'}
function Snapshot-Sources {
    @(Get-ChildItem -LiteralPath data/benchmark-work/fdb -File |
        Where-Object {$_.Extension -in '.fdb','.ready','.idx'} | Sort-Object FullName | ForEach-Object {
            [PSCustomObject]@{path=$_.FullName;bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName).Hash}
        })
}
if (-not (Get-ChildItem -LiteralPath data/benchmark-work/fdb -Filter '*.ready' -File)) {throw 'Existing DB required'}
$before=Snapshot-Sources
$before | ConvertTo-Json | Set-Content "$prefix-before.json" -Encoding utf8
$programs.GetEnumerator() | ForEach-Object {[PSCustomObject]@{engine=$_.Key;path=$_.Value;sha256=(Get-FileHash -LiteralPath $_.Value).Hash}} | ConvertTo-Json | Set-Content "$prefix-executables.json" -Encoding utf8
& $programs.fdb --prepare-open-checks > "$prefix-preparation.jsonl"
if ($LASTEXITCODE -ne 0) {throw 'VFY01 preparation failed'}
Get-ChildItem -LiteralPath data/benchmark-work/fdb -Filter '*.vfy' -File | ForEach-Object {[PSCustomObject]@{path=$_.FullName;bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName).Hash}} | ConvertTo-Json | Set-Content "$prefix-block-caches.json" -Encoding utf8
foreach ($engine in @('control','fdb')) {
    & $programs[$engine] --deferred --open-only > "$prefix-$engine-warmup.jsonl"
    if ($LASTEXITCODE -ne 0) {throw 'Warmup failed'}
}
foreach ($n in 0..4) {
    $order=@('control','fdb'); if ($n % 2) {$order=@('fdb','control')}
    foreach ($engine in $order) {
        & $programs[$engine] --deferred --open-only > "$prefix-$engine-$n.jsonl"
        if ($LASTEXITCODE -ne 0) {throw "$engine $n failed"}
    }
    Write-Output "Open pair $n complete"
}
foreach ($n in 0..2) {
    $order=@('control','fdb'); if ($n % 2) {$order=@('fdb','control')}
    foreach ($engine in $order) {
        & $programs[$engine] --deferred --deferred-probe > "$prefix-first-use-$engine-$n.jsonl"
        if ($LASTEXITCODE -ne 0) {throw 'First use failed'}
        & $programs[$engine] --deferred > "$prefix-reads-$engine-$n.jsonl"
        if ($LASTEXITCODE -ne 0) {throw 'Read oracle failed'}
    }
    Write-Output "First use / warm reads pair $n complete"
}
& $programs.fdb --deferred --check-integrity > "$prefix-integrity.jsonl"
if ($LASTEXITCODE -ne 0) {throw 'Full integrity failed'}
# Pre-VFY executable must still read unchanged SEG04 with VFY01 files present.
& output/open-read-control.exe --open-only > "$prefix-old-reader-compatibility.jsonl"
if ($LASTEXITCODE -ne 0) {throw 'Old reader compatibility failed'}
$after=Snapshot-Sources
$after | ConvertTo-Json | Set-Content "$prefix-after.json" -Encoding utf8
if ((ConvertTo-Json $before -Compress) -ne (ConvertTo-Json $after -Compress)) {throw 'Source/index/ready files changed'}
Write-Output 'Existing .fdb/.idx/.ready SHA256 unchanged; only small VFY01 caches added.'
