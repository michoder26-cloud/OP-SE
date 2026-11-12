$ErrorActionPreference = 'Stop'

$manifest = Join-Path $PSScriptRoot 'Cargo.toml'
cargo build --release --manifest-path $manifest
if ($LASTEXITCODE -ne 0) {
    throw 'Release build failed.'
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$dist = Join-Path $PSScriptRoot 'dist'
New-Item -ItemType Directory -Path $dist -Force | Out-Null
$archive = Join-Path $dist 'op-se-control-plane-win-x64.zip'
if (Test-Path -LiteralPath $archive) {
    Remove-Item -LiteralPath $archive -Force
}

Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [System.IO.Compression.ZipFile]::Open($archive, [System.IO.Compression.ZipArchiveMode]::Create)
try {
    $files = @{
        'op-se-control-plane.exe' = (Join-Path $PSScriptRoot 'target\release\op-se-control-plane.exe')
        'README.md' = (Join-Path $PSScriptRoot 'README.md')
        '.env.example' = (Join-Path $PSScriptRoot '.env.example')
        'LICENSE.txt' = (Join-Path $repoRoot 'LICENSE.txt')
    }
    foreach ($name in $files.Keys) {
        [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
            $zip, $files[$name], $name, [System.IO.Compression.CompressionLevel]::Optimal
        ) | Out-Null
    }
}
finally {
    $zip.Dispose()
}
Write-Output $archive
