param(
    [string]$Prefix='docs/benchmarks/compact-cache-open-10k-20261017',
    [ValidateSet('open','matrix','dense','profile')][string]$Phase='open',
    [string]$Reader='output/compact-cache-reads-20261017.exe',
    [switch]$ReferencesOnly
)
$ErrorActionPreference='Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$reader=$Reader
$repacker='output/compact-cache-repack-20261017.exe'
if(Test-Path -LiteralPath "$Prefix-design.json"){throw 'Preserve existing report'}
function Snapshot-Sources {
    @(Get-ChildItem -LiteralPath data/benchmark-work/fdb -File | Where-Object {$_.Extension -in @('.fdb','.ready')} | Sort-Object FullName | ForEach-Object {
        [PSCustomObject]@{path=$_.FullName;bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName).Hash}
    })
    $sql=Get-Item -LiteralPath data/benchmark-work/sqlite/sqlite.db
    [PSCustomObject]@{path=$sql.FullName;bytes=$sql.Length;sha256=(Get-FileHash -LiteralPath $sql.FullName).Hash}
}
function Snapshot-Indexes {
    @(Get-ChildItem -LiteralPath data/benchmark-work/fdb -Filter '*.idx' -File | Sort-Object FullName | ForEach-Object {
        [PSCustomObject]@{path=$_.FullName;bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName).Hash}
    })
}
function Run-Probe([string]$Stem,[string[]]$Arguments,[switch]$Memory) {
    $process=Start-Process -FilePath (Resolve-Path -LiteralPath $reader).Path -ArgumentList $Arguments -WindowStyle Hidden -PassThru -RedirectStandardOutput "$Stem.jsonl" -RedirectStandardError "$Stem.stderr"
    $handle=$process.Handle
    try {
        $process.ProcessorAffinity=[IntPtr]4
        $process.WaitForExit()
        $process.Refresh()
        if($Memory){
            [PSCustomObject]@{exit_code=$process.ExitCode;os_lifetime_peaks=(Get-FdbProcessMemoryPeaks $handle)} | ConvertTo-Json -Depth 5 | Set-Content "$Stem.memory.json" -Encoding utf8
        }
        if($process.ExitCode -ne 0){throw "Probe failed: $Stem"}
        $events=@(Get-Content -LiteralPath "$Stem.jsonl" | ForEach-Object {$_ | ConvertFrom-Json})
        if(@($events | Where-Object phase -eq 'opened').Count -ne 1 -or $events[0].pending_source_checks -ne 0){throw 'Expected full strict source validation'}
    } finally {
        if(-not $process.HasExited){$process.Kill();$process.WaitForExit()}
        $process.Dispose()
    }
}
if($Phase -eq 'open'){
    . "$PSScriptRoot/windows-process-memory.ps1"
    Initialize-FdbProcessMemoryCounters
}
$before=@(Snapshot-Sources)
$before | ConvertTo-Json | Set-Content "$Prefix-sources-before.json" -Encoding utf8
$orders=@(@('json','binary','references'),@('references','binary','json'),@('binary','json','references'),@('references','json','binary'))
if($ReferencesOnly){$orders=@(@('json','references'),@('references','json'),@('json','references'),@('references','json'))}
[PSCustomObject]@{phase=$Phase;pairs=4;orders=$orders;pairwise_balanced_order=$true;reader_sha256=(Get-FileHash -LiteralPath $reader).Hash;repacker_sha256=(Get-FileHash -LiteralPath $repacker).Hash;cache='fresh process; warm OS cache; no eviction; untimed strict first-use warmup after each repack';affinity_mask=4;validation='full source CRC/schema/version and full index CRC plus directory/page validation; pending0';corpus='same existing first10000; no corpus preparation or DB copies';write_scope='only fixed-slot derived indexes rewritten between conditions; source/ready/SQLite hashes checked';query_memory='24 searches/771 gets; no corpus/oracle loaded';measurement=$(if($Phase -eq 'profile'){'instrumented strict open-only; nested counters; untimed first-use warmup; no query timing or memory probe'}elseif($Phase -eq 'open'){'strict open+get9999+quixote first/warm, separate lifetime memory'}else{'same38-case full payload/versions oracle; dense selects8 pages,200 samples/page/process'})} | ConvertTo-Json -Depth 6 | Set-Content "$Prefix-design.json" -Encoding utf8
foreach($pair in 0..3){
    foreach($layout in $orders[$pair]){
        & $repacker $layout > "$Prefix-$layout-$pair-repack.jsonl"
        if($LASTEXITCODE -ne 0){throw 'Repack failed'}
        # Independent source/index validation and metadata sidecars are ready
        # before timed reads. Repacking time is never hidden in an open sample.
        Run-Probe "$Prefix-$layout-$pair-warmup" @('--deferred-probe')
        $indexBefore=@(Snapshot-Indexes)
        $indexBefore | ConvertTo-Json | Set-Content "$Prefix-$layout-$pair-indexes.json" -Encoding utf8
        if($Phase -eq 'open'){
            Run-Probe "$Prefix-$layout-$pair" @('--deferred-probe')
            Run-Probe "$Prefix-$layout-$pair-memory" @('--memory-probe') -Memory
        }elseif($Phase -eq 'profile'){
            Run-Probe "$Prefix-$layout-$pair" @('--open-only')
        }else{
            [string[]]$probeArgs=@('--matrix',"$pair")
            if($Phase -eq 'dense'){$probeArgs += '--pages-only'}
            Run-Probe "$Prefix-$layout-$pair" $probeArgs
        }
        $indexAfter=@(Snapshot-Indexes)
        if((ConvertTo-Json $indexBefore -Compress) -ne (ConvertTo-Json $indexAfter -Compress)){throw 'Read changed index/cache files'}
        $after=@(Snapshot-Sources)
        if((ConvertTo-Json $before -Compress) -ne (ConvertTo-Json $after -Compress)){throw 'Source, ready or SQLite changed'}
        Write-Output "Completed $Phase pair $pair layout $layout"
    }
}
$after | ConvertTo-Json | Set-Content "$Prefix-sources-after.json" -Encoding utf8
