param([string]$ReportPrefix="docs/benchmarks/deferred-open-10k-20261012-final")
# Same existing DB and executable: strict versus deferred source verification.
$ErrorActionPreference='Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$prefix=$ReportPrefix
$program='target/release/examples/measure_content_reads.exe'
if (Test-Path -LiteralPath "$prefix-before.json") {throw 'Preserve existing report'}
function Snapshot-Fdb {
    @(Get-ChildItem -LiteralPath data/benchmark-work/fdb -File |
        Where-Object {$_.Extension -in '.fdb','.ready','.idx'} | Sort-Object FullName | ForEach-Object {
            [PSCustomObject]@{path=$_.FullName;bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName).Hash}
        })
}
if (-not (Get-ChildItem -LiteralPath data/benchmark-work/fdb -Filter '*.ready' -File)) {throw 'Existing DB required'}
$before=Snapshot-Fdb
$before | ConvertTo-Json | Set-Content "$prefix-before.json" -Encoding utf8
[PSCustomObject]@{path=$program;sha256=(Get-FileHash -LiteralPath $program).Hash} | ConvertTo-Json | Set-Content "$prefix-executable.json" -Encoding utf8
& $program --open-only > "$prefix-strict-warmup.jsonl"
if ($LASTEXITCODE -ne 0) {throw 'strict warmup failed'}
& $program --deferred --open-only > "$prefix-deferred-warmup.jsonl"
if ($LASTEXITCODE -ne 0) {throw 'deferred warmup failed'}
foreach ($n in 0..4) {
    $order=@('strict','deferred')
    if ($n % 2) {$order=@('deferred','strict')}
    foreach ($mode in $order) {
        if ($mode -eq 'strict') {& $program --open-only > "$prefix-$mode-$n.jsonl"}
        else {& $program --deferred --open-only > "$prefix-$mode-$n.jsonl"}
        if ($LASTEXITCODE -ne 0) {throw "$mode $n failed"}
    }
    Write-Output "Pair $n complete"
}
foreach ($n in 0..2) {
    & $program --deferred --deferred-probe > "$prefix-first-use-$n.jsonl"
    if ($LASTEXITCODE -ne 0) {throw 'first use failed'}
}
& $program --deferred --check-integrity > "$prefix-integrity.jsonl"
if ($LASTEXITCODE -ne 0) {throw 'full integrity failed'}
& $program --deferred > "$prefix-read-validation.jsonl"
if ($LASTEXITCODE -ne 0) {throw 'source oracle validation failed'}
$after=Snapshot-Fdb
$after | ConvertTo-Json | Set-Content "$prefix-after.json" -Encoding utf8
if ((ConvertTo-Json $before -Compress) -ne (ConvertTo-Json $after -Compress)) {throw 'Database changed'}
Write-Output 'Source/index/ready SHA256 unchanged; first-use and full check complete.'
