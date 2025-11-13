use std::env;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: tarsmith <file.tar.gz>");
        std::process::exit(1);
    }

    let archive_path = Path::new(&args[1]);

    println!("=== TarSmith Installer ===");
    println!("Input file: {}", archive_path.display());
    println!();

    if !archive_path.exists() {
        return Err(format!("Archive not found: {}", archive_path.display()).into());
    }
    println!("[1] File exists ✔");

    println!("Choose installation type:");
    println!("1) User-level (~/.local/tarsmith) [default]");
    println!("2) System-wide (/opt)");
    print!("Enter 1 or 2 (default: 1): ");
    io::stdout().flush()?;

    let mut choice = String::new();
    io::stdin().read_line(&mut choice)?;
    let choice = choice.trim();

    let (install_dir, is_user_level) = if choice == "2" {
        // Check sudo permissions before proceeding with system-wide installation
        if !check_sudo_permissions() {
            eprintln!("Error: System-wide installation requires sudo privileges.");
            eprintln!("Please run with: sudo tarsmith {}", archive_path.display());
            eprintln!("Or choose user-level installation (option 1) which doesn't require sudo.");
            std::process::exit(1);
        }
        (Path::new("/opt").to_path_buf(), false)
    } else {
        (dirs::home_dir().unwrap().join(".local/tarsmith"), true)
    };

    if !install_dir.exists() {
        println!("[2] Creating install directory: {}", install_dir.display());
        fs::create_dir_all(&install_dir)?;
    } else {
        println!("[2] Install directory exists: {}", install_dir.display());
    }

    println!("[2] Install directory ready ✔");

    println!("[3] Extracting archive...");

    // Detect compression format and select appropriate tar flags
    let tar_flags = archive_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            if ext == "gz" || ext == "tgz" {
                "-xzf" // gzip
            } else if ext == "xz" || ext == "txz" {
                "-xJf" // xz
            } else if ext == "bz2" {
                "-xjf" // bzip2
            } else if ext == "zst" {
                "--zstd -xf" // zstd (needs special handling)
            } else {
                "-xf" // uncompressed tar
            }
        })
        .unwrap_or("-xf");

    let mut cmd = Command::new("tar");

    // zstd requires separate --zstd flag, not combined with -x
    if tar_flags.contains("zstd") {
        cmd.args(&["--zstd", "-xf", archive_path.to_str().unwrap()]);
    } else {
        cmd.arg(tar_flags);
        cmd.arg(archive_path);
    }

    cmd.arg("-C").arg(&install_dir);

    let status = cmd.status()?;

    if !status.success() {
        return Err("Extraction failed".into());
    }
    println!("[3] Extraction complete ✔");

    println!("[4] Detecting installation folder...");
    let extracted_path = detect_extracted_folder(&install_dir, archive_path)?;
    println!(
        "[4] Detected installation directory: {} ✔",
        extracted_path.display()
    );

    let app_name = infer_app_name(&extracted_path)?;
    println!("[4] Inferred app name: {} ✔", app_name);

    let exec_path = extracted_path.join("bin");
    let executables = find_executables_in_bin(&exec_path)?;

    // Select executable for desktop entry (GUI launcher)
    println!("[5] Select executable for desktop entry (GUI launch):");
    let desktop_exec = if executables.len() == 1 {
        println!(
            "  Only one executable found, using: {}",
            executables[0]
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        );
        Some(executables[0].clone())
    } else {
        println!("  Executables found:");
        for (i, exe) in executables.iter().enumerate() {
            println!(
                "    {}) {}",
                i + 1,
                exe.file_name().unwrap_or_default().to_string_lossy()
            );
        }
        println!("    0) Skip desktop entry");
        print!(
            "  Select executable (0-{}) [default: 0]: ",
            executables.len()
        );
        io::stdout().flush()?;

        let mut selection = String::new();
        io::stdin().read_line(&mut selection)?;
        let selection = selection.trim();

        if selection.is_empty() || selection == "0" {
            None
        } else {
            let selection: usize = selection.parse().map_err(|_| "Invalid selection")?;
            if selection < 1 || selection > executables.len() {
                return Err("Invalid selection".into());
            }
            Some(executables[selection - 1].clone())
        }
    };

    // Create desktop entry if an executable was selected
    if let Some(exec_file) = &desktop_exec {
        println!("[6] Creating desktop entry...");
        let desktop_filename = format!("{}.desktop", app_name);
        let desktop_path = if is_user_level {
            dirs::home_dir()
                .unwrap()
                .join(".local/share/applications")
                .join(&desktop_filename)
        } else {
            Path::new("/usr/share/applications").join(&desktop_filename)
        };

        // Ensure the applications directory exists
        if let Some(parent) = desktop_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Try to find an icon (common locations)
        let icon_path = find_icon(&extracted_path)
            .unwrap_or_else(|| extracted_path.join("bin").join("icon.png"));

        let desktop_contents = format!(
            "[Desktop Entry]
Version=1.0
Type=Application
Name={}
Exec={}
Icon={}
Terminal=false
Categories=Utility;
",
            app_name,
            exec_file.display(),
            icon_path.display()
        );

        fs::write(&desktop_path, desktop_contents)?;
        println!("[6] Desktop entry created at: {} ✔", desktop_path.display());
    } else {
        println!("[6] Skipped desktop entry creation ✔");
    }

    // Select executables to add to PATH (for terminal access)
    println!("[7] Select executables to add to PATH (for terminal use):");
    if executables.len() == 1 {
        println!(
            "  Only one executable found: {}",
            executables[0]
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        );
        print!("  Add to PATH? (Y/n): ");
        io::stdout().flush()?;

        let mut response = String::new();
        io::stdin().read_line(&mut response)?;
        let response = response.trim().to_lowercase();

        let selected_for_path = if response == "n" || response == "no" {
            Vec::new()
        } else {
            vec![executables[0].clone()]
        };

        if !selected_for_path.is_empty() {
            create_path_symlinks(&selected_for_path, is_user_level)?;
        } else {
            println!("[7] Skipped adding to PATH ✔");
        }
    } else {
        println!("  Executables found:");
        for (i, exe) in executables.iter().enumerate() {
            println!(
                "    {}) {}",
                i + 1,
                exe.file_name().unwrap_or_default().to_string_lossy()
            );
        }
        print!(
            "  Enter numbers separated by commas (e.g., 1,2,3) or 'all' for all [default: all]: "
        );
        io::stdout().flush()?;

        let mut selection = String::new();
        io::stdin().read_line(&mut selection)?;
        let selection = selection.trim().to_lowercase();

        let selected_for_path = if selection.is_empty() || selection == "all" {
            executables.clone()
        } else {
            // Parse comma-separated selection (e.g., "1,2,3")
            let indices: Result<Vec<usize>, _> = selection
                .split(',')
                .map(|s| {
                    s.trim()
                        .parse::<usize>()
                        .map_err(|_| "Invalid number format".to_string())
                })
                .collect();

            let indices = indices.map_err(|e| e)?;

            let mut selected = Vec::new();
            for idx in indices {
                if idx < 1 || idx > executables.len() {
                    return Err(format!("Invalid selection: {}", idx).into());
                }
                selected.push(executables[idx - 1].clone());
            }
            selected
        };

        if selected_for_path.is_empty() {
            println!("[7] Skipped adding to PATH ✔");
        } else {
            create_path_symlinks(&selected_for_path, is_user_level)?;
        }
    }

    println!(
        "
Installation complete! 🎉"
    );
    println!("Installed to: {}", extracted_path.display());
    if let Some(exec_file) = &desktop_exec {
        println!(
            "Desktop entry created for: {}",
            exec_file.file_name().unwrap_or_default().to_string_lossy()
        );
    }

    Ok(())
}

