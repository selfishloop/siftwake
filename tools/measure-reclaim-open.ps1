param(
    [string]$Prefix='docs/benchmarks/reclaim-open-repeat-10k-20261017',
    [string]$Control='output/seg05-default-reads-20261017.exe',
    [string]$Candidate='output/reclaim-reads-20261017.exe'
)
$ErrorActionPreference='Stop'
Set-Location (Join-Path $PSScriptRoot '..')
if(Test-Path -LiteralPath "$Prefix-design.json"){throw 'Preserve existing report'}
$readers=@{control=$Control;fdb=$Candidate}
function Snapshot-Core {
    @(Get-ChildItem -LiteralPath data/benchmark-work/fdb -File | Sort-Object Name | ForEach-Object {[PSCustomObject]@{name=$_.Name;bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName).Hash}})
}
function Probe([string]$variant,[string]$stem) {
    $p=Start-Process -FilePath (Resolve-Path -LiteralPath $readers[$variant]).Path -ArgumentList '--deferred-probe' -WindowStyle Hidden -PassThru -RedirectStandardOutput "$stem.jsonl" -RedirectStandardError "$stem.stderr"
    try {
        $p.ProcessorAffinity=[IntPtr]4
        $p.WaitForExit()
        if($p.ExitCode -ne 0){throw 'Strict open probe failed'}
    } finally {$p.Dispose()}
}
[PSCustomObject]@{pairs=4;orders=@('control/fdb','fdb/control','control/fdb','fdb/control');cache='warm OS cache; no eviction; fresh normal strict processes; mask4; no hardware/power isolation';validation='full source/index CRC and schema; pending0; get9999/quixote first-use correctness';database='same already cleaned17 core files; no writes/reset/new corpus/DB copies';executables=@(foreach($e in 'control','fdb'){[PSCustomObject]@{variant=$e;path=$readers[$e];sha256=(Get-FileHash -LiteralPath $readers[$e]).Hash}})} | ConvertTo-Json -Depth 5 | Set-Content "$Prefix-design.json" -Encoding utf8
foreach($e in 'control','fdb'){Probe $e "$Prefix-$e-warmup"}
$before=@(Snapshot-Core)
$before | ConvertTo-Json | Set-Content "$Prefix-before.json" -Encoding utf8
foreach($pair in 0..3){
    $order=@('control','fdb');if($pair%2){[Array]::Reverse($order)}
    foreach($e in $order){Probe $e "$Prefix-$e-$pair"}
}
$after=@(Snapshot-Core)
$after | ConvertTo-Json | Set-Content "$Prefix-after.json" -Encoding utf8
if((ConvertTo-Json $before -Compress) -ne (ConvertTo-Json $after -Compress)){throw 'Read changed files'}
