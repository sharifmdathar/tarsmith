# TarSmith

A simple, interactive installer for tar archives (`.tar.gz`, `.tar.xz`, `.tar.bz2`, etc.) that automates the installation process for Linux applications distributed as compressed archives.

## Features

- 🎯 **Smart Detection**: Automatically detects the extracted directory and application name
- 📦 **Multiple Formats**: Supports `.tar.gz`, `.tar.xz`, `.tar.bz2`, `.tar.zst`, and uncompressed `.tar` files
- 🖥️ **Installation Types**: Choose between user-level (`~/.local/tarsmith`) or system-wide (`/opt`) installation
- 🎨 **Desktop Integration**: Optionally create desktop entries for GUI applications
- 🔗 **PATH Management**: Automatically adds selected executables to your PATH
- 🛠️ **Multiple Executables**: Select which binaries to add to PATH (useful for tools like Node.js with `node`, `npm`, `npx`, etc.)
- 🐚 **Shell Detection**: Automatically configures PATH for bash, zsh, or fish shells
- ✨ **Zero Configuration**: Sensible defaults - just press Enter to accept
- 🚀 **Non-Interactive Mode**: Use command-line flags for automated installations

## Demo

Watch TarSmith in action:

https://github.com/user-attachments/assets/2b726305-ffc2-47d7-91fb-f1e544e6a91b

## Installation

### Prerequisites

- `tar` command-line tool (usually pre-installed on Linux)

### Download Pre-built Binary (Recommended)

