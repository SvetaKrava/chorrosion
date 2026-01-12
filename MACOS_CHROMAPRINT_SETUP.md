# macOS Chromaprint Setup Guide

This project uses chromaprint for audio fingerprinting. On macOS, the chromaprint native library must be available during compilation and runtime.

## Prerequisites

### 1. Homebrew Installation

If you don't have Homebrew installed, install it first:

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

### 2. Install chromaprint

Install chromaprint via Homebrew:

```bash
brew install chromaprint
```

This installs the chromaprint library including dependencies needed for audio decoding.

## Verifying Installation

To verify chromaprint is installed correctly:

```bash
brew list chromaprint
pkg-config --cflags --libs libchromaprint
```

The second command should output something like:

```bash
-I/usr/local/opt/chromaprint/include -L/usr/local/opt/chromaprint/lib -lchromaprint
```

Or on Apple Silicon Macs (M1/M2/M3):

```bash
-I/opt/homebrew/opt/chromaprint/include -L/opt/homebrew/opt/chromaprint/lib -lchromaprint
```

## Building and Testing

With chromaprint installed via Homebrew, you can build and test normally:

```bash
cargo build
cargo test --workspace
```

The linker will automatically find chromaprint through `pkg-config`.

## Architecture-Specific Notes

### Intel Macs

Chromaprint will be installed to `/usr/local/opt/chromaprint`

### Apple Silicon Macs (M1/M2/M3)

Chromaprint will be installed to `/opt/homebrew/opt/chromaprint`

Homebrew handles both architectures transparently, so no manual configuration is needed.

## Troubleshooting

### Error: "ld: library 'chromaprint' not found"

- Ensure chromaprint is installed: `brew install chromaprint`
- Verify the installation: `brew list chromaprint`
- Check pkg-config can find it: `pkg-config --list-all | grep chromaprint`

### Error: "Package chromaprint not found"

- If `pkg-config` isn't working, try:

  ```bash
  brew install pkg-config
  ```

- Then reinstall chromaprint:

  ```bash
  brew reinstall chromaprint
  ```

### Build still fails after reinstalling

- Try clearing Cargo cache:

  ```bash
  cargo clean
  cargo build
  ```

### Cross-compilation issues

- If cross-compiling to a different architecture (e.g., compiling for ARM on Intel or vice versa), ensure you've installed the appropriate target:

  ```bash
  rustup target add aarch64-apple-darwin  # For Apple Silicon target
  rustup target add x86_64-apple-darwin   # For Intel target
  ```

## GitHub Actions CI

The GitHub Actions CI automatically installs chromaprint on macOS runners using:

```yaml
- name: Install chromaprint (macOS)
  if: runner.os == 'macos'
  run: brew install chromaprint
```

No additional setup is needed in CI environments.
