param([switch]$CompareChunkScan,[switch]$Matrix)
# Existing slots only: no corpus generation, reset, ingest, or maintenance.
$ErrorActionPreference='Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$report='docs/benchmarks/content-reads-10k-20261012'
if($CompareChunkScan){$report='docs/benchmarks/contains-chunk-scan-10k-20261012'}
if($Matrix -and $CompareChunkScan){throw 'Select only one benchmark mode'}
if($Matrix){$report='docs/benchmarks/read-matrix-10k-20261012'}
if(-not (Test-Path -LiteralPath 'data/benchmark-work/sqlite/sqlite.db' -PathType Leaf)) {
    throw 'Existing SQLite database required; no database will be generated'
}
if(-not (Test-Path -LiteralPath 'data/benchmark-work/fdb' -PathType Container) -or
   -not (Get-ChildItem -LiteralPath 'data/benchmark-work/fdb' -Filter '*.ready' -File)) {
    throw 'Existing published FDB database required; no database will be generated'
}
function Snapshot-Databases {
    @(Get-ChildItem -LiteralPath 'data/benchmark-work/fdb','data/benchmark-work/sqlite' -File |
        Where-Object {$_.Extension -in '.fdb','.ready','.idx' -or $_.Name -eq 'sqlite.db'} |
        Sort-Object FullName | ForEach-Object {
            [PSCustomObject]@{path=$_.FullName;bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash}
        })
}
$before=Snapshot-Databases
$before | ConvertTo-Json -Depth 5 | Set-Content "$report-before.json" -Encoding utf8
$programs=@{fdb='target/release/examples/measure_content_reads.exe';sqlite='tools/native_sqlite/target/release/content_reads.exe'}
if($CompareChunkScan){$programs.control='output/content-reads-control-20261012.exe'}
if($Matrix){$programs.control='output/read-matrix-full-scan-control.exe'}
$programs.GetEnumerator() | ForEach-Object {[PSCustomObject]@{engine=$_.Key;path=$_.Value;sha256=(Get-FileHash -LiteralPath $_.Value).Hash}} | ConvertTo-Json | Set-Content "$report-executables.json" -Encoding utf8
$runs=@(@('fdb',0),@('sqlite',0),@('sqlite',1),@('fdb',1),@('fdb',2),@('sqlite',2))
if($CompareChunkScan){$runs=@(@('control',0),@('fdb',0),@('sqlite',0),@('sqlite',1),@('fdb',1),@('control',1))}
if($Matrix){$runs=@(@('control',0),@('fdb',0),@('sqlite',0),@('sqlite',1),@('fdb',1),@('fdb',2),@('sqlite',2),@('sqlite',3),@('fdb',3),@('fdb',4),@('sqlite',4),@('control',1))}
foreach($run in $runs) {
    $engine=$run[0]; $iteration=$run[1]
    if($Matrix -and $engine -eq 'control'){& $programs[$engine] --matrix $iteration --stress-only > "$report-$engine-$iteration.jsonl"}
    elseif($Matrix){& $programs[$engine] --matrix $iteration > "$report-$engine-$iteration.jsonl"}
    else{& $programs[$engine] > "$report-$engine-$iteration.jsonl"}
    if($LASTEXITCODE -ne 0){throw "$engine $iteration failed"}
    Write-Output "$engine $iteration completed"
}
$after=Snapshot-Databases
$after | ConvertTo-Json -Depth 5 | Set-Content "$report-after.json" -Encoding utf8
if((ConvertTo-Json $before -Depth 5 -Compress) -ne (ConvertTo-Json $after -Depth 5 -Compress)){throw 'Persistent database files changed'}
Write-Output 'All persistent source/cache files unchanged (SHA256); read-only runs complete.'
