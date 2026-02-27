# V-OS Development Script for Windows
# PowerShell build automation

param(
    [Parameter(Position=0)]
    [ValidateSet("all", "kernel", "packages", "qemu", "clean", "distclean", "test", "help", "setup")]
    [string]$Command = "all",
    
    [Alias("a")]
    [ValidateSet("x86_64", "aarch64", "riscv64gc")]
    [string]$Arch = "x86_64",
    
    [Alias("c")]
    [string]$Config = "v-os-desktop",
    
    [Alias("j")]
    [int]$Jobs = [Environment]::ProcessorCount,
    
    [Alias("v")]
    [switch]$VerboseOutput,
    
    [Alias("h")]
    [switch]$ShowHelp
)

# =============================================================================
# Configuration
# =============================================================================

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RootDir = Split-Path -Parent $ScriptDir

# =============================================================================
# Helper Functions
# =============================================================================

function Write-Banner {
    $banner = @"
╔══════════════════════════════════════════════════════════════════╗
║                                                                  ║
║   ███╗   ███╗ █████╗  ██████╗██████╗  ██████╗ ██╗  ██╗ █████╗   ║
║   ████╗ ████║██╔══██╗██╔════╝██╔══██╗██╔═══██╗██║  ██║██╔══██╗  ║
║   ██╔████╔██║███████║██║     ██████╔╝██║   ██║███████║███████║  ║
║   ██║╚██╔╝██║██╔══██║██║     ██╔══██╗██║   ██║██╔══██║██╔══██║  ║
║   ██║ ╚═╝ ██║██║  ██║╚██████╗██║  ██║╚██████╔╝██║  ██║██║  ██║  ║
║   ╚═╝     ╚═╝╚═╝  ╚═╝ ╚═════╝╚═╝  ╚═╝ ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝  ║
║                                                                  ║
║              V-OS Build System v0.1.0                    ║
║              Windows Development Environment                     ║
║                                                                  ║
╚══════════════════════════════════════════════════════════════════╝
"@
    Write-Host $banner -ForegroundColor Cyan
}

function Write-Info {
    param([string]$Message)
    Write-Host "[INFO] " -ForegroundColor Blue -NoNewline
    Write-Host $Message
}

function Write-Success {
    param([string]$Message)
    Write-Host "[SUCCESS] " -ForegroundColor Green -NoNewline
    Write-Host $Message
}

function Write-Warn {
    param([string]$Message)
    Write-Host "[WARN] " -ForegroundColor Yellow -NoNewline
    Write-Host $Message
}

function Write-Error {
    param([string]$Message)
    Write-Host "[ERROR] " -ForegroundColor Red -NoNewline
    Write-Host $Message
}

function Write-Step {
    param([string]$Message)
    Write-Host "[STEP] " -ForegroundColor Cyan -NoNewline
    Write-Host $Message
}

function Show-Usage {
    @"
Usage: .\v-os-dev.ps1 [COMMAND] [OPTIONS]

Commands:
  all         Build complete OS image (default)
  kernel      Build kernel only
  packages    Build packages only
  qemu        Build and run in QEMU
  clean       Clean build artifacts
  distclean   Clean everything including downloads
  test        Run test suite
  setup       Set up development environment
  help        Show this help message

Options:
  -Arch, -a       Target architecture (x86_64, aarch64, riscv64gc)
  -Config, -c     Build configuration (v-os-desktop, v-os-server, v-os-minimal)
  -Jobs, -j       Number of parallel jobs
  -VerboseOutput, -v    Verbose output
  -ShowHelp, -h         Show this help message

Examples:
  .\v-os-dev.ps1                           # Build v-os-desktop for x86_64
  .\v-os-dev.ps1 -Arch aarch64 -Config v-os-server all
  .\v-os-dev.ps1 setup                     # Set up WSL2 development environment
  .\v-os-dev.ps1 qemu                      # Build and run in QEMU

Note: Building V-OS requires WSL2 with a Linux distribution.
      Use the 'setup' command to configure your environment.
"@
}

function Test-WSL {
    try {
        $null = wsl --status 2>&1
        return $LASTEXITCODE -eq 0
    }
    catch {
        return $false
    }
}

function Test-Dependencies {
    Write-Step "Checking dependencies..."
    
    $missing = @()
    
    # Check for WSL2
    if (-not (Test-WSL)) {
        $missing += "WSL2"
    }
    
    # Check for QEMU (optional)
    if (-not (Get-Command "qemu-system-x86_64" -ErrorAction SilentlyContinue)) {
        Write-Warn "QEMU not found in PATH. Install for local testing."
    }
    
    if ($missing.Count -gt 0) {
        Write-Error "Missing dependencies: $($missing -join ', ')"
        Write-Info "Run '.\v-os-dev.ps1 setup' to configure your environment."
        exit 1
    }
    
    Write-Success "Dependencies check passed"
}

# =============================================================================
# Commands
# =============================================================================

