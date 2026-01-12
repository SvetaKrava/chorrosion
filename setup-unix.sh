#!/usr/bin/env bash
# Automated setup script for Chorrosion development environment (Linux/macOS)
# Installs chromaprint and verifies the environment.

set -e

echo "Chorrosion Linux/macOS Development Setup"
echo "=========================================="
echo ""

# Detect OS
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    OS="linux"
    DISTRO=$(lsb_release -si 2>/dev/null || echo "Unknown")
elif [[ "$OSTYPE" == "darwin"* ]]; then
    OS="macos"
else
    echo "❌ Unsupported OS: $OSTYPE"
    exit 1
fi

echo "Detected OS: $OS ($OSTYPE)"
echo ""

# Step 1: Install chromaprint
echo "Step 1: Installing chromaprint and dependencies..."

if [ "$OS" = "linux" ]; then
    if command -v apt-get &> /dev/null; then
        echo "Using apt-get (Ubuntu/Debian)..."
        sudo apt-get update
        sudo apt-get install -y libchromaprint-dev libchromaprint0 ffmpeg
    elif command -v yum &> /dev/null; then
        echo "Using yum (RHEL/CentOS/Fedora)..."
        sudo yum install -y chromaprint-devel chromaprint ffmpeg
    elif command -v pacman &> /dev/null; then
        echo "Using pacman (Arch)..."
        sudo pacman -S --noconfirm chromaprint ffmpeg
    else
        echo "❌ Unsupported package manager. Please install chromaprint manually."
        exit 1
    fi
elif [ "$OS" = "macos" ]; then
    if ! command -v brew &> /dev/null; then
        echo "❌ Homebrew not found. Install from https://brew.sh"
        exit 1
    fi
    echo "Using brew (macOS)..."
    brew install chromaprint ffmpeg
fi

echo "✓ chromaprint and ffmpeg installed"
echo ""

# Step 2: Verify Rust
echo "Step 2: Verifying Rust toolchain..."

if ! command -v rustc &> /dev/null; then
    echo "❌ Rust not found. Install from https://rustup.rs"
    exit 1
fi

RUST_VERSION=$(rustc --version)
CARGO_VERSION=$(cargo --version)
echo "✓ $RUST_VERSION"
echo "✓ $CARGO_VERSION"
echo ""

# Step 3: Verification
echo "Step 3: Verifying installations..."

CHECKS_PASSED=0
CHECKS_TOTAL=0

check_command() {
    local name=$1
    local cmd=$2
    ((CHECKS_TOTAL++))
    
    if eval "$cmd" &> /dev/null; then
        echo "✓ $name"
        ((CHECKS_PASSED++))
    else
        echo "✗ $name"
    fi
}

check_file() {
    local name=$1
    local file=$2
    ((CHECKS_TOTAL++))
    
    if [ -f "$file" ]; then
        echo "✓ $name: $file"
        ((CHECKS_PASSED++))
    else
        echo "ℹ $name not found (may be okay, depending on system): $file"
    fi
}

check_command "pkg-config" "pkg-config --modversion libchromaprint"
check_command "chromaprint library" "pkg-config --list-all | grep -q chromaprint"

if [ "$OS" = "macos" ]; then
    check_file "chromaprint library" "$(brew --cellar chromaprint)/*/lib/libchromaprint.dylib"
fi

echo ""
echo "Verification: $CHECKS_PASSED/$CHECKS_TOTAL checks passed"
echo ""

# Step 4: Build test
echo "Step 4: Testing build..."

if cargo build -p chorrosion-fingerprint; then
    echo "✓ fingerprint crate builds successfully"
else
    echo "⚠ fingerprint crate build failed. Review errors above."
fi

echo ""
echo "✓ Setup complete!"
echo ""
echo "Next steps:"
echo "  1. Build: cargo build"
echo "  2. Test:  cargo test --workspace"
echo "  3. Run:   cargo run -p chorrosion-cli"
echo ""
echo "For more information, see EXTERNAL_DEPENDENCIES.md"