/// Checks if the current user has sudo/root permissions for system-wide installation
/// Returns true if we can write to /opt (either as root or with sudo)
fn check_sudo_permissions() -> bool {
    let opt_path = Path::new("/opt");

    // Try to create a test file in /opt to check write permissions
    let test_file = opt_path.join(".tarsmith_test_write_permissions");

    // Try writing a test file
    match fs::write(&test_file, "test") {
        Ok(_) => {
            // Successfully wrote, clean up and return true
            fs::remove_file(&test_file).ok();
            true
        }
        Err(_) => {
            // Can't write, check if sudo is available and works
            let sudo_check = Command::new("sudo").arg("-n").arg("true").output();

            if let Ok(output) = sudo_check {
                output.status.success()
            } else {
                false
            }
        }
    }
}

/// Detects the main extracted directory by trying multiple strategies:
/// 1. Exact match with archive name
/// 2. Name variations (handling hyphens/underscores)
/// 3. Directories containing bin/ folder (sorted by modification time)
/// 4. Newest directory as fallback
fn detect_extracted_folder(install_dir: &Path, archive: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let stem = archive
        .file_stem()
        .ok_or("Cannot find archive name")?
        .to_string_lossy()
        .replace(".tar", "");

    let candidate = install_dir.join(&stem);
    if candidate.exists() && candidate.join("bin").exists() {
        return Ok(candidate);
    }

    // Extract base name (first component before version/platform suffixes)
    let stem_first_part = stem
        .split(&['-', '_'][..])
        .next()
        .unwrap_or(&stem)
        .to_string();

    // Try multiple naming variations (handles both hyphens and underscores)
    let stem_variants = vec![
        stem.clone(),
        stem.replace('_', "-"),
        stem.replace('-', "_"),
        stem.split(&['-', '_'][..])
            .take_while(|p| {
                !p.chars()
                    .next()
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>()
            .join("-"),
        stem.split(&['-', '_'][..])
            .take_while(|p| {
                !p.chars()
                    .next()
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>()
            .join("_"),
        stem_first_part.clone(),
    ];

    for variant in stem_variants {
        let candidate = install_dir.join(&variant);
        if candidate.exists() && candidate.join("bin").exists() {
            return Ok(candidate);
        }
    }

    // Collect directories with bin/ folder, along with their modification times
    let mut candidates_with_bin: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
    for entry in fs::read_dir(install_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() && path.join("bin").exists() {
            if let Ok(metadata) = fs::metadata(&path) {
                if let Ok(modified) = metadata.modified() {
                    candidates_with_bin.push((path, modified));
                }
            }
        }
    }

    if !candidates_with_bin.is_empty() {
        // Sort by modification time (newest first) to prioritize recently extracted folders
        candidates_with_bin.sort_by(|a, b| b.1.cmp(&a.1));

        // Prefer directories matching the archive name, but use newest if multiple matches
        if let Some(matched) = candidates_with_bin.iter().find(|(p, _)| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| {
                    let name_lower = n.to_lowercase();
                    name_lower.starts_with(&stem_first_part.to_lowercase())
                        || name_lower.contains(&stem_first_part.to_lowercase())
                })
                .unwrap_or(false)
        }) {
            return Ok(matched.0.clone());
        }

        // Fallback: return newest directory if no name match found
        return Ok(candidates_with_bin[0].0.clone());
    }

    // Last resort: find newest directory (ignoring bin/ requirement)
    let mut newest: Option<(PathBuf, std::time::SystemTime)> = None;
    for entry in fs::read_dir(install_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let metadata = fs::metadata(&path)?;
            if let Ok(time) = metadata.modified() {
                if newest.is_none() || time > newest.as_ref().unwrap().1 {
                    newest = Some((path.clone(), time));
                }
            }
        }
    }

    newest
        .map(|n| n.0)
        .ok_or_else(|| "Unable to detect extracted folder".into())
}

/// Finds all executable files in the bin/ directory by checking file permissions
fn find_executables_in_bin(bin_dir: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut executables = Vec::new();

    for entry in fs::read_dir(bin_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let metadata = fs::metadata(&path)?;
            let perms = metadata.permissions();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                // Check if file has execute permission (0o111 = --x--x--x)
                if perms.mode() & 0o111 != 0 {
                    executables.push(path);
                }
            }
        }
    }

    if executables.is_empty() {
        Err("No executable found in bin/ folder".into())
    } else {
        Ok(executables)
    }
}

