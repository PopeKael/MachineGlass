param([string]$OutputDirectory = 'dist\MachineGlass')
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $binary = Join-Path $repoRoot 'target\release\MachineGlass.exe'
    if (-not (Test-Path -LiteralPath $binary)) { throw 'Run cargo build --release --locked first.' }
    New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
    Copy-Item -LiteralPath $binary -Destination (Join-Path $OutputDirectory 'MachineGlass.exe')
    Copy-Item -LiteralPath 'README.md' -Destination $OutputDirectory
    Copy-Item -LiteralPath 'docs' -Destination $OutputDirectory -Recurse -Force
    $metadataText = & cargo metadata --locked --offline --format-version 1 --filter-platform x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'Could not inspect locked dependencies.' }
    $metadata = $metadataText | ConvertFrom-Json
    $nodes = @{}
    foreach ($node in $metadata.resolve.nodes) { $nodes[$node.id] = $node }
    $seen = [System.Collections.Generic.HashSet[string]]::new()
    $queue = [System.Collections.Generic.Queue[string]]::new()
    $queue.Enqueue($metadata.resolve.root)
    while ($queue.Count -gt 0) {
        $packageId = $queue.Dequeue()
        if ($seen.Add($packageId)) {
            foreach ($dep in $nodes[$packageId].dependencies) { $queue.Enqueue($dep) }
        }
    }
    $noticeRoot = Join-Path $OutputDirectory 'third-party'
    New-Item -ItemType Directory -Force -Path $noticeRoot | Out-Null
    $lines = [System.Collections.Generic.List[string]]::new()
    $lines.Add('# Third-party source licence inventory')
    $lines.Add('Generated from Cargo.lock for the Windows dependency graph. Includes build-time dependencies.')
    foreach ($package in ($metadata.packages | Sort-Object name, version)) {
        if (-not $seen.Contains($package.id) -or $package.name -eq 'machineglass') { continue }
        $lines.Add("`n## $($package.name) $($package.version)`n$($package.license)`n$($package.repository)")
        $sourceDir = Split-Path -Parent $package.manifest_path
        $texts = Get-ChildItem -LiteralPath $sourceDir -File -Recurse | Where-Object {
            $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE|OFL|UFL|FONTLOG)([-._].*)?$'
        }
        $destination = Join-Path $noticeRoot "$($package.name)-$($package.version)"
        if ($texts.Count -gt 0) {
            foreach ($file in $texts) {
                $relativePath = $file.FullName.Substring($sourceDir.Length + 1)
                $targetFile = Join-Path $destination $relativePath
                New-Item -ItemType Directory -Force -Path (Split-Path -Parent $targetFile) | Out-Null
                Copy-Item -LiteralPath $file.FullName -Destination $targetFile
            }
        } else { $lines.Add('No standalone licence file found in packaged crate source; review before public distribution.') }
    }
    $lines | Set-Content -LiteralPath (Join-Path $OutputDirectory 'THIRD-PARTY-NOTICES.md') -Encoding UTF8
    Get-FileHash -LiteralPath (Join-Path $OutputDirectory 'MachineGlass.exe') -Algorithm SHA256
} finally { Pop-Location }
