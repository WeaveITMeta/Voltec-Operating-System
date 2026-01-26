# =============================================================================
# EustressEngine Build Script for Redox Quantum OS
# =============================================================================
# This script sets up and builds EustressEngine with the Redox-compatible
# workspace configuration (sparse checkout compatible).
#
# Usage:
#   .\build-eustress.ps1 [command]
#
# Commands:
#   setup    - Copy Redox workspace config (first time setup)
#   build    - Build the engine
#   run      - Run the engine
#   check    - Check compilation without building
#   restore  - Restore original Cargo.toml
# =============================================================================

param(
    [Parameter(Position=0)]
    [string]$Command = "build"
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$EustressDir = Join-Path $ScriptDir "eustress-engine\eustress"
$OriginalCargo = Join-Path $EustressDir "Cargo.toml"
$RedoxCargo = Join-Path $EustressDir "Cargo.redox.toml"
$BackupCargo = Join-Path $EustressDir "Cargo.original.toml"

function Write-Header {
    param([string]$Message)
    Write-Host ""
    Write-Host "=== $Message ===" -ForegroundColor Cyan
    Write-Host ""
}

function Setup-RedoxWorkspace {
    Write-Header "Setting up Redox workspace configuration"
    
    # Backup original if not already backed up
    if (-not (Test-Path $BackupCargo)) {
        Write-Host "Backing up original Cargo.toml..." -ForegroundColor Yellow
        Copy-Item $OriginalCargo $BackupCargo
    }
    
    # Copy Redox config
    Write-Host "Copying Redox workspace configuration..." -ForegroundColor Yellow
    Copy-Item $RedoxCargo $OriginalCargo -Force
    
    Write-Host "Setup complete!" -ForegroundColor Green
    Write-Host "Original Cargo.toml backed up to Cargo.original.toml"
}

function Restore-OriginalWorkspace {
    Write-Header "Restoring original workspace configuration"
    
    if (Test-Path $BackupCargo) {
        Copy-Item $BackupCargo $OriginalCargo -Force
        Write-Host "Original Cargo.toml restored!" -ForegroundColor Green
    } else {
        Write-Host "No backup found. Cannot restore." -ForegroundColor Red
        exit 1
    }
}

function Build-Engine {
    param([string]$Profile = "dev")
    
    Write-Header "Building EustressEngine"
    
    # Ensure Redox config is in place
    if (-not (Test-Path $BackupCargo)) {
        Setup-RedoxWorkspace
    }
    
    Push-Location $EustressDir
    try {
        if ($Profile -eq "release") {
            cargo build -p eustress-engine --release
        } else {
            cargo build -p eustress-engine
        }
    } finally {
        Pop-Location
    }
}

function Check-Engine {
    Write-Header "Checking EustressEngine compilation"
    
    # Ensure Redox config is in place
    if (-not (Test-Path $BackupCargo)) {
        Setup-RedoxWorkspace
    }
    
    Push-Location $EustressDir
    try {
        cargo check -p eustress-engine
    } finally {
        Pop-Location
    }
}

function Run-Engine {
    Write-Header "Running EustressEngine"
    
    # Ensure Redox config is in place
    if (-not (Test-Path $BackupCargo)) {
        Setup-RedoxWorkspace
    }
    
    Push-Location $EustressDir
    try {
        cargo run -p eustress-engine
    } finally {
        Pop-Location
    }
}

# Main execution
switch ($Command.ToLower()) {
    "setup" {
        Setup-RedoxWorkspace
    }
    "build" {
        Build-Engine
    }
    "release" {
        Build-Engine -Profile "release"
    }
    "check" {
        Check-Engine
    }
    "run" {
        Run-Engine
    }
    "restore" {
        Restore-OriginalWorkspace
    }
    default {
        Write-Host "EustressEngine Build Script for Redox Quantum OS" -ForegroundColor Cyan
        Write-Host ""
        Write-Host "Usage: .\build-eustress.ps1 [command]" -ForegroundColor White
        Write-Host ""
        Write-Host "Commands:" -ForegroundColor Yellow
        Write-Host "  setup    - Copy Redox workspace config (first time setup)"
        Write-Host "  build    - Build the engine (default)"
        Write-Host "  release  - Build release version"
        Write-Host "  check    - Check compilation without building"
        Write-Host "  run      - Run the engine"
        Write-Host "  restore  - Restore original Cargo.toml"
    }
}
