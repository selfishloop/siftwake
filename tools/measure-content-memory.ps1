param([string]$Prefix='content-memory-reuse-10k',
      [string]$FdbExecutable='output/content-memory-fdb-10k-20261011.exe',
      [string]$FdbFeatures='default only',
      [switch]$StreamedInput,
      [string]$SqliteExecutable='output/content-memory-sqlite-10k-20261011.exe',
      [ValidateSet('fdb','sqlite')][string[]]$Engines=@('fdb','sqlite'),
      [ValidateRange(0,3)][int]$IdleRuns=3,
      [ValidateSet('ingest','idle')][string]$Mode='ingest')
$ErrorActionPreference='Stop'
. "$PSScriptRoot/benchmark-workspace.ps1"
. "$PSScriptRoot/windows-process-memory.ps1"
Initialize-FdbProcessMemoryCounters
if(-not (Test-Path -LiteralPath 'data/rkyv-corpus-20261009/json.payload-bench' -PathType Leaf)){throw 'Existing corpus required; do not generate new documents'}
function Measure-ContentProcess([string]$Engine,[string]$Mode,[int]$From,[int]$To,[int]$Run) {
    $probeDb=Get-BenchmarkDatabase $Engine
    $probeExe=[IO.Path]::GetFullPath($SqliteExecutable)
    if($Engine -eq 'fdb'){$probeExe=[IO.Path]::GetFullPath($FdbExecutable)}
    $probeLog=[IO.Path]::GetFullPath("docs/benchmarks/$Prefix-$Engine-$Mode-$To-$Run")
    if(Test-Path -LiteralPath ($probeLog+'.jsonl')){throw 'New prefix required'}
    if($Mode -eq 'ingest') {
        if([IO.DriveInfo]::new([IO.Path]::GetPathRoot($probeDb)).AvailableFreeSpace -lt 12GB){throw 'Insufficient free space before indexing'}
        Reset-BenchmarkDatabase $Engine
    } elseif(-not (Test-Path -LiteralPath $probeDb -PathType Container)){throw 'Existing benchmark database required for reads'}
    $probeArgs=@($Mode,('"'+$probeDb+'"'),$From,$To)
    if($Engine -eq 'fdb' -and $StreamedInput){$probeArgs += '--streamed'}
    $probeStart=Get-Date
    $probeProc=Start-Process -FilePath $probeExe -ArgumentList $probeArgs -WindowStyle Hidden -PassThru -RedirectStandardOutput ($probeLog+'.jsonl') -RedirectStandardError ($probeLog+'.stderr')
    $probeHandle=if($env:OS -eq 'Windows_NT'){$probeProc.Handle}else{[IntPtr]::Zero}
    $probePhase='startup';$probePhases=@{};$probePeak=0L;$probeSamples=0;$probeIdle=$null;$probeLastPhaseRead=$probeStart;$probeLastFree=$probeStart;$probeLowDisk=$false
    while(-not $probeProc.HasExited) {
        $probeNow=Get-Date
        if(($probeNow-$probeLastPhaseRead).TotalMilliseconds -ge 100) {
            # A growing file can yield several appended lines even with -Tail1.
            # Keep a scalar phase; an array becomes a multi-key dictionary index.
            $probeLine=Get-Content -LiteralPath ($probeLog+'.jsonl') -Tail 1 | Select-Object -Last 1
            if($probeLine){
                try {
                    $probeEvent=$probeLine|ConvertFrom-Json -ErrorAction Stop
                    if($probeEvent.phase -is [string] -and $probeEvent.phase.Length){$probePhase=$probeEvent.phase}
                } catch [System.ArgumentException] {
                    # The last append may still be partial; retain the last phase.
                }
            }
            $probeLastPhaseRead=$probeNow
        }
        try {
            $probeProc.Refresh()
            if(-not $probePhases.ContainsKey($probePhase)){$probePhases[$probePhase]=[ordered]@{sampled_peak_private_bytes=0L;sampled_peak_working_set_bytes=0L;samples=0}}
            $probePoint=$probePhases[$probePhase]
            $probePoint.sampled_peak_private_bytes=[Math]::Max($probePoint.sampled_peak_private_bytes,$probeProc.PrivateMemorySize64)
            $probePoint.sampled_peak_working_set_bytes=[Math]::Max($probePoint.sampled_peak_working_set_bytes,$probeProc.WorkingSet64)
            $probePoint.samples++
            $probePeak=[Math]::Max($probePeak,$probeProc.PeakWorkingSet64);$probeSamples++
            if($probePhase -eq 'idle_ready' -and $null -eq $probeIdle){$probeIdle=[ordered]@{private_bytes=$probeProc.PrivateMemorySize64;working_set_bytes=$probeProc.WorkingSet64;elapsed_process_s=($probeNow-$probeStart).TotalSeconds;after_open_wait_s=5;search_calls=0}}
        } catch {if(-not $probeProc.HasExited){throw}}
        if($Mode -eq 'ingest' -and ($probeNow-$probeLastFree).TotalSeconds -ge 2) {
            $probeDrive=[IO.DriveInfo]::new([IO.Path]::GetPathRoot($probeDb))
            if($probeDrive.AvailableFreeSpace -lt 8GB){Stop-Process -Id $probeProc.Id;$probeLowDisk=$true;break}
            $probeLastFree=$probeNow
        }
        Start-Sleep -Milliseconds 20
    }
    $probeProc.WaitForExit()
    $probeLifetimePeaks=Get-FdbProcessMemoryPeaks $probeHandle
    $probeFiles=@(Get-ChildItem -LiteralPath $probeDb -File | Where-Object {$_.Extension -in @('.fdb','.idx','.ready') -or $_.Name -like 'sqlite.db*'} | ForEach-Object {[pscustomobject]@{name=$_.Name;bytes=$_.Length}})
    $probeMemory=[ordered]@{engine=$Engine;mode=$Mode;from=$From;to=$To;run=$Run;exit_code=$probeProc.ExitCode;low_disk_stop=$probeLowDisk;phases=$probePhases;idle=$probeIdle;os_peak_working_set_bytes=$probePeak;samples=$probeSamples;sample_interval_ms=20;phase_poll_ms=100;elapsed_s=((Get-Date)-$probeStart).TotalSeconds;cpu_ms=$probeProc.TotalProcessorTime.TotalMilliseconds;retained_disk_bytes=($probeFiles|Measure-Object -Property bytes -Sum).Sum;files=$probeFiles;features=$(if($Engine -eq 'fdb'){$FdbFeatures}else{'default only'});cache='OS cache, no eviction, no search warmup';includes_os_file_cache=$false;input='streamed CRC-checked JSON corpus, batch500; no books/oracle resident';executable=$probeExe;database=$probeDb}
    $probeMemory['os_lifetime_peaks']=$probeLifetimePeaks
    $probeMemory|ConvertTo-Json -Depth 8|Set-Content -Encoding utf8 -LiteralPath ($probeLog+'.memory.json')
    [pscustomobject]@{engine=$Engine;mode=$Mode;count=$To;run=$Run;exit_code=$probeProc.ExitCode;idle=$probeIdle;peak_private_indexing=$probePhases['indexing'].sampled_peak_private_bytes;disk=$probeMemory.retained_disk_bytes}|ConvertTo-Json -Depth 4
    if($probeProc.ExitCode -ne 0 -or $probeLowDisk){Get-Content -LiteralPath ($probeLog+'.stderr');throw 'Probe failed; all data retained'}
    if($Mode -eq 'idle' -and $null -eq $probeIdle){throw 'Idle marker was not sampled'}
    $probeProc.Dispose()
}
foreach($probeCount in @(10000)) {
    $probeFrom=0
    $probeOrder=$Engines
    if($Mode -eq 'ingest') {foreach($probeEngine in $probeOrder){Measure-ContentProcess $probeEngine 'ingest' $probeFrom $probeCount 0}}
    for($probeRun=0;$probeRun -lt $IdleRuns;$probeRun++) {
        $probeIdleOrder=@($Engines);if($probeRun%2 -eq 1){[Array]::Reverse($probeIdleOrder)}
        foreach($probeEngine in $probeIdleOrder){Measure-ContentProcess $probeEngine 'idle' 0 $probeCount $probeRun}
    }
}
