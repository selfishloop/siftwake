[CmdletBinding()]
param([Parameter(Mandatory)][string]$Trace,[Parameter(Mandatory)][string]$OutputDir,
      [Parameter(Mandatory)][string]$Consumer,[string[]]$ConsumerArgs=@())
$ErrorActionPreference='Stop'
$tracePath=(Resolve-Path -LiteralPath $Trace).Path
$outPath=(Resolve-Path -LiteralPath $OutputDir).Path
$python=Join-Path $env:USERPROFILE '.cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe'
$id='fdb-etl-offline-'+[guid]::NewGuid().ToString('N')
$pipePath='\\.\pipe\'+$id
$peers=[Collections.Generic.List[object]]::new()
$decode=$null; $consume=$null; $copy=$null; $success=$false
function Start-OfflineProcess([string]$exe,[string[]]$arguments,[bool]$input) {
    $psi=[Diagnostics.ProcessStartInfo]::new($exe)
    $psi.UseShellExecute=$false
    $psi.CreateNoWindow=$true
    $psi.RedirectStandardInput=$input
    $psi.RedirectStandardOutput=$true
    $psi.RedirectStandardError=$true
    $psi.Environment['PYTHONUTF8']='1'
    foreach ($arg in $arguments) { $psi.ArgumentList.Add($arg) }
    return [Diagnostics.Process]::Start($psi)
}
try {
    if (Test-Path -LiteralPath (Join-Path $outPath 'tracerpt-summary.txt')) { throw 'Output already claimed' }
    for ($i=0;$i -lt 6;$i++) {
        $s=[IO.Pipes.NamedPipeServerStream]::new($id,[IO.Pipes.PipeDirection]::InOut,6,[IO.Pipes.PipeTransmissionMode]::Byte,[IO.Pipes.PipeOptions]::Asynchronous)
        $peers.Add(@{stream=$s; connecting=$s.WaitForConnectionAsync(); reading=$null; byte=[byte[]]::new(1); done=$false})
    }
    $summary=Join-Path $outPath 'tracerpt-summary.txt'
    $decode=Start-OfflineProcess (Join-Path $env:windir 'System32/tracerpt.exe') @($tracePath,'-of','CSV','-o',$pipePath,'-summary',$summary,'-y') $false
    $decoderOut=$decode.StandardOutput.ReadToEndAsync(); $decoderErr=$decode.StandardError.ReadToEndAsync()
    $timer=[Diagnostics.Stopwatch]::StartNew()
    $active=$null
    while ($null -eq $active -or -not $copy.IsCompleted -or -not $decode.HasExited) {
        if ($timer.Elapsed.TotalSeconds -gt 55 -or [DateTimeOffset]::UtcNow.AddSeconds(12) -ge [DateTimeOffset]::Parse($env:FDB_OPEN_COLD_DEADLINE)) { throw 'Offline pipe time cap' }
        foreach ($peer in $peers) {
            if ($peer.done) { continue }
            if ($null -eq $peer.reading -and $peer.connecting.IsCompletedSuccessfully) {
                $peer.reading=$peer.stream.ReadAsync($peer.byte,0,1)
            }
            if ($peer.reading -and $peer.reading.IsCompleted) {
                $peer.done=$true
                if ($peer.reading.IsCompletedSuccessfully -and $peer.reading.Result -eq 1) {
                    if ($null -ne $active) { throw 'More than one named pipe received CSV data' }
                    $active=$peer
                    $consume=Start-OfflineProcess $python (@($Consumer)+$ConsumerArgs) $true
                    $consumerOut=$consume.StandardOutput.ReadToEndAsync()
                    $consumerErr=$consume.StandardError.ReadToEndAsync()
                    $dst=$consume.StandardInput.BaseStream
                    $dst.WriteByte($active.byte[0])
                    $copy=$active.stream.CopyToAsync($dst,65536)
                } elseif (-not $peer.reading.IsCompletedSuccessfully) {
                    throw "Pipe read failed: $($peer.reading.Exception)"
                }
            }
        }
        if ($null -eq $active -and $decode.HasExited) { throw "No CSV bytes. Tracerpt exit $($decode.ExitCode)" }
        if ($consume -and $consume.HasExited -and -not $copy.IsCompleted) {throw "Consumer closed before EOF: $($consumerErr.Result)"}
        Start-Sleep -Milliseconds 35
    }
    $copy.GetAwaiter().GetResult()
    $dst.Close()
    if (-not $consume.WaitForExit(5000)) {throw 'Consumer EOF exit timeout'}
    if ($decode.ExitCode -ne 0 -or $consume.ExitCode -ne 0) {throw "Decoder/consumer exit: $($decode.ExitCode)/$($consume.ExitCode) $($consumerErr.Result)"}
    $success=$true
} finally {
    foreach ($process in @($decode,$consume)) {
        if ($process -and -not $process.HasExited) {$process.Kill(); $process.WaitForExit()}
    }
    [ordered]@{success=$success; decoder_exit=if($decode){$decode.ExitCode}; consumer_exit=if($consume){$consume.ExitCode};
        connected_peers=@($peers | Where-Object {$_.connecting.IsCompletedSuccessfully}).Count;
        decoder_stdout=if($decoderOut){$decoderOut.Result}; decoder_stderr=if($decoderErr){$decoderErr.Result};
        consumer_stdout=if($consumerOut){$consumerOut.Result}; consumer_stderr=if($consumerErr){$consumerErr.Result}} |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $outPath 'pipe-run.json') -Encoding utf8
    foreach ($peer in $peers) {$peer.stream.Dispose()}
    foreach ($process in @($decode,$consume)) {if($process){$process.Dispose()}}
}


