# Chorrosion System Requirements

## Quick Reference

| Component | Windows | Linux | macOS |
| --------- | ------- | ----- | ----- |
| **Rust** | ✓ via rustup | ✓ via rustup | ✓ via rustup |
| **Cargo** | ✓ (with Rust) | ✓ (with Rust) | ✓ (with Rust) |
| **chromaprint** | ✓ via vcpkg | ✓ apt-get/yum/pacman | ✓ brew |
| **ffmpeg** | ✓ via vcpkg | ✓ auto (with chromaprint) | ✓ auto (with brew) |
| **MSVC Build Tools** | ✓ required | ✗ N/A | ✗ N/A |

## Installation Commands

### Windows (PowerShell)

```powershell
# One-time: Run automated setup
.\setup-windows.ps1

# Or manual setup:
git clone https://github.com/microsoft/vcpkg C:\util\vcpkg
C:\util\vcpkg\bootstrap-vcpkg.bat
C:\util\vcpkg\vcpkg install chromaprint:x64-windows
$env:PATH="C:\util\vcpkg\installed\x64-windows\bin;$env:PATH"
```

### Linux

```bash
# Ubuntu/Debian
chmod +x setup-unix.sh
./setup-unix.sh

# Or manual:
sudo apt-get install -y libchromaprint-dev libchromaprint0 ffmpeg
```

### macOS

```bash
# Using Homebrew
chmod +x setup-unix.sh
./setup-unix.sh

# Or manual:
brew install chromaprint ffmpeg
```

## Build & Test

```bash
# Build all crates
cargo build

# Run tests
cargo test --workspace

# Format code
cargo fmt --all

# Lint checks
cargo clippy --all-targets -- -D warnings

# Run the CLI
cargo run -p chorrosion-cli
```

## Documentation

- **Detailed Instructions**: [EXTERNAL_DEPENDENCIES.md](EXTERNAL_DEPENDENCIES.md)
- **Windows-Specific**: [WINDOWS_CHROMAPRINT_SETUP.md](WINDOWS_CHROMAPRINT_SETUP.md)
- **Setup Scripts**:
  - Windows: `setup-windows.ps1`
  - Linux/macOS: `setup-unix.sh`

## Environment Variables (Optional)

```bash
# Logging
RUST_LOG=info,api=debug,registry=debug

# Database location (default: sqlite://data/chorrosion.db)
CHORROSION_DATABASE__URL=sqlite://data/chorrosion.db

# Server binding (default: 127.0.0.1:5150)
CHORROSION_HTTP__HOST=127.0.0.1
CHORROSION_HTTP__PORT=5150
```

## Verification

After installation, verify everything works:

```bash
# Check Rust
rustc --version
cargo --version

# Check chromaprint (Linux/macOS)
pkg-config --modversion libchromaprint

# Check chromaprint (Windows)
dir C:\util\vcpkg\installed\x64-windows\lib\chromaprint.lib

# Build test
cargo build -p chorrosion-fingerprint

# Full test suite
cargo test --workspace
```

## Troubleshooting

**Issue**: "chromaprint.lib not found" (Windows)  
**Solution**: Ensure vcpkg is at `C:\util\vcpkg` and run `.\setup-windows.ps1`

**Issue**: "DLL not found" at runtime (Windows)  
**Solution**: Add to PATH: `C:\util\vcpkg\installed\x64-windows\bin`

**Issue**: pkg-config can't find chromaprint (Linux)  
**Solution**: `sudo apt-get install libchromaprint-dev`

For more detailed troubleshooting, see [EXTERNAL_DEPENDENCIES.md](EXTERNAL_DEPENDENCIES.md#troubleshooting).
