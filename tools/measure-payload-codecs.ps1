param([string]$Corpus='data/rkyv-corpus-20261009',[string]$Prefix='rkyv-payload-v1-20261009',[string]$Executable='output/fdb-rkyv-payload-v1-20261009.exe',[string[]]$Strategies=@(),[int]$Runs=3,[string[]]$Codecs=@('json','rkyv'))
$ErrorActionPreference='Stop'
$exe=[IO.Path]::GetFullPath($Executable)
$root=[IO.Path]::GetFullPath($Corpus)
$labelStrategy=$Strategies.Count -gt 0
if (-not $labelStrategy) { $Strategies=@('baseline') }
if ($Runs -lt 1) { throw 'Runs must be positive.' }
foreach ($codec in $Codecs) { if ($codec -notin @('json','rkyv','binary')) { throw 'Invalid codec.' } }
foreach ($strategy in $Strategies) { if ($strategy -notin @('baseline','ascii','reuse')) { throw 'Invalid strategy.' } }
for ($run=0;$run -lt $Runs;$run++) {
    $codecOrder=@($Codecs);$strategyOrder=@($Strategies)
    if ($run % 2 -eq 1) { [Array]::Reverse($codecOrder);[Array]::Reverse($strategyOrder) }
    foreach ($codec in $codecOrder) {
      foreach ($strategy in $strategyOrder) {
        $tag=$codec
        if ($labelStrategy) { $tag="$codec-$strategy" }
        $output=[IO.Path]::GetFullPath("docs/benchmarks/$Prefix-$tag-$run")
        if (Test-Path -LiteralPath ($output+'.jsonl')) { throw 'Use a new output prefix.' }
        $benchmarkArgs=@($codec,('"'+$root+'"'))
        if ($labelStrategy) { $benchmarkArgs+=$strategy }
        $proc=Start-Process -FilePath $exe -ArgumentList $benchmarkArgs -WindowStyle Hidden -PassThru -RedirectStandardOutput ($output+'.jsonl') -RedirectStandardError ($output+'.stderr')
        $peakWorking=0L;$peakPrivate=0L;$samples=0;$start=Get-Date
        while (-not $proc.HasExited) {
            try { $proc.Refresh();$peakWorking=[Math]::Max($peakWorking,$proc.PeakWorkingSet64);$peakPrivate=[Math]::Max($peakPrivate,$proc.PrivateMemorySize64);$samples++ }
            catch { if (-not $proc.HasExited) { throw } }
            Start-Sleep -Milliseconds 20
        }
        $proc.WaitForExit()
        $result=[ordered]@{exit_code=$proc.ExitCode;codec=$codec;strategy=$strategy;run=$run;executable=$exe;corpus=$root;peak_working_set_bytes=$peakWorking;sampled_peak_private_bytes=$peakPrivate;sample_interval_ms=20;samples=$samples;elapsed_s=((Get-Date)-$start).TotalSeconds;total_cpu_ms=$proc.TotalProcessorTime.TotalMilliseconds;includes_os_file_cache=$false;memory_window='whole measured process: oracle, full open validation, queries and full-page oracle'}
        $result | ConvertTo-Json | Set-Content -Encoding utf8 -LiteralPath ($output+'.memory.json')
        $result | ConvertTo-Json
        if ($proc.ExitCode -ne 0) { Get-Content -LiteralPath ($output+'.stderr');throw 'Payload benchmark failed.' }
      }
    }
}
