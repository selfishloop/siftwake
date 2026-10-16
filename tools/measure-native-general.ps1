param([ValidateSet('short','long','prepare')][string]$Phase='short',[string]$Prefix='native-general-20261011',[int]$Runs=3)
$ErrorActionPreference='Stop'
$generalSaved=$env:FDB_BENCH_PAYLOAD
try {
    $env:FDB_BENCH_PAYLOAD='native'
    $generalRounds=if($Phase -eq 'prepare'){1}else{$Runs}
    for($generalRun=0;$generalRun -lt $generalRounds;$generalRun++) {
        $generalOrder=@('json','binary')
        if($generalRun % 2 -eq 1){[Array]::Reverse($generalOrder)}
        if($Phase -eq 'prepare'){$generalOrder=@('binary')}
        foreach($generalVariant in $generalOrder) {
            $generalExample=if($Phase -eq 'short'){'scale'}else{'long'}
            $generalExe=[IO.Path]::GetFullPath("output/fdb-native-general-$generalVariant-$generalExample-20261011.exe")
            $generalDb=if($Phase -eq 'short'){"data/$Prefix-short-$generalVariant-$generalRun"}elseif($generalVariant -eq 'json'){'data/engine-fold-v2-20261011'}else{'data/native-general-long-binary-20261011'}
            $generalDb=[IO.Path]::GetFullPath($generalDb)
            if($Phase -eq 'short' -and (Test-Path -LiteralPath $generalDb)){throw 'New short database required'}
            $generalLog=[IO.Path]::GetFullPath("docs/benchmarks/$Prefix-$Phase-$generalVariant-$generalRun")
            if(Test-Path -LiteralPath ($generalLog+'.jsonl')){throw 'New log prefix required'}
            $generalArgs=@(('"'+$generalDb+'"'))
            if($Phase -eq 'short'){$generalArgs+=@('100000','256','10000','on')}
            if($Phase -eq 'prepare'){$generalArgs+=@('prepare')}
            $generalProcess=Start-Process -FilePath $generalExe -ArgumentList $generalArgs -WindowStyle Hidden -PassThru -RedirectStandardOutput ($generalLog+'.jsonl') -RedirectStandardError ($generalLog+'.stderr')
            $generalPeak=0L;$generalPrivate=0L;$generalSamples=0;$generalStarted=Get-Date
            while(-not $generalProcess.HasExited) {
                try {$generalProcess.Refresh();$generalPeak=[Math]::Max($generalPeak,$generalProcess.PeakWorkingSet64);$generalPrivate=[Math]::Max($generalPrivate,$generalProcess.PrivateMemorySize64);$generalSamples++}
                catch {if(-not $generalProcess.HasExited){throw}}
                Start-Sleep -Milliseconds 20
            }
            $generalProcess.WaitForExit()
            $generalFiles=@(Get-ChildItem -LiteralPath $generalDb -File | Where-Object {$_.Extension -in @('.fdb','.idx','.ready')} | ForEach-Object {[pscustomobject][ordered]@{name=$_.Name;bytes=$_.Length}})
            $generalDisk=($generalFiles | Measure-Object -Property bytes -Sum).Sum
            $generalMemory=[ordered]@{variant=$generalVariant;run=$generalRun;phase=$Phase;order=$generalOrder;exit_code=$generalProcess.ExitCode;executable=$generalExe;database=$generalDb;peak_working_set_bytes=$generalPeak;sampled_peak_private_bytes=$generalPrivate;sample_interval_ms=20;samples=$generalSamples;elapsed_s=((Get-Date)-$generalStarted).TotalSeconds;cpu_ms=$generalProcess.TotalProcessorTime.TotalMilliseconds;retained_engine_disk_bytes=$generalDisk;files=$generalFiles;window='whole process including validated opens, corpus/oracle, all requested phases';includes_os_file_cache=$false;features='default only';wal_sync='on'}
            $generalMemory | ConvertTo-Json -Depth 5 | Set-Content -Encoding utf8 -LiteralPath ($generalLog+'.memory.json')
            $generalMemory | ConvertTo-Json -Depth 5
            if($generalProcess.ExitCode -ne 0){Get-Content -LiteralPath ($generalLog+'.stderr');throw 'General benchmark failed'}
        }
    }
} finally {$env:FDB_BENCH_PAYLOAD=$generalSaved}