Pre-built binaries are available in the [Releases](https://github.com/sharifmdathar/tarsmith/releases) section.

1. Download the binary
2. Make it executable:
   ```bash
   chmod +x tarsmith
   ```
3. Move it to a directory in your PATH (optional):
   ```bash
   sudo mv tarsmith /usr/local/bin/tarsmith
   ```
   Or for user-level installation:
   ```bash
   mkdir -p ~/.local/bin
   mv tarsmith ~/.local/bin/tarsmith
   ```

### Build from Source

If you prefer to build from source, you'll need:

- Rust (latest stable version)

```bash
git clone https://github.com/sharifmdathar/tarsmith
cd tarsmith
cargo build --release
```

The binary will be available at `target/release/tarsmith`.

### Install System-Wide (Optional)

```bash
sudo cp target/release/tarsmith /usr/local/bin/
```

## Usage

### Basic Usage

```bash
tarsmith <archive-file>
```

### Command-Line Options

- `-s, --system`: Install system-wide to `/opt` (non-interactive)
- `-u, --user`: Install user-level to `~/.local/tarsmith` (non-interactive)
- `-nd, --no-desktop`: Skip desktop entry creation
- `-np, --no-path`: Skip adding executables to PATH
- `-h, --help`: Print help information
- `-V, --version`: Print version information

### Examples

#### Install Node.js

```bash
tarsmith node-v24.11.1-linux-x64.tar.xz
```

When prompted:

- **Installation type**: Press Enter (defaults to user-level)
- **Desktop entry**: Press Enter (skip, since Node.js is CLI-only)
- **Add to PATH**: Press Enter (defaults to all executables: `node`, `npm`, `npx`, `corepack`)

After installation, restart your terminal or run `source ~/.bashrc` (or your shell's config file) to use the commands.

#### Install Android Studio

```bash
tarsmith android-studio-2025.2.1.7-linux.tar.gz
```

When prompted:

- **Installation type**: Press Enter (user-level)
- **Desktop entry**: Select `2` for `studio` executable
- **Add to PATH**: Select `2` for `studio` executable (or `all` for all executables)

#### Install with Custom Selections

```bash
tarsmith myapp-1.0.0-linux.tar.gz
```

- **Desktop entry**: Select `1` for the main executable
- **Add to PATH**: Enter `1 3 5` to add specific executables (space-separated), or `all` for all

#### Non-Interactive Installation

Install without prompts using command-line flags:

```bash
# User-level installation (non-interactive)
tarsmith node-v24.11.1-linux-x64.tar.xz -u
# or: tarsmith node-v24.11.1-linux-x64.tar.xz --user

# System-wide installation (non-interactive)
sudo tarsmith android-studio.tar.gz -s
# or: sudo tarsmith android-studio.tar.gz --system

# Skip desktop entry creation
tarsmith app.tar.gz -u -nd
# or: tarsmith app.tar.gz --user --no-desktop

# Skip PATH symlinks
tarsmith app.tar.gz -u -np
# or: tarsmith app.tar.gz --user --no-path

# Full non-interactive installation (using shorthand)
sudo tarsmith app.tar.gz -s -nd -np
# or: sudo tarsmith app.tar.gz --system --no-desktop --no-path
```

**Non-interactive mode defaults:**

- When `--system` or `--user` is specified:
  - Desktop entry: Uses the first executable found (unless `--no-desktop`)
  - PATH: Adds all executables to PATH (unless `--no-path`)

## How It Works

1. **Extraction**: Detects compression format and extracts the archive to the chosen directory
2. **Detection**: Finds the main extracted folder by matching archive name and checking for `bin/` directories
3. **Name Inference**: Extracts a clean app name (removes version numbers and platform suffixes)
4. **Executable Discovery**: Scans the `bin/` directory for executable files
5. **Desktop Entry**: Optionally creates a `.desktop` file for GUI applications
6. **PATH Setup**: Creates symlinks in `~/.local/bin` (user-level) or `/usr/local/bin` (system-wide)
7. **Shell Configuration**: Automatically adds `~/.local/bin` to PATH in your shell config file

## Installation Locations

### User-Level (Default)

- **Installation**: `~/.local/tarsmith/<app-name>/`
- **Desktop Entry**: `~/.local/share/applications/<app-name>.desktop`
- **PATH Symlinks**: `~/.local/bin/`
- **No sudo required**

### System-Wide

- **Installation**: `/opt/<app-name>/`
- **Desktop Entry**: `/usr/share/applications/<app-name>.desktop`
- **PATH Symlinks**: `/usr/local/bin/`
- **Requires sudo** for desktop entries

## Supported Archive Formats

- `.tar.gz` / `.tgz` - Gzip compression
- `.tar.xz` / `.txz` - XZ compression
- `.tar.bz2` - Bzip2 compression
- `.tar.zst` - Zstandard compression
- `.tar` - Uncompressed

## Features in Detail

### Smart Directory Detection

TarSmith uses multiple strategies to find the correct extracted directory:

1. Exact match with archive name
2. Name variations (handles hyphens/underscores)
3. Directories containing `bin/` folder (sorted by modification time)
4. Newest directory as fallback

This ensures it finds the right folder even if the archive structure is unexpected.

### Multiple Executable Selection

When multiple executables are found, you can:

- Select one for desktop entry (GUI launcher)
- Select multiple for PATH (space-separated: `1 2 3` or type `all`)

Perfect for tools like Node.js where you want `node`, `npm`, `npx`, and `corepack` all available.

### Automatic PATH Configuration

For user-level installations, TarSmith automatically:

- Checks if `~/.local/bin` is already in PATH
- Detects your shell (bash/zsh/fish)
- Adds the appropriate export statement to your shell config
- Avoids duplicate entries

## Troubleshooting

### "Command not found" after installation

Restart your terminal or run:

```bash
source ~/.bashrc  # for bash
source ~/.zshrc   # for zsh
```

### Wrong directory detected

TarSmith prioritizes directories with `bin/` folders and sorts by modification time. If it picks the wrong one:

- Ensure the archive extracts to a directory with a `bin/` subdirectory
- The directory name should match or be similar to the archive name

### Permission denied errors

- For user-level installation: Ensure you have write permissions to `~/.local/`
- For system-wide installation: Run with `sudo` for desktop entries in `/usr/share/applications/`

## Requirements

- **Rust**: 1.70+ (for edition 2024)
- **Dependencies**:
  - `dirs` crate (for home directory detection)
- **System**: Linux (uses Unix-specific features)

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Author

Created to simplify the installation of Linux applications distributed as tar archives.
