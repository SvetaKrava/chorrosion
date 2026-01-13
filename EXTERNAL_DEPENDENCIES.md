<!-- markdownlint-disable-file MD024 -->

# External Dependencies

This document lists system-level dependencies required to build and run Chorrosion. Most Rust dependencies are managed by Cargo, but certain native libraries must be installed separately.

## Quick Setup

### Windows

```powershell
# Install vcpkg (one-time setup)
git clone https://github.com/microsoft/vcpkg C:\util\vcpkg
cd C:\util\vcpkg
.\bootstrap-vcpkg.bat
.\vcpkg integrate install

# Install chromaprint
C:\util\vcpkg\vcpkg install chromaprint:x64-windows

# Add to PATH (session)
$env:PATH="C:\util\vcpkg\installed\x64-windows\bin;$env:PATH"

# Or add permanently via Environment Variables (Windows Settings)
```

### Linux (Ubuntu/Debian)

```bash
sudo apt-get update
sudo apt-get install -y libchromaprint-dev libchromaprint0
```

### macOS

```bash
brew install chromaprint
```

---

## Detailed Dependency Breakdown

### 1. **Chromaprint** (Required)

**Purpose:** Audio fingerprinting library for identifying songs without metadata  
**Version:** 1.6.0 (via vcpkg) or latest (via system package manager)  
**Used by:** `chorrosion-fingerprint` crate for `FingerprintGenerator`

#### Installation by Platform

##### Windows

```powershell
# Requires vcpkg at C:\util\vcpkg
C:\util\vcpkg\vcpkg install chromaprint:x64-windows

# Verify installation
Get-ChildItem "C:\util\vcpkg\installed\x64-windows\lib\chromaprint.lib"
Get-ChildItem "C:\util\vcpkg\installed\x64-windows\bin\*.dll" | Select-String chromaprint
```

##### Linux (Ubuntu/Debian)

```bash
sudo apt-get install -y libchromaprint-dev libchromaprint0

# Verify installation
pkg-config --modversion libchromaprint
ldconfig -p | grep chromaprint
```

##### macOS

```bash
brew install chromaprint

# Verify installation
brew list chromaprint
pkg-config --modversion libchromaprint
```

### 2. **FFmpeg** (Indirect Dependency)

**Purpose:** Audio and video processing library  
**Pulled by:** Chromaprint (on Windows via vcpkg); system package managers on Linux/macOS  
**Note:** Not directly used in Rust code, but required by chromaprint's native library

#### Installation by Platform

##### Windows

- Automatically installed with `vcpkg install chromaprint:x64-windows`
- Location: `C:\util\vcpkg\installed\x64-windows\bin\`

##### Linux (Ubuntu/Debian)

- Automatically installed as a dependency of `libchromaprint-dev`
- Or install explicitly: `sudo apt-get install -y ffmpeg`

##### macOS

- Automatically installed with `brew install chromaprint`
- Or install explicitly: `brew install ffmpeg`

### 3. **Visual Studio Build Tools** (Windows Only)

**Purpose:** C++ compiler and linker for native library integration  
**Required for:** Linking against `chromaprint.lib`

#### Installation

```powershell
# Install via Visual Studio Installer
# Select: "Desktop development with C++" workload
# Or install Community Edition: https://visualstudio.microsoft.com/downloads/
```

#### Verification

```powershell
# Check if MSVC is available
cl.exe /? # Should show compiler help
link.exe /? # Should show linker help
```

### 4. **Rust Toolchain** (Required)

**Purpose:** Rust compiler and package manager  
**Version:** 1.56+ (tested with latest stable)

#### Installation

```bash
# Windows, macOS, Linux
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

---

## Build Configuration

### Windows-Specific Configuration

The project includes `.cargo/config.toml` for Windows to properly link native libraries:

```toml
[target.x86_64-pc-windows-msvc]
rustflags = ["-L", "C:/util/vcpkg/installed/x64-windows/lib"]
```

This tells the MSVC linker where to find `chromaprint.lib`.

### Environment Variables

#### Windows

```powershell
# For compilation (optional - handled by .cargo/config.toml)
$env:VCPKG_ROOT="C:\util\vcpkg"

# For runtime (required for tests/execution)
$env:PATH="C:\util\vcpkg\installed\x64-windows\bin;$env:PATH"
```

