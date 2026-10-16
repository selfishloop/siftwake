$ErrorActionPreference='Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$prefix='docs/benchmarks/record-reads-10k-20261013'
$programs=@{control='output/record-read-control-20261013.exe';fdb='target/release/examples/measure_content_reads.exe'}
if (Test-Path -LiteralPath "$prefix-before.json") {throw 'Preserve existing report'}
function Snapshot-Sources {
    @(Get-ChildItem -LiteralPath data/benchmark-work/fdb -File |
        Where-Object {$_.Extension -in '.fdb','.ready','.idx','.vfy'} | Sort-Object FullName | ForEach-Object {
            [PSCustomObject]@{path=$_.FullName;bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName).Hash}
        })
}
if (-not (Get-ChildItem -LiteralPath data/benchmark-work/fdb -Filter '*.ready' -File)) {throw 'Existing DB required'}
$before=Snapshot-Sources
$before | ConvertTo-Json | Set-Content "$prefix-before.json" -Encoding utf8
$programs.GetEnumerator() | ForEach-Object {[PSCustomObject]@{engine=$_.Key;path=$_.Value;sha256=(Get-FileHash -LiteralPath $_.Value).Hash}} | ConvertTo-Json | Set-Content "$prefix-executables.json" -Encoding utf8
& $programs.fdb --prepare-record-reads > "$prefix-preparation.jsonl"
if ($LASTEXITCODE -ne 0) {throw 'Record cache preparation failed'}
Get-ChildItem -LiteralPath data/benchmark-work/fdb -File | Where-Object {$_.Extension -in '.rmp','.svfy'} | ForEach-Object {[PSCustomObject]@{path=$_.FullName;bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName).Hash}} | ConvertTo-Json | Set-Content "$prefix-new-caches.json" -Encoding utf8
$modes=@{control='--deferred';fdb='--record-reads'}
foreach ($engine in @('control','fdb')) {
    & $programs[$engine] $modes[$engine] --open-only > "$prefix-$engine-warmup.jsonl"
    if ($LASTEXITCODE -ne 0) {throw 'Warmup failed'}
}
foreach ($n in 0..4) {
    $order=@('control','fdb'); if ($n % 2) {$order=@('fdb','control')}
    foreach ($engine in $order) {
        & $programs[$engine] $modes[$engine] --deferred-probe > "$prefix-first-use-$engine-$n.jsonl"
        if ($LASTEXITCODE -ne 0) {throw 'First use failed'}
    }
    Write-Output "First use pair $n complete"
}
foreach ($n in 0..2) {
    $order=@('control','fdb'); if ($n % 2) {$order=@('fdb','control')}
    foreach ($engine in $order) {
        & $programs[$engine] $modes[$engine] > "$prefix-reads-$engine-$n.jsonl"
        if ($LASTEXITCODE -ne 0) {throw 'Read oracle failed'}
    }
    Write-Output "Warm reads pair $n complete"
}
& $programs.fdb --record-reads --check-integrity > "$prefix-integrity.jsonl"
if ($LASTEXITCODE -ne 0) {throw 'Full integrity failed'}
& output/open-read-control.exe --open-only > "$prefix-old-reader-compatibility.jsonl"
if ($LASTEXITCODE -ne 0) {throw 'Old reader failed'}
$after=Snapshot-Sources
$after | ConvertTo-Json | Set-Content "$prefix-after.json" -Encoding utf8
if ((ConvertTo-Json $before -Compress) -ne (ConvertTo-Json $after -Compress)) {throw 'Source/index/ready/VFY files changed'}
Write-Output 'Existing sources/indexes/ready/VFY unchanged; only small RMP/SVFY metadata added.'
