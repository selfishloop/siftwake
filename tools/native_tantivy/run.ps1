param(
    [Parameter(Mandatory)] [string] $Binary,
    [switch] $DiagnosePartialTantivy,
    [switch] $ResumeAfterDiagnostic,
    [switch] $ResumeAfterVisibilityError,
    [switch] $ResumeSecondSeed
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
$work = (Resolve-Path -LiteralPath (Join-Path $root 'data/benchmark-work')).Path
$activeFdb = (Resolve-Path -LiteralPath (Join-Path $work 'fdb')).Path
$binaryPath = (Resolve-Path -LiteralPath $Binary).Path
$slots = @{
    fdb = Join-Path $activeFdb 'tantivy-comparison'
    tantivy = Join-Path $work 'tantivy'
}
$report = Join-Path $root 'docs/benchmarks/tantivy-comparison-50k-20261116'
$initial = [DateTimeOffset]::UtcNow
$deadline = $initial.AddMinutes(35)
$owned = @{}

$resuming = $ResumeAfterDiagnostic -or $ResumeAfterVisibilityError -or $ResumeSecondSeed
if (-not $DiagnosePartialTantivy -and -not $resuming) {
    foreach ($path in @($slots.fdb, $slots.tantivy, $report)) {
        if (Test-Path -LiteralPath $path) { throw "Path already exists: $path" }
    }
}
foreach ($path in @($work, $activeFdb)) {
    if ((Get-Item -LiteralPath $path).Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw "Workspace reparse point: $path"
    }
}
if ([IO.DriveInfo]::new([IO.Path]::GetPathRoot($work)).AvailableFreeSpace -lt 8GB) {
    throw 'At least 8 GiB workspace headroom required'
}
[void][IO.Directory]::CreateDirectory($report)

$env:FDB_BENCH_CORPUS_PATH = 'D:\frunner-corpus\trwiki-100k-cap200k\corpus.payload-bench'
$env:FDB_BENCH_CORPUS_INDEX = Join-Path $root 'docs/benchmarks/trwiki-cap200k-20261029-v3-catalog.json'
$env:FDB_SCALE_DOCUMENTS = '50000'
$env:FDB_BENCH_ORIGINAL_BODIES = '1'
$sourceHash = (Get-FileHash -LiteralPath $env:FDB_BENCH_CORPUS_PATH).Hash
$catalogHash = (Get-FileHash -LiteralPath $env:FDB_BENCH_CORPUS_INDEX).Hash
if ($sourceHash -ne '881756DA400952500BA871F6C7BA8FF8D17E5348E5975420BC5CD90952A7CF12') {
    throw "Unexpected corpus hash $sourceHash"
}
$catalogExpected = (Get-Content -Raw -LiteralPath (Join-Path $root 'docs/benchmarks/cap200k-scale-20261029-v1-design.json') | ConvertFrom-Json).catalog_sha256
if ($catalogHash -ne $catalogExpected) { throw "Unexpected catalog hash $catalogHash" }
if (-not $DiagnosePartialTantivy -and -not $resuming) {
[ordered]@{
    utc_start = $initial.ToString('o')
    utc_deadline = $deadline.ToString('o')
    source_sha256 = $sourceHash
    catalog_sha256 = $catalogHash
    binary_sha256 = (Get-FileHash -LiteralPath $binaryPath).Hash
    engine_head = (& git -C $root rev-parse HEAD)
    logical_processors = [Environment]::ProcessorCount
    order = @('fdb','tantivy','tantivy','fdb')
    slots = $slots
    query_samples_per_case_per_arm_per_block = 12
    open_samples_per_arm_per_block = 6
} | ConvertTo-Json -Depth 5 | Set-Content -Encoding UTF8 -LiteralPath (Join-Path $report 'plan.json')
}

function Clear-OwnedSlot($arm) {
    $path = $slots[$arm]
    if ($owned[$arm] -ne $path) { throw "Missing current-run ownership: $path" }
    $resolved = (Resolve-Path -LiteralPath $path).Path
    if ($resolved -ne $path -or [IO.Path]::GetDirectoryName($resolved) -notin @($work,$activeFdb) -or
        ((Get-Item -LiteralPath $resolved).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw "Unexpected cleanup target: $resolved"
    }
    $files = @(Get-ChildItem -Force -LiteralPath $resolved)
    foreach ($file in $files) {
        if ($file.PSIsContainer -or ($file.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
            $file.DirectoryName -ne $resolved) { throw "Unexpected slot entry: $($file.FullName)" }
    }
    foreach ($file in $files) { Remove-Item -LiteralPath $file.FullName -Force }
    Remove-Item -LiteralPath $resolved
    $owned.Remove($arm)
}

function Warm-Files($path) {
    $buffer = [byte[]]::new(1MB)
    foreach ($file in Get-ChildItem -File -Force -LiteralPath $path) {
        $stream = [IO.File]::Open($file.FullName, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
        try { while ($stream.Read($buffer, 0, $buffer.Length) -gt 0) {} }
        finally { $stream.Dispose() }
    }
}

function Run-Step($arm, $mode, $block, $sample = '', $startAt = 0) {
    $tag = "$block-$arm-$mode" + $sample
    if ([DateTimeOffset]::UtcNow -ge $deadline) { throw 'Benchmark deadline reached' }
    $argsLine = @($arm, $mode, ('"' + $slots[$arm] + '"'))
    if ($mode -eq 'matrix') { $argsLine += @($block, 12) }
    if ($mode -eq 'visibility' -and $startAt -ne 0) { $argsLine += $startAt }
    $stdout = Join-Path $report "$tag.jsonl"
    $stderr = Join-Path $report "$tag.stderr"
    Write-Output "$(Get-Date -Format o) $tag"
    $p = Start-Process -FilePath $binaryPath -ArgumentList $argsLine -WorkingDirectory $root -PassThru -WindowStyle Hidden -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    try {
        while (-not $p.WaitForExit(1000)) {
            if ([DateTimeOffset]::UtcNow -ge $deadline) { throw "Deadline while $tag" }
            if ([IO.DriveInfo]::new([IO.Path]::GetPathRoot($work)).AvailableFreeSpace -lt 5GB) {
                throw "Workspace free disk below 5 GiB while $tag"
            }
            $bytes = (Get-ChildItem -Force -File -LiteralPath $slots[$arm] | Measure-Object Length -Sum).Sum
            if ($bytes -gt 1500MB) { throw "Slot data limit exceeded while $tag" }
        }
        $p.WaitForExit()
        if ($p.ExitCode -ne 0) {
            $detail = Get-Content -Raw -LiteralPath $stderr
            throw "$tag exited $($p.ExitCode): $detail"
        }
    } finally {
        if (-not $p.HasExited) { $p.Kill(); $p.WaitForExit() }
        $p.Dispose()
    }
}

if ($DiagnosePartialTantivy) {
    $prior = Get-Content -Raw -LiteralPath (Join-Path $report 'plan.json') | ConvertFrom-Json
    $failure = Get-Content -Raw -LiteralPath (Join-Path $report '0-tantivy-seed.stderr')
    if ($prior.slots.tantivy -ne $slots.tantivy -or $failure -notmatch 'index writer was killed' -or
        (Test-Path -LiteralPath (Join-Path $report 'diagnostic-worker-tantivy-seed.stderr'))) {
        throw 'Expected own failed initial Tantivy seed'
    }
    $owned['tantivy'] = $slots.tantivy
    Clear-OwnedSlot 'tantivy'
    [void][IO.Directory]::CreateDirectory($slots.tantivy)
    $owned['tantivy'] = $slots.tantivy
    Run-Step tantivy seed diagnostic-worker
    return
}

function Recover-Visibility($block, $tag) {
    $prefix = "$block-tantivy-visibility"
    $samples = @(Get-ChildItem -File -LiteralPath $report -Filter "$prefix*.jsonl" |
        Get-Content | ForEach-Object { $_ | ConvertFrom-Json })
    if ($samples.Count -eq 0 -or $samples.Count -ge 24 -or
        (Test-Path -LiteralPath (Join-Path $report "$prefix-$tag.jsonl"))) {
        throw 'Unexpected visibility checkpoint'
    }
    $completed = @($samples | Sort-Object sample | ForEach-Object { $_.sample })
    if (($completed -join ',') -ne ((0..($samples.Count - 1)) -join ',')) {
        throw 'Non-contiguous visibility checkpoint'
    }
    Run-Step tantivy visibility $block "-$tag" $samples.Count
}

function Measure-Ready($arm, $block) {
    Run-Step $arm verify $block
    Warm-Files $slots[$arm]
    for ($i = 0; $i -lt 6; $i++) { Run-Step $arm open $block $i }
    Run-Step $arm idle $block
    Run-Step $arm matrix $block
    try { Run-Step $arm visibility $block }
    catch {
        if ($arm -ne 'tantivy' -or $_.Exception.Message -notmatch 'Access is denied') { throw }
        Recover-Visibility $block retry
    }
}

$startBlock = 0
if ($ResumeSecondSeed) {
    $prior = Get-Content -Raw -LiteralPath (Join-Path $report 'plan.json') | ConvertFrom-Json -DateKind String
    $deadline = [DateTimeOffset]::Parse($prior.utc_deadline, [Globalization.CultureInfo]::InvariantCulture)
    $failure = Get-Content -Raw -LiteralPath (Join-Path $report '1-tantivy-seed.stderr')
    if ($prior.slots.tantivy -ne $slots.tantivy -or $failure -notmatch 'Access is denied|index writer was killed' -or
        (Test-Path -LiteralPath $slots.fdb) -or (Test-Path -LiteralPath (Join-Path $report '1-tantivy-seed-before-resume.stderr'))) {
        throw 'Expected own incomplete second Tantivy seed'
    }
    Move-Item -LiteralPath (Join-Path $report '1-tantivy-seed.stderr') -Destination (Join-Path $report '1-tantivy-seed-before-resume.stderr')
    $owned['tantivy'] = $slots.tantivy
    Clear-OwnedSlot 'tantivy'
    $startBlock = 1
}
if ($ResumeAfterVisibilityError) {
    $prior = Get-Content -Raw -LiteralPath (Join-Path $report 'plan.json') | ConvertFrom-Json -DateKind String
    $deadline = [DateTimeOffset]::Parse($prior.utc_deadline, [Globalization.CultureInfo]::InvariantCulture)
    $failure = Get-Content -Raw -LiteralPath (Join-Path $report '0-tantivy-visibility.stderr')
    if ($prior.slots.tantivy -ne $slots.tantivy -or $failure -notmatch 'Access is denied' -or
        (Test-Path -LiteralPath $slots.fdb)) { throw 'Expected own incomplete first Tantivy visibility' }
    $owned['tantivy'] = $slots.tantivy
    [ordered]@{
        resumed_utc = [DateTimeOffset]::UtcNow.ToString('o')
        binary_sha256 = (Get-FileHash -LiteralPath $binaryPath).Hash
        reason = 'Windows access denied during visibility commit'
    } | ConvertTo-Json | Set-Content -Encoding UTF8 -LiteralPath (Join-Path $report 'visibility-resume.json')
    Recover-Visibility 0 resume
    Clear-OwnedSlot 'tantivy'
    $startBlock = 1
}
if ($ResumeAfterDiagnostic) {
    $prior = Get-Content -Raw -LiteralPath (Join-Path $report 'plan.json') | ConvertFrom-Json -DateKind String
    $deadline = [DateTimeOffset]::Parse($prior.utc_deadline, [Globalization.CultureInfo]::InvariantCulture)
    $successfulSeed = Join-Path $report 'diagnostic-worker-tantivy-seed.jsonl'
    $seed = Get-Content -Raw -LiteralPath $successfulSeed | ConvertFrom-Json
    if ($prior.slots.tantivy -ne $slots.tantivy -or $seed.documents -ne 50000 -or
        (Get-ChildItem -LiteralPath $report -Filter '0-tantivy-verify.jsonl') -or
        (Test-Path -LiteralPath $slots.fdb)) {
        throw 'Expected the successful own diagnostic seed and completed first FDB block'
    }
    Move-Item -LiteralPath (Join-Path $report '0-tantivy-seed.stderr') -Destination (Join-Path $report '0-tantivy-seed-initial-failure.stderr')
    Copy-Item -LiteralPath $successfulSeed -Destination (Join-Path $report '0-tantivy-seed.jsonl')
    $owned['tantivy'] = $slots.tantivy
    [ordered]@{
        resumed_utc = [DateTimeOffset]::UtcNow.ToString('o')
        binary_sha256 = (Get-FileHash -LiteralPath $binaryPath).Hash
        reason = 'Tantivy indexing worker stopped twice. Error reporting added; diagnostic seed then completed.'
    } | ConvertTo-Json | Set-Content -Encoding UTF8 -LiteralPath (Join-Path $report 'resume.json')
    Measure-Ready 'tantivy' 0
    Clear-OwnedSlot 'tantivy'
    $startBlock = 1
}

for ($block = $startBlock; $block -lt 2; $block++) {
    $order = if ($block -eq 0) { @('fdb','tantivy') } else { @('tantivy','fdb') }
    foreach ($arm in $order) {
        [void][IO.Directory]::CreateDirectory($slots[$arm])
        $owned[$arm] = $slots[$arm]
        try { Run-Step $arm seed $block }
        catch {
            if ($arm -ne 'tantivy' -or $_.Exception.Message -notmatch 'Access is denied|index writer was killed') { throw }
            Move-Item -LiteralPath (Join-Path $report "$block-$arm-seed.stderr") -Destination (Join-Path $report "$block-$arm-seed-initial-failure.stderr")
            Clear-OwnedSlot $arm
            [void][IO.Directory]::CreateDirectory($slots[$arm])
            $owned[$arm] = $slots[$arm]
            Run-Step $arm seed $block
        }
        Measure-Ready $arm $block
        if ($arm -eq 'fdb' -or $block -eq 0) { Clear-OwnedSlot $arm }
    }
}
[ordered]@{
    utc_end = [DateTimeOffset]::UtcNow.ToString('o')
    source_unchanged = ((Get-FileHash -LiteralPath $env:FDB_BENCH_CORPUS_PATH).Hash -eq $sourceHash)
    fdb_scratch_removed = -not (Test-Path -LiteralPath $slots.fdb)
    retained_tantivy_bytes = (Get-ChildItem -Force -File -LiteralPath $slots.tantivy | Measure-Object Length -Sum).Sum
} | ConvertTo-Json | Set-Content -Encoding UTF8 -LiteralPath (Join-Path $report 'complete.json')