#### Linux/macOS

```bash
# Usually handled by pkg-config automatically
# But can be set if needed:
export PKG_CONFIG_PATH="/usr/lib/pkgconfig:/usr/local/lib/pkgconfig"
```

---

## Verification Checklist

Use this checklist to verify all dependencies are installed correctly:

- [ ] **Rust:** `rustc --version` shows 1.56+
- [ ] **Cargo:** `cargo --version` works
- [ ] **Chromaprint (Windows):** `C:\util\vcpkg\vcpkg list | Select-String chromaprint` shows installation
- [ ] **Chromaprint (Linux):** `pkg-config --modversion libchromaprint` shows version
- [ ] **Chromaprint (macOS):** `brew list chromaprint` shows installation
- [ ] **Build Tools (Windows):** `cl.exe /?` and `link.exe /?` work
- [ ] **PATH (Windows):** `Get-Command -Name "avcodec-60.dll" -ErrorAction SilentlyContinue` or verify manually
- [ ] **Build Test:** `cargo build -p chorrosion-fingerprint` compiles successfully
- [ ] **Test Run:** `cargo test -p chorrosion-fingerprint` passes all tests

---

## Troubleshooting

### "LINK : fatal error LNK1181: cannot open input file 'chromaprint.lib'"

> [!IMPORTANT]
> Windows only

- Verify `C:\util\vcpkg\installed\x64-windows\lib\chromaprint.lib` exists
- Check `.cargo/config.toml` has correct rustflags path
- Ensure vcpkg is installed at `C:\util\vcpkg`

### "DLL not found" at Runtime

> [!IMPORTANT]
> Windows only

- Add `C:\util\vcpkg\installed\x64-windows\bin` to PATH
- Or run: `$env:PATH="C:\util\vcpkg\installed\x64-windows\bin;$env:PATH"`

### "Package chromaprint not found by pkg-config"

> [!IMPORTANT]
> Linux/macOS

- Install development headers: `sudo apt-get install -y libchromaprint-dev` (Ubuntu) or `brew install chromaprint` (macOS)
- Verify: `pkg-config --list-all | grep chromaprint`

### vcpkg Integration Issues

> [!IMPORTANT]
> Windows

```powershell
# Reset vcpkg integration
C:\util\vcpkg\vcpkg integrate remove C:\util\vcpkg\vcpkg integrate install

# Re-install chromaprint
C:\util\vcpkg\vcpkg remove chromaprint:x64-windows
C:\util\vcpkg\vcpkg install chromaprint:x64-windows
```

---

## CI/CD Considerations

For GitHub Actions and other CI/CD systems:

### Windows Runners

```yaml
- name: Setup vcpkg
  run: |
    git clone https://github.com/microsoft/vcpkg C:\vcpkg
    cd C:\vcpkg
    .\bootstrap-vcpkg.bat
    .\vcpkg install chromaprint:x64-windows
    .\vcpkg integrate install

- name: Add vcpkg to PATH
  run: echo "C:\vcpkg\installed\x64-windows\bin" | Out-File -FilePath $env:GITHUB_PATH -Encoding utf8 -Append
```

### Linux Runners

```yaml
- name: Install dependencies
  run: sudo apt-get update && sudo apt-get install -y libchromaprint-dev libchromaprint0

- name: Build
  run: cargo build --verbose
```

### macOS Runners

```yaml
- name: Install dependencies
  run: brew install chromaprint

- name: Build
  run: cargo build --verbose
```

---

## Future Considerations

As the project evolves, additional external dependencies may be needed:

- **OGG/Opus/WavPack/APE Support** (Issue #89): May require additional codecs beyond symphonia if FFmpeg integration is added
- **MusicBrainz Integration**: Already handled via reqwest HTTP client (no system deps)
- **Database Drivers**: SQLite included; PostgreSQL support may require system libraries

---

## References

- [Chromaprint Documentation](https://acoustid.org/chromaprint)
- [vcpkg Package Manager](https://github.com/microsoft/vcpkg)
- [Rust Toolchain](https://rustup.rs/)
- See also: [WINDOWS_CHROMAPRINT_SETUP.md](WINDOWS_CHROMAPRINT_SETUP.md) for Windows-specific details
