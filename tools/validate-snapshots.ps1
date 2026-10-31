param([string]$Attempt='v2')
$ErrorActionPreference='Stop'
$repoRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Push-Location -LiteralPath $repoRoot
try {
    & cargo fmt
    if($LASTEXITCODE -ne 0){throw 'Formatting failed'}
    & cargo fmt --manifest-path tools/native_sqlite/Cargo.toml
    if($LASTEXITCODE -ne 0){throw 'Reference formatting failed'}
    $checks=@(
        @{name='fmt';arguments=@('fmt','--check')},
        @{name='clippy';arguments=@('clippy','--all-targets','--','-D','warnings')},
        @{name='test';arguments=@('test','--all-targets')},
        @{name='no-default';arguments=@('check','--no-default-features','--lib')},
        @{name='all-features';arguments=@('check','--all-features','--lib')},
        @{name='sqlite-fmt';arguments=@('fmt','--manifest-path','tools/native_sqlite/Cargo.toml','--check')},
        @{name='sqlite-clippy';arguments=@('clippy','--manifest-path','tools/native_sqlite/Cargo.toml','--all-targets','--','-D','warnings')},
        @{name='sqlite-test';arguments=@('test','--manifest-path','tools/native_sqlite/Cargo.toml','--bin','content_churn')}
    )
    $results=@()
    $prefix="docs/benchmarks/snapshot-10k-20261026-validation-$Attempt"
    foreach($check in $checks){
        $started=Get-Date
        $log="$prefix-$($check.name).log"
        if(Test-Path -LiteralPath $log){throw "Refusing to overwrite validation evidence: $log"}
        $arguments=$check.arguments
        & cargo @arguments *> $log
        $result=@{name=$check.name;command="cargo $($arguments -join ' ')";exit_code=$LASTEXITCODE;elapsed_s=((Get-Date)-$started).TotalSeconds;log=$log}
        $results+=$result
        $results | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath "$prefix.json" -Encoding utf8NoBOM
        Write-Output "$($check.name): exit $($result.exit_code)"
        if($result.exit_code -ne 0){throw "Validation failed: $($check.name)"}
    }
} finally {Pop-Location}
