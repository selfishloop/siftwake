param(
    [string]$Prefix='docs/benchmarks/gram-read-cause-profile-10k-20261016',
    [string]$Control='output/gram-cause-cycle-control-20261016.exe',
    [string]$Candidate='output/gram-cause-cycle-candidate-20261016.exe',
    [ValidateRange(2,12)][int]$Pairs=4,
    [ValidateRange(1,2147483647)][long]$AffinityMask=4
)
$ErrorActionPreference='Stop'
Set-Location (Join-Path $PSScriptRoot '..')
if($Pairs%2){throw 'Use an even pair count'}
if(Test-Path -LiteralPath "$Prefix-design.json"){throw 'Preserve existing report'}
$programs=@{control=$Control;fdb=$Candidate}
$programs.GetEnumerator() | ForEach-Object {[PSCustomObject]@{engine=$_.Key;path=$_.Value;sha256=(Get-FileHash -LiteralPath $_.Value).Hash}} | ConvertTo-Json | Set-Content "$Prefix-executables.json" -Encoding utf8
function Snapshot-Files {
    @(Get-ChildItem -LiteralPath data/benchmark-work/fdb -File | Sort-Object Name | ForEach-Object {[PSCustomObject]@{name=$_.Name;bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName).Hash}})
}
$before=@(Snapshot-Files)
$before | ConvertTo-Json | Set-Content "$Prefix-before.json" -Encoding utf8
[PSCustomObject]@{pairs=$Pairs;balanced_first_position=$true;processor_affinity_mask=$AffinityMask;orders=@(foreach($pair in 0..($Pairs-1)){if($pair%2){'fdb/control'}else{'control/fdb'}});cache='warm OS cache, no eviction; untimed per-case warmup';instrumented=$true;iterations_per_page_per_process=30;input='same existing first10000; no database reset or corpus generation';scope='nested wall spans and exact work counts; not CPU attribution or normal-release performance gate'} | ConvertTo-Json -Depth 4 | Set-Content "$Prefix-design.json" -Encoding utf8
foreach($pair in 0..($Pairs-1)){
    $order=@('control','fdb');if($pair%2){$order=@('fdb','control')}
    foreach($engine in $order){
        $launch=[Diagnostics.Stopwatch]::StartNew()
        $process=Start-Process -FilePath (Resolve-Path -LiteralPath $programs[$engine]).Path -ArgumentList '--record-reads','--profile-pages' -PassThru -WindowStyle Hidden -RedirectStandardOutput "$Prefix-$engine-$pair.jsonl" -RedirectStandardError "$Prefix-$engine-$pair.stderr"
        try{
            $process.ProcessorAffinity=[IntPtr]$AffinityMask
            $appliedMs=$launch.Elapsed.TotalMilliseconds
            $actualMask=$process.ProcessorAffinity.ToInt64()
            if($actualMask -ne $AffinityMask){throw 'Affinity was not applied'}
            $process.WaitForExit()
            $process.Refresh()
            [PSCustomObject]@{pid=$process.Id;exit_code=$process.ExitCode;affinity_mask=$actualMask;affinity_applied_after_ms=$appliedMs;wall_ms=$launch.Elapsed.TotalMilliseconds;process_cpu_ms=$process.TotalProcessorTime.TotalMilliseconds;start_utc=$process.StartTime.ToUniversalTime().ToString('o')} | ConvertTo-Json | Set-Content "$Prefix-$engine-$pair-process.json" -Encoding utf8
            if($process.ExitCode -ne 0){throw 'Diagnostic page probe failed'}
        }finally{
            if(-not $process.HasExited){$process.Kill();$process.WaitForExit()}
            $process.Dispose()
        }
    }
    Write-Output "Diagnostic pair $pair complete"
}
$after=@(Snapshot-Files)
$after | ConvertTo-Json | Set-Content "$Prefix-after.json" -Encoding utf8
if((ConvertTo-Json $before -Compress) -ne (ConvertTo-Json $after -Compress)){throw 'Database files changed'}
Write-Output 'FDB and index hashes unchanged.'
