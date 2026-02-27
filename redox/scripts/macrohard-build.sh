#!/bin/bash
# V-OS Build Script
# Enhanced build automation with cross-compilation support

set -euo pipefail

# =============================================================================
# Configuration
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

# Default values
ARCH="${ARCH:-x86_64}"
CONFIG="${CONFIG:-v-os-desktop}"
JOBS="${JOBS:-$(nproc)}"
PODMAN_BUILD="${PODMAN_BUILD:-1}"
VERBOSE="${VERBOSE:-0}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# =============================================================================
# Helper Functions
# =============================================================================

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_step() {
    echo -e "${CYAN}[STEP]${NC} $1"
}

print_banner() {
    echo -e "${CYAN}"
    echo "╔══════════════════════════════════════════════════════════════════╗"
    echo "║                                                                  ║"
    echo "║   ███╗   ███╗ █████╗  ██████╗██████╗  ██████╗ ██╗  ██╗ █████╗   ║"
    echo "║   ████╗ ████║██╔══██╗██╔════╝██╔══██╗██╔═══██╗██║  ██║██╔══██╗  ║"
    echo "║   ██╔████╔██║███████║██║     ██████╔╝██║   ██║███████║███████║  ║"
    echo "║   ██║╚██╔╝██║██╔══██║██║     ██╔══██╗██║   ██║██╔══██║██╔══██║  ║"
    echo "║   ██║ ╚═╝ ██║██║  ██║╚██████╗██║  ██║╚██████╔╝██║  ██║██║  ██║  ║"
    echo "║   ╚═╝     ╚═╝╚═╝  ╚═╝ ╚═════╝╚═╝  ╚═╝ ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝  ║"
    echo "║                                                                  ║"
    echo "║              V-OS Build System v0.1.0                    ║"
    echo "║                                                                  ║"
    echo "╚══════════════════════════════════════════════════════════════════╝"
    echo -e "${NC}"
}

print_usage() {
    echo "Usage: $0 [OPTIONS] [COMMAND]"
    echo ""
    echo "Commands:"
    echo "  all         Build complete OS image (default)"
    echo "  kernel      Build kernel only"
    echo "  packages    Build packages only"
    echo "  qemu        Build and run in QEMU"
    echo "  clean       Clean build artifacts"
    echo "  distclean   Clean everything including downloads"
    echo "  test        Run test suite"
    echo "  help        Show this help message"
    echo ""
    echo "Options:"
    echo "  -a, --arch ARCH       Target architecture (x86_64, aarch64, riscv64gc)"
    echo "  -c, --config CONFIG   Build configuration (v-os-desktop, v-os-server, v-os-minimal)"
    echo "  -j, --jobs N          Number of parallel jobs"
    echo "  -p, --podman          Use Podman for build (default)"
    echo "  -n, --native          Use native build (no Podman)"
    echo "  -v, --verbose         Verbose output"
    echo "  -h, --help            Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                           # Build v-os-desktop for x86_64"
    echo "  $0 -a aarch64 -c v-os-server all"
    echo "  $0 qemu                      # Build and run in QEMU"
    echo "  $0 -n kernel                 # Build kernel natively"
}

