param([string]$Database='data/engine-fold-v2-20261011',[string]$Prefix='engine-binary-20261011',[string]$Executable='output/fdb-engine-binary-20261011.exe',[int]$Runs=3)
$ErrorActionPreference='Stop'
$payloadDb=[IO.Path]::GetFullPath($Database)
$payloadExe=[IO.Path]::GetFullPath($Executable)
$payloadSaved=$env:FDB_BENCH_PAYLOAD
try {
    for($payloadRun=0;$payloadRun -lt $Runs;$payloadRun++) {
        $payloadVariants=@('native','json','binary')
        $payloadOrder=@(0..2 | ForEach-Object {$payloadVariants[($_+$payloadRun)%3]})
        foreach($payloadVariant in $payloadOrder) {
            $env:FDB_BENCH_PAYLOAD=$payloadVariant
            $payloadLog=[IO.Path]::GetFullPath("docs/benchmarks/$Prefix-$payloadVariant-$payloadRun")
            if(Test-Path -LiteralPath ($payloadLog+'.jsonl')) {throw 'Use a new output prefix'}
            $payloadProcess=Start-Process -FilePath $payloadExe -ArgumentList @(('"'+$payloadDb+'"')) -WindowStyle Hidden -PassThru -RedirectStandardOutput ($payloadLog+'.jsonl') -RedirectStandardError ($payloadLog+'.stderr')
            $payloadPeak=0L;$payloadPrivate=0L;$payloadSamples=0;$payloadStart=Get-Date
            while(-not $payloadProcess.HasExited) {
                try {$payloadProcess.Refresh();$payloadPeak=[Math]::Max($payloadPeak,$payloadProcess.PeakWorkingSet64);$payloadPrivate=[Math]::Max($payloadPrivate,$payloadProcess.PrivateMemorySize64);$payloadSamples++}
                catch {if(-not $payloadProcess.HasExited){throw}}
                Start-Sleep -Milliseconds 20
            }
            $payloadProcess.WaitForExit()
            $payloadMemory=[ordered]@{variant=$payloadVariant;run=$payloadRun;order=$payloadOrder;exit_code=$payloadProcess.ExitCode;executable=$payloadExe;database=$payloadDb;peak_working_set_bytes=$payloadPeak;sampled_peak_private_bytes=$payloadPrivate;sample_interval_ms=20;samples=$payloadSamples;elapsed_s=((Get-Date)-$payloadStart).TotalSeconds;cpu_ms=$payloadProcess.TotalProcessorTime.TotalMilliseconds;window='whole process: validated native open + full overlay binding + corpus/oracle + actual Database::search';includes_os_file_cache=$false}
            $payloadMemory | ConvertTo-Json | Set-Content -Encoding utf8 -LiteralPath ($payloadLog+'.memory.json')
            $payloadMemory | ConvertTo-Json
            if($payloadProcess.ExitCode -ne 0){Get-Content -LiteralPath ($payloadLog+'.stderr');throw 'Engine payload benchmark failed'}
        }
    }
} finally {$env:FDB_BENCH_PAYLOAD=$payloadSaved}
