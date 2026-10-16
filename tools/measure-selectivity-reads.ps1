# Existing slots only. Identical full matrix/seed for old fold, new fold, SQLite.
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$report = 'docs/benchmarks/selectivity-reads-10k-20261012'
function Snapshot-Databases {
    @(Get-ChildItem -LiteralPath 'data/benchmark-work/fdb','data/benchmark-work/sqlite' -File |
        Where-Object {$_.Extension -in '.fdb','.ready','.idx' -or $_.Name -eq 'sqlite.db'} |
        Sort-Object FullName | ForEach-Object {
            [PSCustomObject]@{path=$_.FullName;bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName).Hash}
        })
}
$programs = @{control='output/selectivity-read-control.exe';fdb='target/release/examples/measure_content_reads.exe';sqlite='tools/native_sqlite/target/release/content_reads.exe'}
if (Test-Path -LiteralPath "$report-before.json") { throw 'Reports already exist; preserve them' }
if (-not (Test-Path -LiteralPath 'data/benchmark-work/sqlite/sqlite.db') -or
    -not (Get-ChildItem -LiteralPath 'data/benchmark-work/fdb' -Filter '*.ready' -File)) { throw 'Existing databases required' }
$before = Snapshot-Databases
$before | ConvertTo-Json -Depth 5 | Set-Content "$report-before.json" -Encoding utf8
$programs.GetEnumerator() | ForEach-Object {[PSCustomObject]@{engine=$_.Key;path=$_.Value;sha256=(Get-FileHash -LiteralPath $_.Value).Hash}} | ConvertTo-Json | Set-Content "$report-executables.json" -Encoding utf8
foreach ($run in @(@('control',0),@('fdb',0),@('sqlite',0),@('sqlite',1),@('fdb',1),@('control',1),@('control',2),@('fdb',2),@('sqlite',2))) {
    $engine=$run[0]; $seed=$run[1]
    & $programs[$engine] --matrix $seed > "$report-$engine-$seed.jsonl" 2> "$report-$engine-$seed.stderr"
    if ($LASTEXITCODE -ne 0) { throw "$engine seed $seed failed" }
    Write-Output "$engine seed $seed complete"
}
$after = Snapshot-Databases
$after | ConvertTo-Json -Depth 5 | Set-Content "$report-after.json" -Encoding utf8
if ((ConvertTo-Json $before -Depth 5 -Compress) -ne (ConvertTo-Json $after -Depth 5 -Compress)) { throw 'Database changed' }
Write-Output 'All persistent files unchanged (SHA256).'