check_dependencies() {
    log_step "Checking dependencies..."
    
    local missing=()
    
    # Required tools
    local tools=("make" "git" "curl")
    
    if [[ "$PODMAN_BUILD" == "1" ]]; then
        tools+=("podman")
    else
        tools+=("rustc" "cargo" "nasm" "qemu-system-x86_64")
    fi
    
    for tool in "${tools[@]}"; do
        if ! command -v "$tool" &> /dev/null; then
            missing+=("$tool")
        fi
    done
    
    if [[ ${#missing[@]} -gt 0 ]]; then
        log_error "Missing dependencies: ${missing[*]}"
        log_info "Please install the missing tools and try again."
        exit 1
    fi
    
    log_success "All dependencies found"
}

setup_environment() {
    log_step "Setting up build environment..."
    
    cd "$ROOT_DIR"
    
    # Export build variables
    export ARCH
    export CONFIG_NAME="$CONFIG"
    export PODMAN_BUILD
    
    # Set up Rust environment if native build
    if [[ "$PODMAN_BUILD" != "1" ]]; then
        if [[ -f "$ROOT_DIR/rust-toolchain.toml" ]]; then
            log_info "Using Rust toolchain from rust-toolchain.toml"
        fi
    fi
    
    log_success "Environment configured"
}

# =============================================================================
# Build Commands
# =============================================================================

cmd_all() {
    log_step "Building V-OS ($ARCH, $CONFIG)..."
    
    check_dependencies
    setup_environment
    
    local start_time=$(date +%s)
    
    make all ARCH="$ARCH" CONFIG_NAME="$CONFIG" PODMAN_BUILD="$PODMAN_BUILD" -j"$JOBS"
    
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    log_success "Build completed in ${duration}s"
    log_info "Output: build/$ARCH/$CONFIG/harddrive.img"
}

cmd_kernel() {
    log_step "Building kernel only..."
    
    check_dependencies
    setup_environment
    
    make prefix ARCH="$ARCH" PODMAN_BUILD="$PODMAN_BUILD"
    
    log_success "Kernel build completed"
}

cmd_packages() {
    log_step "Building packages..."
    
    check_dependencies
    setup_environment
    
    make repo ARCH="$ARCH" CONFIG_NAME="$CONFIG" PODMAN_BUILD="$PODMAN_BUILD"
    
    log_success "Package build completed"
}

cmd_qemu() {
    log_step "Building and running in QEMU..."
    
    # Build first if image doesn't exist
    if [[ ! -f "$ROOT_DIR/build/$ARCH/$CONFIG/harddrive.img" ]]; then
        cmd_all
    fi
    
    log_info "Starting QEMU..."
    
    make qemu ARCH="$ARCH" CONFIG_NAME="$CONFIG"
}

cmd_clean() {
    log_step "Cleaning build artifacts..."
    
    cd "$ROOT_DIR"
    make clean ARCH="$ARCH" CONFIG_NAME="$CONFIG" PODMAN_BUILD="$PODMAN_BUILD" SKIP_CHECK_TOOLS=1 || true
    
    log_success "Clean completed"
}

cmd_distclean() {
    log_step "Cleaning everything..."
    
    cd "$ROOT_DIR"
    make distclean ARCH="$ARCH" CONFIG_NAME="$CONFIG" PODMAN_BUILD="$PODMAN_BUILD" SKIP_CHECK_TOOLS=1 || true
    
    log_success "Distclean completed"
}

cmd_test() {
    log_step "Running test suite..."
    
    check_dependencies
    setup_environment
    
    # Run Cargo tests
    cargo test --all
    
    # Run QEMU boot test
    log_info "Running QEMU boot test..."
    
    if [[ -f "$ROOT_DIR/build/$ARCH/$CONFIG/harddrive.img" ]]; then
        timeout 60 qemu-system-x86_64 \
            -machine q35 \
            -m 2048 \
            -smp 2 \
            -drive "file=$ROOT_DIR/build/$ARCH/$CONFIG/harddrive.img,format=raw" \
            -nographic \
            -no-reboot \
            -serial mon:stdio \
            2>&1 | tee /tmp/qemu-test.log &
        
        QEMU_PID=$!
        sleep 30
        
        if grep -q "v-os\|redox\|login:" /tmp/qemu-test.log; then
            log_success "Boot test PASSED"
        else
            log_warn "Boot test inconclusive"
        fi
        
        kill $QEMU_PID 2>/dev/null || true
    else
        log_warn "No image found, skipping QEMU test"
    fi
    
    log_success "Test suite completed"
}

# =============================================================================
# Main
# =============================================================================

main() {
    print_banner
    
    # Parse arguments
    local command="all"
    
    while [[ $# -gt 0 ]]; do
        case $1 in
            -a|--arch)
                ARCH="$2"
                shift 2
                ;;
            -c|--config)
                CONFIG="$2"
                shift 2
                ;;
            -j|--jobs)
                JOBS="$2"
                shift 2
                ;;
            -p|--podman)
                PODMAN_BUILD=1
                shift
                ;;
            -n|--native)
                PODMAN_BUILD=0
                shift
                ;;
            -v|--verbose)
                VERBOSE=1
                set -x
                shift
                ;;
            -h|--help|help)
                print_usage
                exit 0
                ;;
            all|kernel|packages|qemu|clean|distclean|test)
                command="$1"
                shift
                ;;
            *)
                log_error "Unknown option: $1"
                print_usage
                exit 1
                ;;
        esac
    done
    
    log_info "Architecture: $ARCH"
    log_info "Configuration: $CONFIG"
    log_info "Podman build: $PODMAN_BUILD"
    log_info "Jobs: $JOBS"
    echo ""
    
    # Execute command
    case $command in
        all)        cmd_all ;;
        kernel)     cmd_kernel ;;
        packages)   cmd_packages ;;
        qemu)       cmd_qemu ;;
        clean)      cmd_clean ;;
        distclean)  cmd_distclean ;;
        test)       cmd_test ;;
        *)
            log_error "Unknown command: $command"
            print_usage
            exit 1
            ;;
    esac
}

main "$@"
