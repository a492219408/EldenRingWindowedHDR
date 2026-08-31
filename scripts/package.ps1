[CmdletBinding()]
param(
    [switch] $SkipBuild
)

$ErrorActionPreference = 'Stop'
$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Version = (Select-String -Path (Join-Path $RepositoryRoot 'Cargo.toml') -Pattern '^version\s*=\s*"([^"]+)"$').Matches.Groups[1].Value
if ([string]::IsNullOrWhiteSpace($Version)) {
    throw 'Cannot read package version from Cargo.toml'
}

if (-not $SkipBuild) {
    & (Join-Path $PSScriptRoot 'build.ps1')
    if ($LASTEXITCODE -ne 0) {
        throw "build.ps1 failed with exit code $LASTEXITCODE"
    }
}

$DistributionRoot = [System.IO.Path]::GetFullPath((Join-Path $RepositoryRoot 'dist'))
$PackageRoot = [System.IO.Path]::GetFullPath((Join-Path $DistributionRoot "EldenRingWindowedHDR-$Version"))
if (-not $PackageRoot.StartsWith($DistributionRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to package outside $DistributionRoot"
}

if (Test-Path -LiteralPath $PackageRoot) {
    $ExistingTestResults = Join-Path $PackageRoot 'test-results'
    if (Test-Path -LiteralPath $ExistingTestResults) {
        throw "Refusing to replace $PackageRoot because it contains test-results. Preserve the evidence or bump the package version first."
    }
    Remove-Item -LiteralPath $PackageRoot -Recurse -Force
}
$NativesDirectory = New-Item -ItemType Directory -Force -Path (Join-Path $PackageRoot 'natives')

$ReleaseDirectory = Join-Path $RepositoryRoot 'target\x86_64-pc-windows-msvc\release'
Copy-Item -LiteralPath (Join-Path $ReleaseDirectory 'EldenRingWindowedHDR.dll') -Destination $NativesDirectory.FullName
Copy-Item -LiteralPath (Join-Path $RepositoryRoot 'EldenRingWindowedHDR.ini') -Destination $NativesDirectory.FullName
Copy-Item -LiteralPath (Join-Path $RepositoryRoot 'packaging\EldenRingWindowedHDR.me3') -Destination $PackageRoot
Copy-Item -LiteralPath (Join-Path $RepositoryRoot 'packaging\release\README.txt') -Destination $PackageRoot
Copy-Item -LiteralPath (Join-Path $RepositoryRoot 'packaging\release\README.zh-CN.txt') -Destination $PackageRoot
Copy-Item -LiteralPath (Join-Path $RepositoryRoot 'LICENSE') -Destination (Join-Path $PackageRoot 'LICENSE.txt')
Copy-Item -LiteralPath (Join-Path $RepositoryRoot 'packaging\release\THIRD_PARTY_NOTICES.txt') -Destination $PackageRoot

$ArchivePath = "$PackageRoot.zip"
if (Test-Path -LiteralPath $ArchivePath) {
    Remove-Item -LiteralPath $ArchivePath -Force
}
Compress-Archive -LiteralPath $PackageRoot -DestinationPath $ArchivePath -CompressionLevel Optimal
$ArchiveHashPath = "$ArchivePath.sha256"
if (Test-Path -LiteralPath $ArchiveHashPath) {
    Remove-Item -LiteralPath $ArchiveHashPath -Force
}
$ArchiveHash = (Get-FileHash -LiteralPath $ArchivePath -Algorithm SHA256).Hash.ToLowerInvariant()
"$ArchiveHash  $([System.IO.Path]::GetFileName($ArchivePath))" | Set-Content -LiteralPath $ArchiveHashPath -Encoding ascii

Write-Host "Package directory: $PackageRoot"
Write-Host "Package archive:   $ArchivePath"
Write-Host "SHA-256 file:      $ArchiveHashPath"