/// Extracts a clean application name from the extracted folder path
/// Removes version numbers and platform suffixes (e.g., "android-studio-2025.2.1.7-linux" -> "android-studio")
fn infer_app_name(extracted_path: &Path) -> Result<String, Box<dyn Error>> {
    let folder_name = extracted_path
        .file_name()
        .ok_or("Cannot get folder name")?
        .to_string_lossy();

    let name = folder_name
        .split('-')
        .take_while(|part| {
            // Stop when we encounter version numbers or platform identifiers
            !part
                .chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
                && !matches!(
                    part.to_lowercase().as_str(),
                    "linux" | "x64" | "x86" | "amd64" | "arm64" | "aarch64"
                )
        })
        .collect::<Vec<_>>()
        .join("-");

    if name.is_empty() {
        Ok(folder_name.to_string())
    } else {
        Ok(name)
    }
}

/// Searches common locations for application icon files
fn find_icon(extracted_path: &Path) -> Option<PathBuf> {
    let common_icon_paths = vec![
        extracted_path.join("bin").join("icon.png"),
        extracted_path.join("bin").join("studio.png"),
        extracted_path.join("icon.png"),
        extracted_path.join("icon.svg"),
        extracted_path.join("bin").join("icon.svg"),
    ];

    for icon_path in common_icon_paths {
        if icon_path.exists() {
            return Some(icon_path);
        }
    }

    None
}

