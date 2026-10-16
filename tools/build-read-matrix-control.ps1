# Source-only snapshot for the previous full-body scan. No DB/data copied.
$ErrorActionPreference='Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$root=Join-Path (Get-Location) 'output/read-matrix-full-scan-source'
if(Test-Path -LiteralPath $root){throw 'Control snapshot already exists; preserve it'}
New-Item -ItemType Directory -Path $root | Out-Null
Copy-Item -LiteralPath 'Cargo.toml','Cargo.lock' -Destination $root
Copy-Item -LiteralPath 'src' -Destination $root -Recurse
New-Item -ItemType Directory -Path "$root/examples","$root/tools/bench_support" | Out-Null
Copy-Item -LiteralPath 'examples/measure_content_reads.rs' -Destination "$root/examples"
Copy-Item -LiteralPath 'tools/bench_support/memory_corpus.rs','tools/bench_support/content_reads.rs','tools/bench_support/read_matrix.rs' -Destination "$root/tools/bench_support"
$segment=Join-Path $root 'src/segment.rs'
$code=[IO.File]::ReadAllText($segment)
$needle='if text.len() > 1024 {'
if(([regex]::Matches($code,[regex]::Escape($needle))).Count -ne 1){throw 'Unexpected chunk branch'}
[IO.File]::WriteAllText($segment,$code.Replace($needle,'if false && text.len() > 1024 {'),[Text.UTF8Encoding]::new($false))
& cargo build --release --manifest-path "$root/Cargo.toml" --target-dir tools/native_sqlite/target --example measure_content_reads --features 'experimental-merge-postings experimental-delta-bitmaps'
if($LASTEXITCODE -ne 0){throw 'Control build failed'}
Copy-Item -LiteralPath 'tools/native_sqlite/target/release/examples/measure_content_reads.exe' -Destination 'output/read-matrix-full-scan-control.exe'
