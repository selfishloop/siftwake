param(
    [string]$Prefix='docs/benchmarks/stabilize-open-10k-20261016',
    [string]$Control='output/stabilize-control-reads-20261016.exe',
    [string]$Candidate='output/stabilize-default-reads-20261016.exe',
    [ValidateSet(2,4)][int]$Pairs=4,
    [switch]$StrictFirstUse,
    [switch]$Warmup,
    [switch]$MeasureQueryMemory
)
$ErrorActionPreference='Stop'
Set-Location (Join-Path $PSScriptRoot '..')
if(Test-Path -LiteralPath "$Prefix-design.json"){throw 'Preserve existing report'}
function Snapshot-OpenFiles {
    @(Get-ChildItem -LiteralPath data/benchmark-work/fdb -File | Sort-Object FullName | ForEach-Object {[PSCustomObject]@{path=$_.FullName;bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName).Hash}})
}
$programs=@{control=$Control;fdb=$Candidate}
$before=@(Snapshot-OpenFiles)
$before | ConvertTo-Json | Set-Content "$Prefix-before.json" -Encoding utf8
$programs.GetEnumerator() | ForEach-Object {[PSCustomObject]@{engine=$_.Key;path=$_.Value;sha256=(Get-FileHash -LiteralPath $_.Value).Hash}} | ConvertTo-Json | Set-Content "$Prefix-executables.json" -Encoding utf8
$modes=@('strict','record-first');if($StrictFirstUse){$modes=@('strict','strict-first','record-first')}
function Assert-OpenMode([string]$Path,[string]$Mode){
    $events=@(Get-Content -LiteralPath $Path | ForEach-Object {$_ | ConvertFrom-Json})
    $opened=@($events | Where-Object phase -eq 'opened')
    if($opened.Count -ne 1){throw 'Missing open result'}
    if($events.phase -contains 'oracle' -or $events.phase -contains 'reads'){throw 'Unexpected oracle/query series: check native argument array'}
    if($Mode -eq 'strict'){
        if($events.Count -ne 1 -or $opened[0].pending_source_checks -ne 0){throw 'Strict open-only probe did not run'}
    }elseif(@($events | Where-Object phase -eq 'deferred_first_use').Count -ne 1){
        throw 'First-use probe did not run'
    }
}
[PSCustomObject]@{pairs=$Pairs;orders=@(foreach($pair in 0..($Pairs-1)){if($pair%2){'fdb/control'}else{'control/fdb'}});cache='fresh processes, warm OS cache; no eviction';modes=$modes;explicit_untimed_warmup=[bool]$Warmup;query_memory_pairs=$(if($MeasureQueryMemory){2}else{0});first_query='quixote; exact total56, first50 and repeated full page; get9999 version20';strict='Database::open with full source verification; no query';strict_first='normal strict open then get9999 and first/warm quixote';record_first='explicit open_record_reads; first get and first search include required checks';diagnostic_timing=$false} | ConvertTo-Json -Depth 4 | Set-Content "$Prefix-design.json" -Encoding utf8
foreach($mode in $modes){
    [string[]]$probeArgs=@(switch($mode){'strict'{@('--open-only')};'strict-first'{@('--deferred-probe')};'record-first'{@('--record-reads','--deferred-probe')}})
    if($Warmup){
        foreach($engine in @('control','fdb')){
            & $programs[$engine] @probeArgs > "$Prefix-$mode-$engine-warmup.jsonl"
            if($LASTEXITCODE -ne 0){throw 'Open warmup failed'}
            Assert-OpenMode "$Prefix-$mode-$engine-warmup.jsonl" $mode
        }
    }
    foreach($pair in 0..($Pairs-1)){
        $order=@('control','fdb');if($pair%2){$order=@('fdb','control')}
        foreach($engine in $order){
            & $programs[$engine] @probeArgs > "$Prefix-$mode-$engine-$pair.jsonl"
            if($LASTEXITCODE -ne 0){throw 'Open/first-use probe failed'}
            Assert-OpenMode "$Prefix-$mode-$engine-$pair.jsonl" $mode
        }
    }
    Write-Output "Completed $mode mode"
}
if($MeasureQueryMemory){
    . "$PSScriptRoot/windows-process-memory.ps1"
    Initialize-FdbProcessMemoryCounters
    foreach($pair in 0..1){
        $order=@('control','fdb');if($pair%2){[Array]::Reverse($order)}
        foreach($engine in $order){
            $stem="$Prefix-memory-$engine-$pair"
            $process=Start-Process -FilePath (Resolve-Path -LiteralPath $programs[$engine]).Path -ArgumentList '--memory-probe' -WindowStyle Hidden -PassThru -RedirectStandardOutput "$stem.jsonl" -RedirectStandardError "$stem.stderr"
            $handle=$process.Handle
            try{
                $process.WaitForExit()
                [PSCustomObject]@{exit_code=$process.ExitCode;os_lifetime_peaks=(Get-FdbProcessMemoryPeaks $handle);scope='strict open plus24 queries/771 gets, no corpus/oracle loaded'} | ConvertTo-Json -Depth 5 | Set-Content "$stem.memory.json" -Encoding utf8
                if($process.ExitCode -ne 0){throw 'Open/query memory probe failed'}
            }finally{$process.Dispose()}
        }
    }
}
$after=@(Snapshot-OpenFiles)
$after | ConvertTo-Json | Set-Content "$Prefix-after.json" -Encoding utf8
if((ConvertTo-Json $before -Compress) -ne (ConvertTo-Json $after -Compress)){throw 'Read-only open changed database files'}
Write-Output 'Source, index and prepared-cache hashes unchanged.'
