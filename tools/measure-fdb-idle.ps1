param([Parameter(Mandatory=$true)][string]$DatabasePath, [string]$OutputPath='output/fdb-idle.json', [string]$BinaryPath='target/release/siftwake.exe')
$ErrorActionPreference='Stop'
$startInfo=[System.Diagnostics.ProcessStartInfo]::new()
$startInfo.FileName=[System.IO.Path]::GetFullPath($BinaryPath)
$startInfo.ArgumentList.Add([System.IO.Path]::GetFullPath($DatabasePath))
$startInfo.ArgumentList.Add('--stdio')
$startInfo.UseShellExecute=$false
$startInfo.CreateNoWindow=$true
$startInfo.RedirectStandardInput=$true
$startInfo.RedirectStandardOutput=$true
$proc=[System.Diagnostics.Process]::Start($startInfo)
try {
    $started=Get-Date
    $proc.StandardInput.WriteLine('{"op":"stats"}')
    $proc.StandardInput.Flush()
    $response=$proc.StandardOutput.ReadLine() | ConvertFrom-Json
    if (-not $response.ok) { throw 'FDB open failed' }
    $openSeconds=((Get-Date)-$started).TotalSeconds
    Start-Sleep -Milliseconds 1000
    $proc.Refresh()
    $result=[ordered]@{binary=$startInfo.FileName;binary_sha256=(Get-FileHash -LiteralPath $startInfo.FileName -Algorithm SHA256).Hash;documents=$response.result.documents;sequence=$response.result.sequence;segments=$response.result.segments;open_and_stats_s=$openSeconds;idle_working_set_bytes=$proc.WorkingSet64;idle_private_bytes=$proc.PrivateMemorySize64;peak_open_working_set_bytes=$proc.PeakWorkingSet64}
    $result | ConvertTo-Json | Set-Content -Encoding utf8 -LiteralPath $OutputPath
    $result | ConvertTo-Json
} finally {
    $proc.StandardInput.Close()
    $proc.WaitForExit()
    $proc.Dispose()
}
