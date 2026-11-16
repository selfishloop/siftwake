#Requires -Version 7.5
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$root = $PSScriptRoot
$who = [Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $who.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Bu betiği açık yönetici PowerShell penceresinden çalıştırın.'
}
$plan = Get-Content -LiteralPath (Join-Path $root 'plan.json') -Raw | ConvertFrom-Json -DateKind String
if (-not (Get-Process -Id $plan.controller_pid -ErrorAction SilentlyContinue)) { throw 'Ölçüm denetleyicisi çalışmıyor.' }
$deadline = [DateTimeOffset]::Parse($plan.deadline_utc)
if ([DateTimeOffset]::UtcNow -ge $deadline) { throw 'Ölçüm penceresi kapandı.' }
$profile = Join-Path $root 'cold-identity.wprp'
$rammap = 'C:\Users\Serkan\Downloads\RAMMap\RAMMap.exe'
$wpr = Join-Path $env:windir 'System32/wpr.exe'
if ((Get-FileHash -LiteralPath $profile).Hash -ne $plan.profile_sha) { throw 'WPR profili değişmiş.' }
if ((Get-Item -LiteralPath $rammap).VersionInfo.FileVersion -ne '1.63' -or
    (Get-AuthenticodeSignature -LiteralPath $rammap).Status -ne 'Valid') { throw 'RAMMap 1.63 doğrulanamadı.' }
$claim = [IO.File]::Open((Join-Path $root 'admin-claimed'),[IO.FileMode]::CreateNew)
$claim.Dispose()

function Write-Signal([string]$Dir, [string]$Name) {
    [IO.File]::WriteAllBytes((Join-Path $Dir $Name), [byte[]]@())
}

function Still-Armed {
    return [DateTimeOffset]::UtcNow -lt $deadline -and
        (Get-Process -Id $plan.controller_pid -ErrorAction SilentlyContinue)
}

function Invoke-ColdTrial([string]$Dir, $Arm) {
    $result = [ordered]@{engine=$Arm.engine; trial=$Arm.trial; started_utc=[DateTimeOffset]::UtcNow.ToString('o')
        start_exit=$null; es_exit=$null; et_exit=$null; stop_exit=$null; stop_status_idle=$false
        merged_bytes=0; failure=$null; collector_near_cap=$false}
    $recording = $false
    try {
        $traceBytes = [long](Get-ChildItem -LiteralPath $root -Recurse -File -Filter '*.etl' |
            Measure-Object Length -Sum).Sum
        $outputBytes = [long](Get-ChildItem -LiteralPath $root -Recurse -File |
            Measure-Object Length -Sum).Sum
        $idle = (& $wpr -status | Out-String) -match 'WPR is not recording'
        if ($idle -and $traceBytes + 196MB -le 320MB -and $outputBytes + 196MB -le 512MB) {
            & $wpr -start ($profile + '!ColdIdentity') -filemode -recordtempto $Dir
            $result.start_exit = $LASTEXITCODE
            $recording = $LASTEXITCODE -eq 0
            if (-not $recording) { $result.failure = 'WPR start' }
        } else {
            $result.failure = 'WPR busy or ETL reserve unavailable'
        }
        foreach ($flag in '-Es','-Et') {
            $key = $flag.TrimStart('-').ToLowerInvariant()
            $ram = Start-Process -FilePath $rammap -ArgumentList $flag -WindowStyle Hidden -PassThru
            try {
                if (-not $ram.WaitForExit(20000)) {
                    $result.failure = 'RAMMap timeout'
                    break
                }
                $result[$key + '_exit'] = $ram.ExitCode
                $result[$key + '_utc'] = [DateTimeOffset]::UtcNow.ToString('o')
                if ($ram.ExitCode -ne 0) {
                    $result.failure = "RAMMap $flag exit $($ram.ExitCode)"
                    break
                }
            } finally { $ram.Dispose() }
        }
        Write-Signal $Dir 'go'
        if ($recording) {
            $timer = [Diagnostics.Stopwatch]::StartNew()
            while (-not (Test-Path -LiteralPath (Join-Path $Dir 'first-ready'))) {
                if (-not (Still-Armed) -or $timer.Elapsed.TotalSeconds -ge 30) {
                    $result.failure = 'Trace window ended before first result'
                    break
                }
                if (@(Get-ChildItem -LiteralPath $Dir -File -Filter '*.etl' | Where-Object Length -ge 94MB).Count) {
                    $result.collector_near_cap = $true
                    $result.failure = 'Collector near limit before first result'
                    break
                }
                Start-Sleep -Milliseconds 30
            }
        }
    } catch {
        $result.failure = $_.Exception.Message
    } finally {
        if (-not (Test-Path -LiteralPath (Join-Path $Dir 'go'))) { Write-Signal $Dir 'go' }
        $trace = Join-Path $Dir 'preflight.etl'
        if ($recording) {
            $result.stop_request_utc = [DateTimeOffset]::UtcNow.ToString('o')
            try {
                & $wpr -stop $trace "Cold open trial $($Arm.trial) $($Arm.engine)" -skipPdbGen -compress
                $result.stop_exit = $LASTEXITCODE
                if ($LASTEXITCODE -ne 0 -and -not $result.failure) { $result.failure = 'WPR stop' }
            } catch { $result.failure = $_.Exception.Message }
        }
        $result.stop_status_idle = (& $wpr -status | Out-String) -match 'WPR is not recording'
        if (Test-Path -LiteralPath $trace -PathType Leaf) { $result.merged_bytes = (Get-Item -LiteralPath $trace).Length }
        $result.ended_utc = [DateTimeOffset]::UtcNow.ToString('o')
        $result | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Dir 'admin-result.pending') -Encoding utf8
        [IO.File]::Move((Join-Path $Dir 'admin-result.pending'), (Join-Path $Dir 'admin-result.json'))
        Write-Signal $Dir 'trace-finished'
    }
}

for ($i=1; $i -le $plan.order.Count; $i++) {
    $dir = Join-Path $root ('trial-{0:00}' -f $i)
    $ready = Join-Path $dir 'ready.json'
    while (-not (Test-Path -LiteralPath $ready -PathType Leaf)) {
        if (-not (Still-Armed) -or (Test-Path -LiteralPath (Join-Path $root 'done'))) { return }
        Start-Sleep -Milliseconds 100
    }
    if (-not (Still-Armed)) { return }
    $arm = Get-Content -LiteralPath $ready -Raw | ConvertFrom-Json
    if ($arm.trial -ne $i -or $arm.engine -ne $plan.order[$i-1] -or $arm.nonce -cne $plan.nonce) {
        throw 'Deneme kimliği uyuşmadı.'
    }
    "Deneme $i — $($arm.engine): purge ve ilk sonuç izi"
    Invoke-ColdTrial $dir $arm
}
