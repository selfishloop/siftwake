param(
    [string]$Prefix='docs/benchmarks/stabilize-open-10k-20261016',
    [string]$Control='output/stabilize-control-reads-20261016.exe',
    [string]$Candidate='output/stabilize-default-reads-20261016.exe',
    [ValidateSet(2,4)][int]$Pairs=4
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
[PSCustomObject]@{pairs=$Pairs;orders=@(foreach($pair in 0..($Pairs-1)){if($pair%2){'fdb/control'}else{'control/fdb'}});cache='fresh processes, warm OS cache; no eviction';modes=@('strict','record-first');first_query='quixote; exact total56, first50 and repeated full page; get9999 version20';strict='Database::open with full source verification; no query';record_first='explicit open_record_reads; first get and first search include required checks';diagnostic_timing=$false} | ConvertTo-Json -Depth 4 | Set-Content "$Prefix-design.json" -Encoding utf8
foreach($mode in @('strict','record-first')){
    foreach($pair in 0..($Pairs-1)){
        $order=@('control','fdb');if($pair%2){$order=@('fdb','control')}
        foreach($engine in $order){
            $probeArgs=if($mode -eq 'strict'){@('--open-only')}else{@('--record-reads','--deferred-probe')}
            & $programs[$engine] @probeArgs > "$Prefix-$mode-$engine-$pair.jsonl"
            if($LASTEXITCODE -ne 0){throw 'Open/first-use probe failed'}
        }
    }
    Write-Output "Completed $mode mode"
}
$after=@(Snapshot-OpenFiles)
$after | ConvertTo-Json | Set-Content "$Prefix-after.json" -Encoding utf8
if((ConvertTo-Json $before -Compress) -ne (ConvertTo-Json $after -Compress)){throw 'Read-only open changed database files'}
Write-Output 'Source, index and prepared-cache hashes unchanged.'
