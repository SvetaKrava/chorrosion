# Windows Chromaprint Setup Guide

This project uses chromaprint for audio fingerprinting. On Windows, the chromaprint native library must be available during compilation and runtime.

## Prerequisites

### 1. vcpkg Installation

Install vcpkg at `C:\util\vcpkg` if not already present:

```powershell
git clone https://github.com/microsoft/vcpkg
cd vcpkg
.\bootstrap-vcpkg.bat
```

### 2. Install chromaprint

```powershell
C:\util\vcpkg\vcpkg install chromaprint:x64-windows
```

This installs chromaprint 1.6.0 with ffmpeg dependencies for audio decoding.

### 3. Configure PATH for Runtime

When running tests or the application, the ffmpeg DLLs must be available:

```powershell
$env:PATH="C:\util\vcpkg\installed\x64-windows\bin;$env:PATH"
```

Or add permanently to system environment variables.

## Configuration Details

### Build Configuration (.cargo/config.toml)

The project includes a `.cargo/config.toml` that adds the vcpkg lib directory to the linker search path:

```toml
[target.x86_64-pc-windows-msvc]
rustflags = ["-L", "C:/util/vcpkg/installed/x64-windows/lib"]
```

This allows the MSVC linker to find `chromaprint.lib`.

## Building and Testing

With PATH configured:

```powershell
$env:PATH="C:\util\vcpkg\installed\x64-windows\bin;$env:PATH"
cargo build
cargo test --workspace
```

## Troubleshooting

## Error: "LINK : fatal error LNK1181: cannot open input file 'chromaprint.lib'"

- Verify `C:\util\vcpkg\installed\x64-windows\lib\chromaprint.lib` exists
- Check `.cargo/config.toml` rustflags are correct

## Error: "DLL not found" at runtime

- Ensure PATH includes `C:\util\vcpkg\installed\x64-windows\bin`
- This directory contains ffmpeg DLLs required by chromaprint

## vcpkg install fails

- Run `C:\util\vcpkg\vcpkg integrate install` to set up MSBuild integration
- Ensure Visual Studio Build Tools are installed
