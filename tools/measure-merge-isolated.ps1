param([string]$Prefix='merge-reuse-10k-20261012')
$ErrorActionPreference='Stop'
. "$PSScriptRoot/benchmark-workspace.ps1"
$control='output/content-memory-merge-control-10k-20261012.exe'
$candidate='output/content-memory-merge-isolated-10k-20261012.exe'
$hashes=Get-Content 'docs/benchmarks/merge-isolated-10k-20261012-executables.json' -Raw|ConvertFrom-Json
foreach($entry in $hashes){if((Get-FileHash -LiteralPath $entry.Path -Algorithm SHA256).Hash -ne $entry.Hash){throw 'Frozen executable changed'}}
if(Get-ChildItem -LiteralPath docs/benchmarks -Filter "$Prefix-*.jsonl"){throw 'Choose an unused log prefix; database paths remain fixed'}
# Same first 10k documents in every build. Only two DB slots, never per-run copies.
# Verify each result BEFORE reusing its slot; read/search tests can reuse the final DB.
foreach($run in @(0,1)) {
    $order=@('control','candidate');if($run -eq 1){[Array]::Reverse($order)}
    foreach($variant in $order) {
        $exe=if($variant -eq 'control'){$control}else{$candidate}
        $features=if($variant -eq 'control'){'default only'}else{'experimental-merge-postings'}
        $engines=if($variant -eq 'candidate' -and $run -eq 0){@('fdb','sqlite')}else{@('fdb')}
        $idleRuns=if($run -eq 0){3}else{0}
        ./tools/measure-content-memory.ps1 -Prefix "$Prefix-$variant-$run" -FdbExecutable $exe -FdbFeatures $features -Engines $engines -IdleRuns $idleRuns
        $db=Get-BenchmarkDatabase fdb
        $log="docs/benchmarks/$Prefix-$variant-$run-verify"
        & $exe verify $db 0 10000 1> "$log.jsonl" 2> "$log.stderr"
        if($LASTEXITCODE -ne 0){throw 'Full verification failed; current slot retained'}
        if($engines -contains 'sqlite') {
            & ./output/sqlite-native-general-long-20261011.exe integrity (Get-BenchmarkDatabase sqlite) $db 1> "docs/benchmarks/$Prefix-sqlite-integrity.jsonl"
            if($LASTEXITCODE -ne 0){throw 'SQLite integrity failed'}
        }
    }
}
# Instrumented builds reuse the same FDB slot; no other volume or profile DB copies.
foreach($variant in @('control','isolated')) {
    $exe="output/content-memory-merge-profile-$variant-10k-20261012.exe"
    if([IO.DriveInfo]::new([IO.Path]::GetPathRoot($BenchmarkWorkspace)).AvailableFreeSpace -lt 12GB){throw 'Insufficient profile disk space'}
    Reset-BenchmarkDatabase fdb
    $db=Get-BenchmarkDatabase fdb
    $log="docs/benchmarks/$Prefix-profile-$variant"
    & $exe ingest $db 0 10000 1> "$log.jsonl" 2> "$log.stderr"
    if($LASTEXITCODE -ne 0){throw 'Diagnostic profile failed'}
    & $exe verify $db 0 10000 1> "$log-verify.jsonl" 2> "$log-verify.stderr"
    if($LASTEXITCODE -ne 0){throw 'Diagnostic verification failed'}
    [ordered]@{executable=$exe;database=$db;instrumented=$true;normal_timing_comparison=$false;reason='overlapping diagnostic spans';documents=10000;corpus_reused=$true}|ConvertTo-Json|Set-Content -Encoding utf8 "$log.metadata.json"
}
foreach($entry in $hashes){if((Get-FileHash -LiteralPath $entry.Path -Algorithm SHA256).Hash -ne $entry.Hash){throw 'Frozen executable changed'}}
'All runs and oracles passed. Only final FDB/SQLite slots retained; corpus unchanged.'
