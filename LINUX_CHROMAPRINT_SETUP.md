# Linux Chromaprint Setup Guide

This project uses chromaprint for audio fingerprinting. On Linux, the chromaprint native library must be available during compilation and runtime.

## Prerequisites

### 1. Install chromaprint and dependencies

On Ubuntu/Debian-based systems:

```bash
sudo apt-get update
sudo apt-get install -y libchromaprint-dev pkg-config
```

On Fedora/RHEL-based systems:

```bash
sudo dnf install chromaprint-devel pkgconfig
```

On Arch:

```bash
sudo pacman -S chromaprint
```

### 2. Verify Installation

Verify that chromaprint is installed and pkg-config can find it:

```bash
pkg-config --cflags --libs libchromaprint
```

This should output something like:

```bash
-I/usr/include -L/usr/lib/x86_64-linux-gnu -lchromaprint
```

If this command fails, try setting the PKG_CONFIG_PATH explicitly:

```bash
export PKG_CONFIG_PATH="/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig:$PKG_CONFIG_PATH"
pkg-config --cflags --libs libchromaprint
```

## Building and Testing

With chromaprint installed and pkg-config configured, you can build and test normally:

```bash
cargo build
cargo test --workspace
```

### Setting PKG_CONFIG_PATH if needed

If pkg-config still cannot find chromaprint, you may need to set the environment variable:

```bash
export PKG_CONFIG_PATH="/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig:$PKG_CONFIG_PATH"
cargo build
cargo test --workspace
```

Or add it to your shell profile (`~/.bashrc`, `~/.zshrc`, etc.):

```bash
export PKG_CONFIG_PATH="/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig:$PKG_CONFIG_PATH"
```

## Troubleshooting

### Error: "rust-lld: error: unable to find library -lchromaprint"

- Ensure chromaprint is installed: `sudo apt-get install -y libchromaprint-dev`
- Verify pkg-config can find it: `pkg-config --list-all | grep chromaprint`
- Try setting PKG_CONFIG_PATH as shown above

### Error: "package 'libchromaprint' not found"

- Install pkg-config: `sudo apt-get install -y pkg-config`
- Check the installation: `dpkg -l | grep chromaprint`
- If not listed, reinstall: `sudo apt-get install --reinstall libchromaprint-dev`

### Build fails with linking errors

- Try clearing Cargo cache:

  ```bash
  cargo clean
  cargo build
  ```

- Ensure pkg-config is in PATH:

  ```bash
  which pkg-config
  ```

### Architecture mismatch

- Ensure the chromaprint library matches your system architecture:

  ```bash
  dpkg -l libchromaprint0  # Check installed architecture
  uname -m                 # Check system architecture
  ```

## GitHub Actions CI

The GitHub Actions CI automatically installs chromaprint and sets PKG_CONFIG_PATH on Ubuntu runners using:

```yaml
env:
  PKG_CONFIG_PATH: /usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig

- name: Install chromaprint (Ubuntu)
  if: runner.os == 'linux'
  run: sudo apt-get update && sudo apt-get install -y libchromaprint-dev pkg-config
```

No additional setup is needed in CI environments.
