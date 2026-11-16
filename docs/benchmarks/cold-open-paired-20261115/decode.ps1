param([Parameter(Mandatory)][string]$Trial, [switch]$AuditOnly)
$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$decoded = Join-Path $Trial 'decoded'
$options = @{
    Trace = (Join-Path $Trial 'preflight.etl')
    OutputDir = $decoded
    Consumer = (Join-Path $root 'filter-paired.py')
    ConsumerArgs = @($Trial, (Join-Path $root 'cold-inventory.json'), $decoded)
}
if (-not $AuditOnly) { & (Join-Path $root 'stream-tracerpt.ps1') @options }
$proof = Get-Content -LiteralPath (Join-Path $decoded 'identity.json') -Raw | ConvertFrom-Json
if ($proof.technical_pass -or $proof.errors.Count -ne 1 -or
    $proof.errors[0] -cne 'merged loss or full event count' -or $proof.merged_lost -ne 0) { return }
$allowed = @('EventTrace/Extension','EventTrace/EndExtension','EventTrace/RDComplete','FileIo/Create','FileIo/Read','FileIo/OperationEnd')
$other = @($proof.count_differences | Where-Object {
    "$($_[0])/$($_[1])" -notin $allowed -or [long]$_[4] -le 0
})
if ($proof.count_differences.Count -eq 0 -or $other.Count -ne 0 -or
    ($proof.count_differences | ForEach-Object { $_[4] } | Measure-Object -Sum).Sum -ne
        $proof.summary_events - $proof.decoded_events) { return }
$start = [DateTime]::FromFileTimeUtc([long]$proof.purge_end_filetime)
$end = [DateTime]::FromFileTimeUtc([long]$proof.cold_window_filetime[1])
$rows = [Collections.Generic.List[string]]::new()
$q = "*[System[Provider[@Guid='{90cbdc39-4a3e-11d1-84f4-0000f80464e3}'] and (Opcode=64 or Opcode=67 or Opcode=76)]]"
$watch = [Diagnostics.Stopwatch]::StartNew()
$limit = if ($AuditOnly) { 180 } else { 50 }
Get-WinEvent -Path $options.Trace -FilterXPath $q -Oldest -ErrorAction Stop | ForEach-Object {
    try {
        if ($watch.Elapsed.TotalSeconds -ge $limit -or $rows.Count -ge 850000) { throw 'Independent FileIO audit bound.' }
        $utc = $_.TimeCreated.ToUniversalTime()
        if ($utc -ge $start -and $utc -le $end) {
            $rows.Add(('{0:D3}{4}{1:D19}{4}{2:D10}{4}{3:D10}{5}' -f [int]$_.Opcode,
                $utc.ToFileTimeUtc(),[long]$_.ProcessId,[long]$_.ThreadId,[char]9,[char]10))
        }
    } finally { $_.Dispose() }
}
$rows.Sort([StringComparer]::Ordinal)
$hash = [Security.Cryptography.IncrementalHash]::CreateHash([Security.Cryptography.HashAlgorithmName]::SHA256)
try {
    foreach ($line in $rows) { $hash.AppendData([Text.Encoding]::ASCII.GetBytes($line)) }
    $digest = [Convert]::ToHexString($hash.GetHashAndReset())
} finally { $hash.Dispose() }
@{rows=$rows.Count;sha256=$digest;exact=($rows.Count -eq $proof.protected_fileio_rows -and
    $digest -eq $proof.protected_fileio_sha)} | ConvertTo-Json |
    Set-Content -LiteralPath (Join-Path $decoded 'window-reconciliation.json') -Encoding utf8
