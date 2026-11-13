#Requires -Version 7.5
# Runs one armed administrator phase. The normal-user runner owns DB reads and timing.
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$repo = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$benchmarks = Join-Path $repo 'docs/benchmarks'
$baseline = Join-Path $benchmarks 'cold-etw-link-20261107'
$output = Join-Path $benchmarks 'cold-etw-main-20261107'
$armPath = Join-Path $output 'arm.json'
if (-not (Test-Path -LiteralPath $armPath -PathType Leaf)) {
    throw 'Ana kosu bu denemeyi henuz hazirlamadi.'
}

$principal = [Security.Principal.WindowsPrincipal]::new(
    [Security.Principal.WindowsIdentity]::GetCurrent()
)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Betigi acik yonetici PowerShell penceresinde calistir.'
}

$arm = Get-Content -LiteralPath $armPath -Raw | ConvertFrom-Json -DateKind String
$trial = [int]$arm.trial
$order = @('fdb', 'sqlite', 'sqlite', 'fdb', 'sqlite', 'fdb', 'fdb', 'sqlite')
if ($arm.version -ne 1 -or $trial -lt 1 -or $trial -gt 8 -or
    $arm.engine -cne $order[$trial - 1] -or $arm.anchor -cne 'tr-case0') {
    throw 'Deneme kimligi veya sira gecersiz.'
}
if ($arm.run_id -cnotmatch '^[0-9a-f]{32}$') {
    throw 'Kosu kimligi gecersiz.'
}

$start = [DateTimeOffset]::Parse($arm.start_utc, [cultureinfo]::InvariantCulture)
$deadline = [DateTimeOffset]::Parse($arm.deadline_utc, [cultureinfo]::InvariantCulture)
$now = [DateTimeOffset]::UtcNow
if ($start.Offset -ne [TimeSpan]::Zero -or $deadline.Offset -ne [TimeSpan]::Zero -or
    $deadline -ne $start.AddMinutes(30) -or $now -lt $start -or
    $now.AddSeconds(75) -ge $start.AddMinutes(17)) {
    throw 'Yeni iz icin UTC penceresi veya kapanis rezervi yetersiz.'
}

foreach ($dir in @($baseline, $output)) {
    $item = Get-Item -LiteralPath $dir
    if (-not $item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw "Cikti dizini gecersiz: $dir"
    }
}
for ($previous = 1; $previous -lt $trial; $previous++) {
    $verdictPath = Join-Path $output ('trial-{0:00}/verdict.json' -f $previous)
    $verdict = Get-Content -LiteralPath $verdictPath -Raw | ConvertFrom-Json
    if ($verdict.run_id -cne $arm.run_id -or $verdict.trial -ne $previous -or
        $verdict.valid -ne $true -or $verdict.merged_events_lost -ne 0 -or
        $verdict.expected_reads_complete -ne $true -or
        $null -eq $verdict.open_start_to_first_ms -or
        $verdict.open_start_to_first_ms -le 0 -or $verdict.open_start_to_first_ms -gt 1000) {
        throw 'Onceki denemenin kaniti veya performans kapisi acik.'
    }
}

function Get-OutputUsage {
    $files = @(Get-ChildItem -LiteralPath $baseline, $output -File -Recurse -Force)
    [pscustomobject]@{
        Bytes = [long]($files | Measure-Object Length -Sum).Sum
        EtlBytes = [long]($files | Where-Object Extension -eq '.etl' |
            Measure-Object Length -Sum).Sum
    }
}

$etlLimit = 256MB
$outputLimit = 512MB
# 72 MiB collector input, a second copy for merge, and 16 MiB overhead reserve.
$traceReserve = 160MB
$usage = Get-OutputUsage
if ($usage.EtlBytes + $traceReserve -gt $etlLimit -or
    $usage.Bytes + $traceReserve -gt $outputLimit) {
    throw 'Gecici collector ve merge icin disk rezervi yetersiz.'
}

$profile = Join-Path $PSScriptRoot 'fdb-cold-etw-bounded.wprp'
$rammap = 'C:\Users\Serkan\Downloads\RAMMap\RAMMap.exe'
if ((Get-AuthenticodeSignature -LiteralPath $rammap).Status -ne 'Valid' -or
    (Get-Item -LiteralPath $rammap).VersionInfo.FileVersion -ne '1.63') {
    throw 'Yerel RAMMap surumu veya imzasi dogrulanamadi.'
}
$wpr = Join-Path $env:windir 'System32/wpr.exe'
$wprStatus = & $wpr -status | Out-String
if ($LASTEXITCODE -ne 0 -or $wprStatus -notmatch 'WPR is not recording') {
    throw 'WPR bos degil veya durumu okunamadi.'
}

