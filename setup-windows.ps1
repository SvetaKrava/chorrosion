#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Automated setup script for Chorrosion Windows development environment.
    Installs vcpkg, chromaprint, and configures the build environment.

.DESCRIPTION
    This script:
    1. Clones/updates vcpkg to C:\util\vcpkg
    2. Installs chromaprint:x64-windows
    3. Configures Visual Studio Build Tools integration
    4. Adds vcpkg bin directory to PATH for current session
    5. Verifies the installation

.EXAMPLE
    .\setup-windows.ps1

.EXAMPLE
    .\setup-windows.ps1 -VcpkgRoot "D:\my-vcpkg"

.PARAMETER VcpkgRoot
    Location to install vcpkg. Default: C:\util\vcpkg

.PARAMETER SkipVerification
    Skip verification step at the end.
#>

param(
    [string]$VcpkgRoot = "C:\util\vcpkg",
    [switch]$SkipVerification
)

$ErrorActionPreference = "Stop"

Write-Host "Chorrosion Windows Development Setup" -ForegroundColor Green
Write-Host "=====================================" -ForegroundColor Green
Write-Host ""

# Check if running as admin
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole] "Administrator")
if (-not $isAdmin) {
    Write-Warning "This script should ideally run as Administrator for PATH modifications."
    Write-Host "Continuing with current user privileges..." -ForegroundColor Yellow
}

# Step 1: Setup vcpkg
Write-Host "Step 1: Setting up vcpkg..." -ForegroundColor Cyan

if (Test-Path $VcpkgRoot) {
    Write-Host "vcpkg directory already exists at $VcpkgRoot" -ForegroundColor Yellow
    Write-Host "Updating vcpkg..." -ForegroundColor Gray
    Push-Location $VcpkgRoot
    git pull origin master 2>&1 | Out-Null
    Pop-Location
} else {
    Write-Host "Cloning vcpkg to $VcpkgRoot..." -ForegroundColor Gray
    New-Item -ItemType Directory -Path (Split-Path $VcpkgRoot -Parent) -Force | Out-Null
    git clone https://github.com/microsoft/vcpkg $VcpkgRoot
}

# Bootstrap vcpkg
Write-Host "Bootstrapping vcpkg..." -ForegroundColor Gray
if (-not (Test-Path "$VcpkgRoot\vcpkg.exe")) {
    & "$VcpkgRoot\bootstrap-vcpkg.bat" 2>&1 | Out-Null
}

# Step 2: Install chromaprint
Write-Host "Step 2: Installing chromaprint..." -ForegroundColor Cyan

Write-Host "Installing chromaprint:x64-windows (this may take a few minutes)..." -ForegroundColor Gray
& "$VcpkgRoot\vcpkg" install chromaprint:x64-windows
if ($LASTEXITCODE -ne 0) {
    Write-Error "Failed to install chromaprint. Check vcpkg output above."
}

# Step 3: Visual Studio Build Tools Integration
Write-Host "Step 3: Setting up Visual Studio Build Tools integration..." -ForegroundColor Cyan

Write-Host "Running vcpkg integrate install..." -ForegroundColor Gray
& "$VcpkgRoot\vcpkg" integrate install
if ($LASTEXITCODE -ne 0) {
    Write-Warning "vcpkg integrate install had issues. Visual Studio integration may not work."
}

# Step 4: Add to PATH for current session
Write-Host "Step 4: Configuring PATH..." -ForegroundColor Cyan

$chromaprintBin = "$VcpkgRoot\installed\x64-windows\bin"
if ($chromaprintBin -notin $env:PATH.Split(";")) {
    $env:PATH = "$chromaprintBin;$env:PATH"
    Write-Host "Added $chromaprintBin to PATH for current session" -ForegroundColor Gray
} else {
    Write-Host "PATH already contains chromaprint bin directory" -ForegroundColor Gray
}

# Offer to add to system PATH permanently
Write-Host ""
Write-Host "To make this permanent, add the following to your system PATH:" -ForegroundColor Yellow
Write-Host "  $chromaprintBin" -ForegroundColor Cyan
Write-Host ""
Write-Host "You can do this via:" -ForegroundColor Gray
Write-Host "  - Windows Settings → Environment Variables" -ForegroundColor Gray
Write-Host "  - setx PATH ""$chromaprintBin;%PATH%""" -ForegroundColor Gray

# Step 5: Verification
if (-not $SkipVerification) {
    Write-Host "Step 5: Verifying installation..." -ForegroundColor Cyan

    $checks = @(
        @{ Name = "Rust"; Command = "rustc --version" },
        @{ Name = "Cargo"; Command = "cargo --version" },
        @{ Name = "chromaprint.lib"; Path = "$VcpkgRoot\installed\x64-windows\lib\chromaprint.lib" },
        @{ Name = "chromaprint DLLs"; Path = "$VcpkgRoot\installed\x64-windows\bin\chromaprint*.dll" },
        @{ Name = "ffmpeg DLLs"; Path = "$VcpkgRoot\installed\x64-windows\bin\avcodec*.dll" }
    )

    $allPassed = $true
    foreach ($check in $checks) {
        if ($check.Command) {
            try {
                $result = & $check.Command 2>&1 | Select-Object -First 1
                if ($LASTEXITCODE -eq 0) {
                    Write-Host "✓ $($check.Name): $result" -ForegroundColor Green
                } else {
                    Write-Host "✗ $($check.Name): Not found or error" -ForegroundColor Red
                    $allPassed = $false
                }
            } catch {
                Write-Host "✗ $($check.Name): Error - $_" -ForegroundColor Red
                $allPassed = $false
            }
        } elseif ($check.Path) {
            if (Test-Path $check.Path) {
                Write-Host "✓ $($check.Name): Found" -ForegroundColor Green
            } else {
                Write-Host "✗ $($check.Name): Not found at $($check.Path)" -ForegroundColor Red
                $allPassed = $false
            }
        }
    }

    Write-Host ""
    if ($allPassed) {
        Write-Host "✓ All checks passed! Ready to build." -ForegroundColor Green
    } else {
        Write-Host "⚠ Some checks failed. Review the output above." -ForegroundColor Yellow
    }
}

Write-Host ""
Write-Host "Setup complete!" -ForegroundColor Green
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Cyan
Write-Host "  1. Build: cargo build" -ForegroundColor Gray
Write-Host "  2. Test:  cargo test --workspace" -ForegroundColor Gray
Write-Host "  3. Run:   cargo run -p chorrosion-cli" -ForegroundColor Gray
Write-Host ""
Write-Host "For more information, see:" -ForegroundColor Cyan
Write-Host "  - EXTERNAL_DEPENDENCIES.md" -ForegroundColor Gray
Write-Host "  - WINDOWS_CHROMAPRINT_SETUP.md" -ForegroundColor Gray