function Invoke-Setup {
    Write-Step "Setting up V-OS development environment..."
    
    # Check for WSL2
    if (-not (Test-WSL)) {
        Write-Info "Installing WSL2..."
        Write-Info "Please run 'wsl --install' in an elevated PowerShell and restart."
        Write-Info "After restart, run this setup again."
        exit 0
    }
    
    Write-Info "WSL2 is available"
    
    # Install dependencies in WSL
    Write-Step "Installing build dependencies in WSL..."
    
    $wslScript = @'
#!/bin/bash
set -e

echo "Updating package lists..."
sudo apt-get update

echo "Installing build dependencies..."
sudo apt-get install -y \
    build-essential \
    cmake \
    curl \
    file \
    git \
    gperf \
    libfuse-dev \
    libhtml-parser-perl \
    libpng-dev \
    libjpeg-dev \
    meson \
    nasm \
    ninja-build \
    pkg-config \
    qemu-system-x86 \
    qemu-utils \
    syslinux-utils \
    xdg-utils \
    podman

echo "Installing Rust..."
if ! command -v rustup &> /dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

rustup default nightly
rustup component add rust-src

echo "Setup complete!"
'@
    
    # Convert Windows line endings to Unix before passing to WSL
    $wslScript -replace "`r`n", "`n" | wsl bash
    
    Write-Success "Development environment setup complete!"
    Write-Info "You can now build V-OS using: .\v-os-dev.ps1 all"
}

function Invoke-Build {
    param([string]$Target = "all")
    
    Test-Dependencies
    
    Write-Step "Building V-OS ($Arch, $Config)..."
    
    $startTime = Get-Date
    
    # Run build in WSL
    $wslCommand = "cd '$($RootDir -replace '\\', '/' -replace 'C:', '/mnt/c')' && make $Target ARCH=$Arch CONFIG_NAME=$Config PODMAN_BUILD=1 -j$Jobs"
    
    if ($VerboseOutput) {
        Write-Info "Running: $wslCommand"
    }
    
    wsl bash -c $wslCommand
    
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Build failed with exit code $LASTEXITCODE"
        exit $LASTEXITCODE
    }
    
    $duration = (Get-Date) - $startTime
    Write-Success "Build completed in $($duration.TotalSeconds.ToString('F1'))s"
}

function Invoke-QEMU {
    Write-Step "Running V-OS in QEMU..."
    
    $imagePath = Join-Path $RootDir "build\$Arch\$Config\harddrive.img"
    
    if (-not (Test-Path $imagePath)) {
        Write-Info "Image not found, building first..."
        Invoke-Build "all"
    }
    
    # Try Windows QEMU first
    if (Get-Command "qemu-system-x86_64" -ErrorAction SilentlyContinue) {
        Write-Info "Starting QEMU (Windows)..."
        & qemu-system-x86_64 `
            -machine q35 `
            -m 2048 `
            -smp 2 `
            -drive "file=$imagePath,format=raw" `
            -serial mon:stdio
    }
    else {
        # Fall back to WSL QEMU
        Write-Info "Starting QEMU (WSL)..."
        $wslImagePath = $imagePath -replace '\\', '/' -replace 'C:', '/mnt/c'
        wsl qemu-system-x86_64 -machine q35 -m 2048 -smp 2 -drive "file=$wslImagePath,format=raw" -nographic
    }
}

function Invoke-Clean {
    Write-Step "Cleaning build artifacts..."
    
    $wslCommand = "cd '$($RootDir -replace '\\', '/' -replace 'C:', '/mnt/c')' && make clean ARCH=$Arch CONFIG_NAME=$Config PODMAN_BUILD=0 SKIP_CHECK_TOOLS=1 || true"
    wsl bash -c $wslCommand
    
    Write-Success "Clean completed"
}

function Invoke-DistClean {
    Write-Step "Cleaning everything..."
    
    $wslCommand = "cd '$($RootDir -replace '\\', '/' -replace 'C:', '/mnt/c')' && make distclean ARCH=$Arch CONFIG_NAME=$Config PODMAN_BUILD=0 SKIP_CHECK_TOOLS=1 || true"
    wsl bash -c $wslCommand
    
    Write-Success "Distclean completed"
}

function Invoke-Test {
    Write-Step "Running test suite..."
    
    Test-Dependencies
    
    # Run Cargo tests
    $wslCommand = "cd '$($RootDir -replace '\\', '/' -replace 'C:', '/mnt/c')' && cargo test --all"
    wsl bash -c $wslCommand
    
    Write-Success "Test suite completed"
}

# =============================================================================
# Main
# =============================================================================

function Main {
    Write-Banner
    
    if ($ShowHelp) {
        Show-Usage
        exit 0
    }
    
    Write-Info "Architecture: $Arch"
    Write-Info "Configuration: $Config"
    Write-Info "Jobs: $Jobs"
    Write-Host ""
    
    switch ($Command) {
        "all"       { Invoke-Build "all" }
        "kernel"    { Invoke-Build "prefix" }
        "packages"  { Invoke-Build "repo" }
        "qemu"      { Invoke-QEMU }
        "clean"     { Invoke-Clean }
        "distclean" { Invoke-DistClean }
        "test"      { Invoke-Test }
        "setup"     { Invoke-Setup }
        "help"      { Show-Usage }
        default     { Invoke-Build "all" }
    }
}

Main
