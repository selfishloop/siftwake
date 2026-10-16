param([string]$Database='data/engine-fold-v2-20261011',[string]$Prefix='engine-fold-20261011')
$ErrorActionPreference='Stop'
$engineDb=[IO.Path]::GetFullPath($Database)
for ($engineRun=0;$engineRun -lt 3;$engineRun++) {
    $engineVariants=@('baseline','candidate')
    if ($engineRun % 2 -eq 1) { [Array]::Reverse($engineVariants) }
    foreach ($engineVariant in $engineVariants) {
        $engineExe=[IO.Path]::GetFullPath("output/fdb-engine-fold-$engineVariant-20261011.exe")
        $engineLog=[IO.Path]::GetFullPath("docs/benchmarks/$Prefix-$engineVariant-$engineRun")
        if (Test-Path -LiteralPath ($engineLog+'.jsonl')) { throw 'Use a new output prefix.' }
        $engineProcess=Start-Process -FilePath $engineExe -ArgumentList @(('"'+$engineDb+'"')) -WindowStyle Hidden -PassThru -RedirectStandardOutput ($engineLog+'.jsonl') -RedirectStandardError ($engineLog+'.stderr')
        $enginePeak=0L;$enginePrivate=0L;$engineSamples=0;$engineStart=Get-Date
        while (-not $engineProcess.HasExited) {
            try { $engineProcess.Refresh();$enginePeak=[Math]::Max($enginePeak,$engineProcess.PeakWorkingSet64);$enginePrivate=[Math]::Max($enginePrivate,$engineProcess.PrivateMemorySize64);$engineSamples++ }
            catch { if (-not $engineProcess.HasExited) { throw } }
            Start-Sleep -Milliseconds 20
        }
        $engineProcess.WaitForExit()
        $engineMemory=[ordered]@{variant=$engineVariant;run=$engineRun;exit_code=$engineProcess.ExitCode;executable=$engineExe;database=$engineDb;peak_working_set_bytes=$enginePeak;sampled_peak_private_bytes=$enginePrivate;sample_interval_ms=20;samples=$engineSamples;elapsed_s=((Get-Date)-$engineStart).TotalSeconds;cpu_ms=$engineProcess.TotalProcessorTime.TotalMilliseconds;window='whole process including validated open, corpus plan/books/oracle, queries';includes_os_file_cache=$false}
        $engineMemory | ConvertTo-Json | Set-Content -Encoding utf8 -LiteralPath ($engineLog+'.memory.json')
        $engineMemory | ConvertTo-Json
        if ($engineProcess.ExitCode -ne 0) { Get-Content -LiteralPath ($engineLog+'.stderr');throw 'Engine benchmark failed.' }
    }
}
