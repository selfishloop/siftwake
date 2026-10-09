param(
    [string]$Prefix='write-components-20261008',
    [int]$Count=100000,
    [int]$TextBytes=256
)
$ErrorActionPreference='Stop'
# A balanced rotation: each component variant occupies every position once.
$labels=@('full','no-policy','no-spool','no-slots')
foreach ($label in $labels) {
    if (-not (Test-Path -LiteralPath "output/fdb-ablation-20261008-$label.exe")) { throw "Missing binary: $label" }
}
foreach ($run in 0..3) {
    foreach ($position in 0..3) {
        $label=$labels[($run+$position)%4]
        Write-Output "Run $run position $position variant $label"
        & "$PSScriptRoot/measure-native.ps1" -Executable "output/fdb-ablation-20261008-$label.exe" -DatabasePath "data/$Prefix-$label-$run" -Count $Count -TextBytes $TextBytes -OutputPrefix "docs/benchmarks/$Prefix-$label-$run"
    }
}
