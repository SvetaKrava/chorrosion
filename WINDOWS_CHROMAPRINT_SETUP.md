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

### 2.1 Optional: FFmpeg headers for ffmpeg-support

If you enable the `ffmpeg-support` feature, use a Windows FFmpeg build that ships headers. The Gyan.dev release full shared archive includes `include` and `lib`:

```powershell
$ffmpegRoot = "C:\ffmpeg"
New-Item -ItemType Directory -Force -Path $ffmpegRoot | Out-Null

# If 7-Zip is not installed, use one of:
# winget install 7zip.7zip
# OR
# choco install 7zip -y

$sharedUrl = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-full-shared.7z"

Invoke-WebRequest -Uri $sharedUrl -OutFile "$ffmpegRoot\ffmpeg-shared.7z"
7z x "$ffmpegRoot\ffmpeg-shared.7z" -o"$ffmpegRoot\shared" -y

$ffmpegSharedDir = Get-ChildItem -Path "$ffmpegRoot\shared" -Directory | Select-Object -First 1

$env:FFMPEG_DIR = $ffmpegSharedDir.FullName
$env:PATH = "$($ffmpegSharedDir.FullName)\bin;$env:PATH"
```

### 3. Install LLVM (libclang)

Bindgen requires libclang on Windows. Install LLVM and set `LIBCLANG_PATH`.

- Option A: winget (recommended)

```powershell
winget install llvm.llvm
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
```

- Option B: Chocolatey

```powershell
choco install llvm -y
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
```

### 4. Configure PATH for Runtime

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
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
$env:PKG_CONFIG = "C:\util\vcpkg\installed\x64-windows\tools\pkgconf\pkgconf.exe"
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

## Error: "Unable to find libclang" from bindgen

- Ensure LLVM is installed and `LIBCLANG_PATH` points to the LLVM bin directory
- Default path: `C:\Program Files\LLVM\bin`

## Error: "'/usr/include/libavcodec/avfft.h' file not found" on Windows

- Ensure `PKG_CONFIG` points to vcpkg's pkgconf: `C:\util\vcpkg\installed\x64-windows\tools\pkgconf\pkgconf.exe`
- Ensure `PKG_CONFIG_PATH` is `C:\util\vcpkg\installed\x64-windows\lib\pkgconfig`
- If `ffmpeg-support` is enabled, set `FFMPEG_DIR` to a Windows FFmpeg dev package with headers (see Optional section above)

## vcpkg install fails

- Run `C:\util\vcpkg\vcpkg integrate install` to set up MSBuild integration
- Ensure Visual Studio Build Tools are installed
