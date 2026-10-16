param([ValidateSet('short','long')][string]$Phase='short',[string]$Prefix='native-sqlite-general-20261011')
$ErrorActionPreference='Stop'
$savedPayload=$env:FDB_BENCH_PAYLOAD
$savedIntegrity=$env:SQLITE_BENCH_SKIP_INTEGRITY
try {
    $env:FDB_BENCH_PAYLOAD='native'
    $env:SQLITE_BENCH_SKIP_INTEGRITY='1'
    for($benchRun=0;$benchRun -lt 3;$benchRun++) {
        $benchOrder=@('fdb','sqlite')
        if($benchRun % 2 -eq 1){[Array]::Reverse($benchOrder)}
        foreach($benchEngine in $benchOrder) {
            if($Phase -eq 'short') {
                $benchDb=[IO.Path]::GetFullPath("data/$Prefix-short-$benchEngine-$benchRun")
                if(Test-Path -LiteralPath $benchDb){throw 'New short database required'}
                if($benchEngine -eq 'fdb') {
                    $benchExe=[IO.Path]::GetFullPath('output/fdb-native-general-binary-scale-20261011.exe')
                    $benchArgs=@(('"'+$benchDb+'"'),'100000','256','10000','on')
                } else {
                    $benchExe=[IO.Path]::GetFullPath('output/sqlite-native-general-scale-20261011.exe')
                    $benchArgs=@(('"'+$benchDb+'"'),'100000','256','2048')
                }
            } else {
                $benchOracle=[IO.Path]::GetFullPath('data/native-general-long-binary-20261011')
                if($benchEngine -eq 'fdb') {
                    $benchDb=$benchOracle
                    $benchExe=[IO.Path]::GetFullPath('output/fdb-native-general-binary-long-20261011.exe')
                    $benchArgs=@(('"'+$benchDb+'"'))
                } else {
                    $benchDb=[IO.Path]::GetFullPath('data/sqlite-passages-20261011')
                    $benchExe=[IO.Path]::GetFullPath('output/sqlite-native-general-long-20261011.exe')
                    $benchArgs=@('query',('"'+$benchDb+'"'),('"'+$benchOracle+'"'))
                }
            }
            $benchLog=[IO.Path]::GetFullPath("docs/benchmarks/$Prefix-$Phase-$benchEngine-$benchRun")
            if(Test-Path -LiteralPath ($benchLog+'.jsonl')){throw 'New log prefix required'}
            $benchStart=Get-Date
            $benchProc=Start-Process -FilePath $benchExe -ArgumentList $benchArgs -WindowStyle Hidden -PassThru -RedirectStandardOutput ($benchLog+'.jsonl') -RedirectStandardError ($benchLog+'.stderr')
            $benchPeak=0L;$benchPrivate=0L;$benchSamples=0
            while(-not $benchProc.HasExited) {
                try {$benchProc.Refresh();$benchPeak=[Math]::Max($benchPeak,$benchProc.PeakWorkingSet64);$benchPrivate=[Math]::Max($benchPrivate,$benchProc.PrivateMemorySize64);$benchSamples++}
                catch {if(-not $benchProc.HasExited){throw}}
                Start-Sleep -Milliseconds 20
            }
            $benchProc.WaitForExit()
            $benchFiles=@(Get-ChildItem -LiteralPath $benchDb -File | Where-Object {$_.Extension -in @('.fdb','.idx','.ready') -or $_.Name -like 'sqlite.db*'} | ForEach-Object {[pscustomobject]@{name=$_.Name;bytes=$_.Length}})
            $benchMemory=[ordered]@{engine=$benchEngine;run=$benchRun;phase=$Phase;order=$benchOrder;exit_code=$benchProc.ExitCode;executable=$benchExe;database=$benchDb;peak_working_set_bytes=$benchPeak;sampled_peak_private_bytes=$benchPrivate;sample_interval_ms=20;samples=$benchSamples;elapsed_s=((Get-Date)-$benchStart).TotalSeconds;cpu_ms=$benchProc.TotalProcessorTime.TotalMilliseconds;retained_engine_disk_bytes=($benchFiles|Measure-Object -Property bytes -Sum).Sum;files=$benchFiles;features='default only';wal_sync='on';includes_os_file_cache=$false;window='whole native process: ordinary benchmark phases and oracle; separate SQLite integrity excluded'}
            $benchMemory|ConvertTo-Json -Depth 5|Set-Content -Encoding utf8 -LiteralPath ($benchLog+'.memory.json')
            $benchMemory|ConvertTo-Json -Depth 5
            if($benchProc.ExitCode -ne 0){Get-Content -LiteralPath ($benchLog+'.stderr');throw 'Benchmark failed'}
        }
    }
} finally {$env:FDB_BENCH_PAYLOAD=$savedPayload;$env:SQLITE_BENCH_SKIP_INTEGRITY=$savedIntegrity}
