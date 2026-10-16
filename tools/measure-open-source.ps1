# Read existing immutable sources only. No input generation or DB reset.
$ErrorActionPreference='Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$prefix='docs/benchmarks/open-source-10k-20261012'
if (Test-Path -LiteralPath "$prefix-executables.json") {throw 'Existing report; preserve it'}
$programs=@{control='output/open-read-control.exe';fdb='target/release/examples/measure_content_reads.exe'}
$programs.GetEnumerator() | ForEach-Object {[PSCustomObject]@{engine=$_.Key;path=$_.Value;sha256=(Get-FileHash -LiteralPath $_.Value).Hash}} | ConvertTo-Json | Set-Content "$prefix-executables.json" -Encoding utf8
# Warm OS cache explicitly; these two openings are excluded from samples.
foreach ($engine in @('control','fdb')) {
    & $programs[$engine] --open-only > "$prefix-$engine-warmup.jsonl"
    if ($LASTEXITCODE -ne 0) {throw "$engine warmup failed"}
}
foreach ($n in 0..4) {
    $order=@('control','fdb')
    if ($n % 2) {$order=@('fdb','control')}
    foreach ($engine in $order) {
        & $programs[$engine] --open-only > "$prefix-$engine-$n.jsonl"
        if ($LASTEXITCODE -ne 0) {throw "$engine $n failed"}
    }
    Write-Output "Pair $n completed"
}
$after=@(Get-ChildItem -LiteralPath data/benchmark-work/fdb -File |
    Where-Object {$_.Extension -in '.fdb','.ready','.idx'} | Sort-Object FullName | ForEach-Object {
        [PSCustomObject]@{path=$_.FullName;bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName).Hash}
    })
$after | ConvertTo-Json | Set-Content docs/benchmarks/open-source-after.json -Encoding utf8
$before=Get-Content -LiteralPath docs/benchmarks/open-source-before.json -Raw | ConvertFrom-Json
if ((ConvertTo-Json $before -Compress) -ne (ConvertTo-Json $after -Compress)) {throw 'Persistent database changed'}
Write-Output 'All FDB source/index/ready files unchanged (SHA256).'
