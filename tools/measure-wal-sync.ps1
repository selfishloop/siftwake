param([string]$Prefix='wal-sync-20261009')
$ErrorActionPreference='Stop'
$orders=@(@('on','off'),@('off','on'),@('on','off'))
for ($run=0; $run -lt 3; $run++) {
    foreach ($mode in $orders[$run]) {
        ./tools/measure-native.ps1 -Executable output/fdb-wal-sync-scale-20261009.exe -DatabasePath "data/$Prefix-scale-$mode-$run" -Count 100000 -WalSync $mode -OutputPrefix "docs/benchmarks/$Prefix-scale-$mode-$run"
    }
}
foreach ($batch in @(1,50,500)) {
    for ($run=0; $run -lt 3; $run++) {
        foreach ($mode in $orders[$run]) {
            $database="data/$Prefix-commit-$batch-$mode-$run"
            $output="docs/benchmarks/$Prefix-commit-$batch-$mode-$run.json"
            & output/fdb-wal-sync-commit-20261009.exe $database $mode $batch > $output
            if ($LASTEXITCODE -ne 0) { throw "Commit benchmark failed: $database" }
        }
    }
}
