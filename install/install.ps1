# Installer for mcp-tester and mcp-preview binaries from GitHub releases.
# Downloads the correct Windows binary, verifies its SHA256 checksum,
# and installs it to a local bin directory.
#
# Usage:
#   irm https://raw.githubusercontent.com/paiml/rust-mcp-sdk/main/install/install.ps1 | iex
#   .\install.ps1 -Tool mcp-preview -Version v1.18.0

param(
    [string]$Tool = "mcp-tester",
    [string]$Version = "latest",
    [string]$InstallDir = "$env:USERPROFILE\.local\bin"
)

$ErrorActionPreference = "Stop"

# Validate tool name
if ($Tool -notin @("mcp-tester", "mcp-preview")) {
    Write-Error "Unknown tool '$Tool'. Must be mcp-tester or mcp-preview."
    exit 1
}

# Detect architecture
$arch = if ([System.Environment]::Is64BitOperatingSystem) { "X64" } else { "Unknown" }
# Use RuntimeInformation if available (PowerShell 6+)
try {
    $arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
} catch {
    # Fallback already set above
}

switch ($arch) {
    "X64"   { $target = "x86_64-pc-windows-msvc" }
    default { Write-Error "Unsupported architecture: $arch"; exit 1 }
}

Write-Host "Detected platform: $target"

# Build download URL
$assetName = "$Tool-$target.exe"
$repo = "paiml/rust-mcp-sdk"

if ($Version -eq "latest") {
    $url = "https://github.com/$repo/releases/latest/download/$assetName"
} else {
    $url = "https://github.com/$repo/releases/download/$Version/$assetName"
}
$checksumUrl = "$url.sha256"

# Download to temp directory
$tempDir = New-Item -ItemType Directory -Path (Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName()))
$binaryPath = Join-Path $tempDir $assetName
$checksumPath = Join-Path $tempDir "$assetName.sha256"

try {
    Write-Host "Downloading $assetName..."
    Invoke-WebRequest -Uri $url -OutFile $binaryPath -UseBasicParsing

    Write-Host "Downloading checksum..."
    Invoke-WebRequest -Uri $checksumUrl -OutFile $checksumPath -UseBasicParsing

    # Verify checksum
    Write-Host "Verifying checksum..."
    $expectedHash = (Get-Content $checksumPath).Split(" ")[0]
    $actualHash = (Get-FileHash -Path $binaryPath -Algorithm SHA256).Hash.ToLower()

    if ($actualHash -ne $expectedHash) {
        Write-Error "Checksum mismatch! Expected: $expectedHash, Got: $actualHash"
        exit 1
    }
    Write-Host "Checksum verified."

    # Install
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }
    $destPath = Join-Path $InstallDir "$Tool.exe"
    Move-Item -Path $binaryPath -Destination $destPath -Force

    Write-Host ""
    Write-Host "$Tool installed to $destPath"

    # Check PATH
    $currentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    if ($currentPath -notlike "*$InstallDir*") {
        Write-Host ""
        Write-Host "Warning: $InstallDir is not in your PATH."
        Write-Host "Add it with:"
        Write-Host "  [Environment]::SetEnvironmentVariable('PATH', `"$InstallDir;`$env:PATH`", 'User')"
    }
} finally {
    # Cleanup temp directory
    if (Test-Path $tempDir) {
        Remove-Item -Recurse -Force $tempDir -ErrorAction SilentlyContinue
    }
}
