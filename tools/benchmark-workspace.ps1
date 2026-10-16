# The only disposable DB slots used by active 10k benchmarks. Corpus stays intact.
$ErrorActionPreference='Stop'
$BenchmarkRepoRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$BenchmarkWorkspace=[IO.Path]::GetFullPath((Join-Path $BenchmarkRepoRoot 'data/benchmark-work'))
function Get-BenchmarkDatabase([ValidateSet('fdb','sqlite')][string]$Engine) {
    Join-Path $BenchmarkWorkspace $Engine
}
function Reset-BenchmarkDatabase([ValidateSet('fdb','sqlite')][string]$Engine) {
    $dataRoot=[IO.Path]::GetFullPath((Join-Path $BenchmarkRepoRoot 'data'))
    foreach($parent in @($dataRoot,$BenchmarkWorkspace)) {
        if((Test-Path -LiteralPath $parent) -and ((Get-Item -LiteralPath $parent).Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Benchmark parent is a reparse point'}
    }
    $db=[IO.Path]::GetFullPath((Get-BenchmarkDatabase $Engine))
    if([IO.Path]::GetDirectoryName($db) -ne $BenchmarkWorkspace){throw 'Database escapes benchmark workspace'}
    $marker=Join-Path $BenchmarkWorkspace '.fdb-benchmark-owned'
    if(Test-Path -LiteralPath $BenchmarkWorkspace) {
        if(-not (Test-Path -LiteralPath $marker -PathType Leaf) -or (Get-Content -LiteralPath $marker -Raw).Trim() -ne 'FDB benchmark workspace v1'){throw 'Refusing to reset an unowned workspace'}
    } else {
        New-Item -ItemType Directory -Path $BenchmarkWorkspace | Out-Null
        Set-Content -LiteralPath $marker -Value 'FDB benchmark workspace v1' -Encoding utf8
    }
    if(Test-Path -LiteralPath $db) {
        $resolved=(Resolve-Path -LiteralPath $db).Path
        if($resolved -ne $db -or (Get-Item -LiteralPath $db).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Invalid disposable database path'}
        if(Get-ChildItem -LiteralPath $db -Recurse -Force | Where-Object {$_.Attributes -band [IO.FileAttributes]::ReparsePoint}){throw 'Database contains a reparse point'}
        Remove-Item -LiteralPath $db -Recurse
    }
}
