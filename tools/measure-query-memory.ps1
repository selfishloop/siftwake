param(
    [Parameter(Mandatory=$true)][string]$Prefix,
    [Parameter(Mandatory=$true)][string]$Control,
    [Parameter(Mandatory=$true)][string]$Candidate,
    [string]$Sqlite='tools/native_sqlite/target/release/content_reads.exe'
)
$ErrorActionPreference='Stop'
Set-Location (Join-Path $PSScriptRoot '..')
. "$PSScriptRoot/windows-process-memory.ps1"
Initialize-FdbProcessMemoryCounters
function Get-ProgramSha([string]$Path) {
    $hash=[Security.Cryptography.SHA256]::Create()
    $stream=[IO.File]::OpenRead([IO.Path]::GetFullPath($Path))
    try {[BitConverter]::ToString($hash.ComputeHash($stream)).Replace('-','')}
    finally {$stream.Dispose();$hash.Dispose()}
}
if(Test-Path -LiteralPath "$Prefix-design.json"){throw 'Preserve existing reports'}
$programs=@{control=$Control;candidate=$Candidate;sqlite=$Sqlite}
[PSCustomObject]@{
    pairs=4
    scope='normal open plus24 searches/771 gets;no corpus or full payload oracle resident;OS process lifetime peaks'
    cache='same existing first10000 in fixed slots;warm OS cache;no eviction or power isolation'
    validation='strict full source/index open on FDB;ordinary SQLite open;full payload equality checked separately'
    executables=@(foreach($variant in 'control','candidate','sqlite'){
        [PSCustomObject]@{variant=$variant;path=$programs[$variant];sha256=(Get-ProgramSha $programs[$variant])}
    })
} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath "$Prefix-design.json" -Encoding utf8
$records=@()
foreach($pair in 0..3){
    $order=@('control','candidate','sqlite')
    if($pair%2){[Array]::Reverse($order)}
    foreach($variant in $order){
        $stem="$Prefix-$variant-$pair"
        if(Test-Path -LiteralPath "$stem.jsonl"){throw 'Preserve existing log'}
        $proc=Start-Process -FilePath ([IO.Path]::GetFullPath($programs[$variant])) -ArgumentList '--memory-probe' -WindowStyle Hidden -PassThru -RedirectStandardOutput ([IO.Path]::GetFullPath("$stem.jsonl")) -RedirectStandardError ([IO.Path]::GetFullPath("$stem.stderr"))
        $handle=$proc.Handle
        try {
            $proc.WaitForExit()
            if($proc.ExitCode -ne 0){throw "Memory probe failed: $stem"}
            $rows=@(Get-Content -LiteralPath "$stem.jsonl" | ForEach-Object {$_ | ConvertFrom-Json})
            $verified=@($rows | Where-Object phase -eq 'query_memory_verified')
            if($verified.Count -ne 1 -or $verified[0].searches -ne 24 -or $verified[0].gets -ne 771 -or $verified[0].oracle_loaded){throw 'Probe contract mismatch'}
            $opened=$rows | Where-Object phase -eq 'opened'
            $entry=[PSCustomObject]@{variant=$variant;pair=$pair;exit_code=$proc.ExitCode;cpu_ms=$proc.TotalProcessorTime.TotalMilliseconds;os_lifetime_peaks=(Get-FdbProcessMemoryPeaks $handle);opened=$opened;verified=$verified[0]}
            $entry | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath "$stem.memory.json" -Encoding utf8
            $records+=$entry
        } finally {$proc.Dispose()}
    }
    Write-Output "Oracle-free memory pair $pair verified"
}
$records | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath "$Prefix-records.json" -Encoding utf8