$trialDir = Join-Path $output ('trial-{0:00}' -f $trial)
$go = $null
$done = $null
$recording = $false
$claimed = $false
$timer = $null
$result = [ordered]@{
    run_id = $arm.run_id
    trial = $trial
    engine = $arm.engine
    started_utc = [DateTimeOffset]::UtcNow.ToString('o')
    es_started = $false
    es_exit = $null
    et_exit = $null
    wpr_start_exit = $null
    wpr_stop_exit = $null
    failure = $null
}

function Assert-RecordingBudget {
    if ($timer.ElapsedMilliseconds -ge 55000 -or
        [DateTimeOffset]::UtcNow -ge $start.AddMinutes(17).AddSeconds(-15)) {
        throw 'Kayit durdurma rezervine ulasildi.'
    }
    $used = Get-OutputUsage
    # Preserve room for WPR stop/merge while the collector files coexist.
    if ($used.EtlBytes -ge ($etlLimit - 88MB) -or
        $used.Bytes -ge ($outputLimit - 88MB)) {
        throw 'ETL veya toplam disk erken durdurma siniri.'
    }
    foreach ($file in Get-ChildItem -LiteralPath $trialDir -File -Filter '*.etl' -Force) {
        if (($file.Name -like '*System Collector.etl' -and $file.Length -ge 48MB) -or
            ($file.Name -like '*Event Collector.etl' -and $file.Length -ge 24MB)) {
            throw 'Collector dosya sinirina ulasti. Deneme gecersiz.'
        }
    }
}

try {
    $signal = 'Local\FDBColdMain' + $arm.run_id + ('T{0:00}' -f $trial)
    $go = [Threading.EventWaitHandle]::OpenExisting($signal + 'Go')
    $done = [Threading.EventWaitHandle]::OpenExisting($signal + 'Done')
    # Create the per-trial directory once. A second invocation cannot claim it.
    New-Item -Path $trialDir -ItemType Directory -ErrorAction Stop | Out-Null
    $claimed = $true
    $timer = [Diagnostics.Stopwatch]::StartNew()
    & $wpr -start ($profile + '!DiskIOFileIO') -filemode -recordtempto $trialDir
    $result.wpr_start_exit = $LASTEXITCODE
    if ($LASTEXITCODE -ne 0) { throw 'WPR start basarisiz.' }
    $recording = $true

    foreach ($flag in @('-Es', '-Et')) {
        Assert-RecordingBudget
        if ($flag -eq '-Es') { $result.es_started = $true }
        $process = Start-Process -FilePath $rammap -ArgumentList $flag `
            -WindowStyle Hidden -PassThru
        try {
            while (-not $process.WaitForExit(100)) { Assert-RecordingBudget }
            $prefix = $flag.TrimStart('-').ToLowerInvariant()
            $result[$prefix + '_exit'] = $process.ExitCode
            $result[$prefix + '_utc'] = [DateTimeOffset]::UtcNow.ToString('o')
            if ($process.ExitCode -ne 0) { throw "RAMMap $flag basarisiz." }
        } finally {
            $process.Dispose()
        }
    }
    Assert-RecordingBudget
    [void]$go.Set()
    while (-not $done.WaitOne(100)) { Assert-RecordingBudget }
    Assert-RecordingBudget
} catch {
    $result.failure = $_.Exception.Message
} finally {
    if ($recording) {
        $result.stop_request_ms = $timer.ElapsedMilliseconds
        $result.stop_request_utc = [DateTimeOffset]::UtcNow.ToString('o')
        & $wpr -stop (Join-Path $trialDir 'trace.etl') "FDB 2b trial $trial" -skipPdbGen -compress
        $result.wpr_stop_exit = $LASTEXITCODE
        if ($LASTEXITCODE -ne 0) { $result.failure = 'WPR stop basarisiz.' }
    }
    if ($null -ne $go) { $go.Dispose() }
    if ($null -ne $done) { $done.Dispose() }
    if ($claimed) {
        $usage = Get-OutputUsage
        $result.etl_bytes = $usage.EtlBytes
        $result.total_bytes = $usage.Bytes
        $result.ended_utc = [DateTimeOffset]::UtcNow.ToString('o')
        if ($usage.EtlBytes -gt $etlLimit -or $usage.Bytes -gt $outputLimit) {
            $result.failure = 'Kapanista disk siniri asildi.'
        }
        $result | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $trialDir 'admin-result.json')
    }
}
if ($null -ne $result.failure) { throw $result.failure }
$result | ConvertTo-Json
