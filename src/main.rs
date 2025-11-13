// Full version of TarSmith installer
// This performs real actions: file checks, extraction, folder detection, desktop entry creation.
// Note: Requires running with sudo for installing into /opt and writing desktop files.

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
        (Path::new("/opt").to_path_buf(), false)
    } else {
        // Default to user-level (option 1)
        (dirs::home_dir().unwrap().join(".local/tarsmith"), true)
    };

    if !install_dir.exists() {
        println!("[2] Creating install directory: {}", install_dir.display());
        fs::create_dir_all(&install_dir)?;
    } else {
        println!("[2] Install directory exists: {}", install_dir.display());
    }

    println!("[2] Install directory ready ✔");

    // Step 3: Extracting archive
    println!("[3] Extracting archive...");

    // Detect compression format from file extension
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

    // Handle zstd separately as it needs multiple args
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

    // Step 4: Try to detect extracted directory
    println!("[4] Detecting installation folder...");

    let extracted_path = detect_extracted_folder(&install_dir, archive_path)?;
    println!(
        "[4] Detected installation directory: {} ✔",
        extracted_path.display()
    );

    // Infer app name from extracted folder
    let app_name = infer_app_name(&extracted_path)?;
    println!("[4] Inferred app name: {} ✔", app_name);

    // Step 5: Find executable
    println!("[5] Finding executable...");
    let exec_path = extracted_path.join("bin");
    let executables = find_executables_in_bin(&exec_path)?;

    let exec_file = if executables.len() == 1 {
        executables[0].clone()
    } else {
        println!("Multiple executables found:");
        for (i, exe) in executables.iter().enumerate() {
            println!(
                "  {}) {}",
                i + 1,
                exe.file_name().unwrap_or_default().to_string_lossy()
            );
        }
        print!("Select executable (1-{}): ", executables.len());
        io::stdout().flush()?;

        let mut selection = String::new();
        io::stdin().read_line(&mut selection)?;
        let selection: usize = selection.trim().parse().map_err(|_| "Invalid selection")?;

        if selection < 1 || selection > executables.len() {
            return Err("Invalid selection".into());
        }

        executables[selection - 1].clone()
    };
    println!(
        "[5] Selected executable: {} ✔",
        exec_file.file_name().unwrap_or_default().to_string_lossy()
    );

    // Step 6: Create desktop entry
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
    let icon_path =
        find_icon(&extracted_path).unwrap_or_else(|| extracted_path.join("bin").join("icon.png"));

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

    // Step 7: Optionally add to PATH
    println!("[7] Add to PATH?");
    print!("This will create a symlink allowing you to launch from terminal (Y/n): ");
    io::stdout().flush()?;

    let mut add_to_path = String::new();
    io::stdin().read_line(&mut add_to_path)?;
    let add_to_path = add_to_path.trim().to_lowercase();

    // Default to yes (add to PATH) unless explicitly "n" or "no"
    if add_to_path == "n" || add_to_path == "no" {
        println!("[7] Skipped adding to PATH ✔");
    } else {
        let bin_dir = if is_user_level {
            dirs::home_dir().unwrap().join(".local/bin")
        } else {
            Path::new("/usr/local/bin").to_path_buf()
        };

        // Ensure bin directory exists
        if !bin_dir.exists() {
            fs::create_dir_all(&bin_dir)?;
        }

        // Use the executable name or app name as the symlink name
        let symlink_name = exec_file
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let symlink_path = bin_dir.join(&symlink_name);

        // Remove existing symlink if it exists
        if symlink_path.exists() || symlink_path.is_symlink() {
            fs::remove_file(&symlink_path).ok();
        }

        // Create symlink
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(&exec_file, &symlink_path)?;
        }

        println!(
            "[7] Symlink created: {} -> {} ✔",
            symlink_path.display(),
            exec_file.display()
        );

        // For user-level installations, ensure ~/.local/bin is in PATH
        if is_user_level {
            ensure_local_bin_in_path()?;
        }

        println!("    You can now run '{}' from your terminal", symlink_name);
    }

    println!(
        "
Installation complete! 🎉"
    );
    println!("Installed to: {}", extracted_path.display());
    println!("Launch using the system menu or: {}", exec_file.display());

    Ok(())
}

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

    // Get first part of stem, handling both hyphens and underscores
    let stem_first_part = stem
        .split(&['-', '_'][..])
        .next()
        .unwrap_or(&stem)
        .to_string();

    // Try variations with both hyphens and underscores
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
        // Sort by modification time (newest first)
        candidates_with_bin.sort_by(|a, b| b.1.cmp(&a.1));

        // Try to find one that matches the archive name (prefer newest matching)
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

        // If no name match, return the newest directory (most recently extracted)
        return Ok(candidates_with_bin[0].0.clone());
    }

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

fn infer_app_name(extracted_path: &Path) -> Result<String, Box<dyn Error>> {
    let folder_name = extracted_path
        .file_name()
        .ok_or("Cannot get folder name")?
        .to_string_lossy();

    let name = folder_name
        .split('-')
        .take_while(|part| {
            // Stop at version-like patterns (numbers, "linux", "x64", "amd64", etc.)
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

fn ensure_local_bin_in_path() -> Result<(), Box<dyn Error>> {
    let local_bin = dirs::home_dir()
        .ok_or("Cannot determine home directory")?
        .join(".local/bin");

    let local_bin_str = local_bin.to_string_lossy().to_string();

    // Check if already in PATH (check each PATH component)
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

    // Detect shell and config file
    let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    let (config_file, path_export) = if shell.contains("zsh") {
        let file = dirs::home_dir().unwrap().join(".zshrc");
        let export = "export PATH=\"$HOME/.local/bin:$PATH\"";
        (file, export)
    } else if shell.contains("fish") {
        let file = dirs::home_dir().unwrap().join(".config/fish/config.fish");
        // Ensure fish config directory exists
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

    // Check if already added to config file
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

    // Append to config file
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