/// Creates symlinks for selected executables in the appropriate bin directory
/// For user-level: ~/.local/bin, for system-wide: /usr/local/bin
fn create_path_symlinks(
    executables: &[PathBuf],
    is_user_level: bool,
) -> Result<(), Box<dyn Error>> {
    let bin_dir = if is_user_level {
        dirs::home_dir().unwrap().join(".local/bin")
    } else {
        Path::new("/usr/local/bin").to_path_buf()
    };

    if !bin_dir.exists() {
        fs::create_dir_all(&bin_dir)?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        for exec_file in executables {
            let symlink_name = exec_file
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let symlink_path = bin_dir.join(&symlink_name);

            // Remove existing symlink to avoid conflicts
            if symlink_path.exists() || symlink_path.is_symlink() {
                fs::remove_file(&symlink_path).ok();
            }

            symlink(exec_file, &symlink_path)?;
            println!(
                "    Created symlink: {} -> {}",
                symlink_name,
                exec_file.display()
            );
        }
    }

    // For user-level installations, ensure ~/.local/bin is in PATH
    if is_user_level {
        ensure_local_bin_in_path()?;
    }

    let names: Vec<String> = executables
        .iter()
        .map(|e| {
            e.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        })
        .collect();
    println!(
        "    You can now run these commands from your terminal: {}",
        names.join(", ")
    );

    Ok(())
}

/// Ensures ~/.local/bin is added to PATH by modifying the user's shell config file
/// Detects shell type (bash/zsh/fish) and adds appropriate export statement
fn ensure_local_bin_in_path() -> Result<(), Box<dyn Error>> {
    let local_bin = dirs::home_dir()
        .ok_or("Cannot determine home directory")?
        .join(".local/bin");

    let local_bin_str = local_bin.to_string_lossy().to_string();

    // Check if already in PATH by examining each component
    if let Ok(path_var) = env::var("PATH") {
        let path_components: Vec<&str> = path_var.split(':').collect();
        if path_components
            .iter()
            .any(|p| p == &local_bin_str || p.ends_with(".local/bin"))
        {
            println!("[7] ~/.local/bin is already in PATH ✔");
            return Ok(());
        }
    }

    // Detect shell type and determine appropriate config file and export syntax
    let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    let (config_file, path_export) = if shell.contains("zsh") {
        let file = dirs::home_dir().unwrap().join(".zshrc");
        let export = "export PATH=\"$HOME/.local/bin:$PATH\"";
        (file, export)
    } else if shell.contains("fish") {
        let file = dirs::home_dir().unwrap().join(".config/fish/config.fish");
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent).ok();
        }
        let export = "set -gx PATH $HOME/.local/bin $PATH";
        (file, export)
    } else {
        // Default to bash
        let file = dirs::home_dir().unwrap().join(".bashrc");
        let export = "export PATH=\"$HOME/.local/bin:$PATH\"";
        (file, export)
    };

    // Avoid duplicate entries in config file
    if config_file.exists() {
        let contents = fs::read_to_string(&config_file)?;
        if contents.contains("$HOME/.local/bin")
            || contents.contains("~/.local/bin")
            || contents.contains(".local/bin")
        {
            println!(
                "[7] ~/.local/bin export found in {} ✔",
                config_file.display()
            );
            println!(
                "    Note: You may need to restart your terminal or run: source {}",
                config_file.display()
            );
            return Ok(());
        }
    }

    // Append PATH export to shell config file
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config_file)?;

    use std::io::Write;
    writeln!(file, "# Added by TarSmith installer")?;
    writeln!(file, "{}", path_export)?;

    println!(
        "[7] Added ~/.local/bin to PATH in {} ✔",
        config_file.display()
    );
    println!(
        "    Note: Restart your terminal or run: source {}",
        config_file.display()
    );

    Ok(())
}
