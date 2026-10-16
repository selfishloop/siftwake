param([string]$Prefix='delta-flush-10k-20261012')
$ErrorActionPreference='Stop'
. "$PSScriptRoot/benchmark-workspace.ps1"
$control='output/content-memory-merge-isolated-10k-20261012.exe'
$candidate='output/content-memory-delta-flush-10k-20261012.exe'
$oldHashes=Get-Content 'docs/benchmarks/merge-isolated-10k-20261012-executables.json' -Raw|ConvertFrom-Json
foreach($entry in $oldHashes){if((Get-FileHash -LiteralPath $entry.Path -Algorithm SHA256).Hash -ne $entry.Hash){throw 'Existing frozen executable changed'}}
$executables=@($control,$candidate,'output/content-memory-merge-profile-isolated-10k-20261012.exe','output/content-memory-delta-flush-profile-10k-20261012.exe','output/content-memory-sqlite-10k-20261011.exe')
$hashes=@($executables|ForEach-Object {Get-FileHash -LiteralPath $_ -Algorithm SHA256})
if(Get-ChildItem docs/benchmarks -Filter "$Prefix-*.jsonl"){throw 'Unused log prefix required; DB slots remain fixed'}
$hashes|ConvertTo-Json|Set-Content "docs/benchmarks/$Prefix-executables.json" -Encoding utf8
# Opposite-order pairs, including two SQLite runs. No per-run database directories.
foreach($run in @(0,1)) {
    if($run -eq 1){./tools/measure-content-memory.ps1 -Prefix "$Prefix-candidate-1" -Engines sqlite -IdleRuns 0}
    $order=@('control','candidate');if($run -eq 1){[Array]::Reverse($order)}
    foreach($variant in $order) {
        $exe=if($variant -eq 'control'){$control}else{$candidate}
        $features=if($variant -eq 'control'){'experimental-merge-postings'}else{'experimental-merge-postings experimental-delta-bitmaps'}
        $engines=if($variant -eq 'candidate' -and $run -eq 0){@('fdb','sqlite')}else{@('fdb')}
        $idleRuns=if($run -eq 0){3}else{0}
        ./tools/measure-content-memory.ps1 -Prefix "$Prefix-$variant-$run" -FdbExecutable $exe -FdbFeatures $features -Engines $engines -IdleRuns $idleRuns
        $db=Get-BenchmarkDatabase fdb
        & $exe verify $db 0 10000 1> "docs/benchmarks/$Prefix-$variant-$run-verify.jsonl" 2> "docs/benchmarks/$Prefix-$variant-$run-verify.stderr"
        if($LASTEXITCODE -ne 0){throw 'Full FDB verification failed; slot retained'}
        if($variant -eq 'candidate') {
            & ./output/sqlite-native-general-long-20261011.exe integrity (Get-BenchmarkDatabase sqlite) $db 1> "docs/benchmarks/$Prefix-sqlite-integrity-$run.jsonl"
            if($LASTEXITCODE -ne 0){throw 'SQLite integrity failed'}
        }
    }
}
foreach($variant in @('control','isolated')) {
    $exe=if($variant -eq 'control'){'output/content-memory-merge-profile-isolated-10k-20261012.exe'}else{'output/content-memory-delta-flush-profile-10k-20261012.exe'}
    if([IO.DriveInfo]::new([IO.Path]::GetPathRoot($BenchmarkWorkspace)).AvailableFreeSpace -lt 12GB){throw 'Insufficient disk space'}
    Reset-BenchmarkDatabase fdb
    $db=Get-BenchmarkDatabase fdb
    $log="docs/benchmarks/$Prefix-profile-$variant"
    & $exe ingest $db 0 10000 1> "$log.jsonl" 2> "$log.stderr"
    if($LASTEXITCODE -ne 0){throw 'Profile failed'}
    & $exe verify $db 0 10000 1> "$log-verify.jsonl" 2> "$log-verify.stderr"
    if($LASTEXITCODE -ne 0){throw 'Profile verification failed'}
    [ordered]@{executable=$exe;database=$db;instrumented=$true;normal_timing_comparison=$false;documents=10000;corpus_reused=$true}|ConvertTo-Json|Set-Content "$log.metadata.json" -Encoding utf8
}
foreach($entry in $hashes){if((Get-FileHash -LiteralPath $entry.Path -Algorithm SHA256).Hash -ne $entry.Hash){throw 'Frozen executable changed'}}
'All matched runs and profiles verified; only two DB slots retained.'
