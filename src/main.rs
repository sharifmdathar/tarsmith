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
        eprintln!("       tarsmith --version");
        eprintln!("       tarsmith --help");
        std::process::exit(1);
    }

    let mut install_type: Option<bool> = None;
    let mut no_desktop = false;
    let mut no_path = false;
    let mut archive_path: Option<&str> = None;

    for arg in args.iter().skip(1) {
        match arg.as_str() {
            "--version" | "-V" => {
                println!("tarsmith {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                println!("TarSmith - A simple, interactive installer for tar archives");
                println!();
                println!("USAGE:");
                println!("    tarsmith <file.tar.gz> [OPTIONS]");
                println!();
                println!("OPTIONS:");
                println!("    -s, --system      Install system-wide (/opt)");
                println!("    -u, --user        Install user-level (~/.local/tarsmith) [default]");
                println!("    -nd, --no-desktop Skip desktop entry creation");
                println!("    -np, --no-path    Skip adding executables to PATH");
                println!("    -h, --help        Print help information");
                println!("    -V, --version     Print version information");
                println!();
                println!("EXAMPLES:");
                println!("    tarsmith node-v20.0.0-linux-x64.tar.gz");
                println!("    tarsmith android-studio.tar.gz --user");
                println!("    tarsmith app.tar.gz --system --no-desktop");
                return Ok(());
            }
            "--system" | "-s" => {
                if install_type.is_some() {
                    eprintln!("Error: Cannot specify both --system/-s and --user/-u");
                    std::process::exit(1);
                }
                install_type = Some(false);
            }
            "--user" | "-u" => {
                if install_type.is_some() {
                    eprintln!("Error: Cannot specify both --system/-s and --user/-u");
                    std::process::exit(1);
                }
                install_type = Some(true);
            }
            "--no-desktop" | "-nd" => {
                no_desktop = true;
            }
            "--no-path" | "-np" => {
                no_path = true;
            }
            _ => {
                if archive_path.is_some() {
                    eprintln!("Error: Multiple archive files specified");
                    std::process::exit(1);
                }
                if !arg.starts_with('-') {
                    archive_path = Some(arg);
                } else {
                    eprintln!("Error: Unknown option: {}", arg);
                    std::process::exit(1);
                }
            }
        }
    }

    let archive_path = match archive_path {
        Some(path) => Path::new(path),
        None => {
            eprintln!("Error: No archive file specified");
            eprintln!("Usage: tarsmith <file.tar.gz> [OPTIONS]");
            std::process::exit(1);
        }
    };

    println!("=== TarSmith Installer ===");
    println!("Input file: {}", archive_path.display());
    println!();

    if !archive_path.exists() {
        return Err(format!("Archive not found: {}", archive_path.display()).into());
    }
    println!("[1] File exists ✔");

    let (install_dir, is_user_level) = if let Some(user_level) = install_type {
        if !user_level {
            if !check_sudo_permissions() {
                eprintln!("Error: System-wide installation requires sudo privileges.");
                eprintln!(
                    "Please run with: sudo tarsmith {} --system",
                    archive_path.display()
                );
                eprintln!("Or use --user for user-level installation which doesn't require sudo.");
                std::process::exit(1);
            }
            (Path::new("/opt").to_path_buf(), false)
        } else {
            (dirs::home_dir().unwrap().join(".local/tarsmith"), true)
        }
    } else {
        println!("Choose installation type:");
        println!("1) User-level (~/.local/tarsmith) [default]");
        println!("2) System-wide (/opt)");
        print!("Enter 1 or 2 (default: 1): ");
        io::stdout().flush()?;

        let mut choice = String::new();
        io::stdin().read_line(&mut choice)?;
        let choice = choice.trim();

        if choice == "2" {
            if !check_sudo_permissions() {
                eprintln!("Error: System-wide installation requires sudo privileges.");
                eprintln!("Please run with: sudo tarsmith {}", archive_path.display());
                eprintln!(
                    "Or choose user-level installation (option 1) which doesn't require sudo."
                );
                std::process::exit(1);
            }
            (Path::new("/opt").to_path_buf(), false)
        } else {
            (dirs::home_dir().unwrap().join(".local/tarsmith"), true)
        }
    };

    if !install_dir.exists() {
        println!("[2] Creating install directory: {}", install_dir.display());
        fs::create_dir_all(&install_dir)?;
    } else {
        println!("[2] Install directory exists: {}", install_dir.display());
    }

    println!("[2] Install directory ready ✔");

    println!("[3] Extracting archive...");

    let temp_dir = install_dir.join(".tarsmith_temp_extract");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;

    let tar_flags = archive_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            if ext == "gz" || ext == "tgz" {
                "-xzf"
            } else if ext == "xz" || ext == "txz" {
                "-xJf"
            } else if ext == "bz2" {
                "-xjf"
            } else if ext == "zst" {
                "--zstd -xf"
            } else {
                "-xf"
            }
        })
        .unwrap_or("-xf");

    let mut cmd = Command::new("tar");

    if tar_flags.contains("zstd") {
        cmd.args(["--zstd", "-xf", archive_path.to_str().unwrap()]);
    } else {
        cmd.arg(tar_flags);
        cmd.arg(archive_path);
    }

    cmd.arg("-C").arg(&temp_dir);

    let status = cmd.status()?;

    if !status.success() {
        fs::remove_dir_all(&temp_dir).ok();
        return Err("Extraction failed".into());
    }
    println!("[3] Extraction complete ✔");

    println!("[4] Detecting installation folder...");

    let extracted_path = analyze_and_move_extraction(&temp_dir, &install_dir, archive_path)
        .map_err(|e| {
            fs::remove_dir_all(&temp_dir).ok();
            format!("Failed to analyze extraction: {}", e)
        })?;

    fs::remove_dir_all(&temp_dir).ok();
    println!(
        "[4] Detected installation directory: {} ✔",
        extracted_path.display()
    );

    let app_name = infer_app_name(&extracted_path)?;
    println!("[4] Inferred app name: {} ✔", app_name);

    let exec_path = extracted_path.join("bin");
    let executables = if exec_path.exists() && exec_path.is_dir() {
        find_executables_in_bin(&exec_path)?
    } else {
        find_executables_in_bin(&extracted_path)?
    };

    let desktop_exec = if no_desktop {
        None
    } else if install_type.is_some() {
        if executables.is_empty() {
            None
        } else {
            println!(
                "[5] Using first executable for desktop entry: {}",
                executables[0]
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            );
            Some(executables[0].clone())
        }
    } else {
        println!("[5] Select executable for desktop entry (GUI launch):");
        if executables.len() == 1 {
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
        }
    };

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

        if let Some(parent) = desktop_path.parent() {
            fs::create_dir_all(parent)?;
        }

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

    let selected_for_path = if no_path {
        Vec::new()
    } else if install_type.is_some() {
        executables.clone()
    } else {
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

            if response == "n" || response == "no" {
                Vec::new()
            } else {
                vec![executables[0].clone()]
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
                "  Enter numbers separated by spaces (e.g., 1 2 3) or 'all' for all [default: all]: "
            );
            io::stdout().flush()?;

            let mut selection = String::new();
            io::stdin().read_line(&mut selection)?;
            let selection = selection.trim().to_lowercase();

            if selection.is_empty() || selection == "all" {
                executables.clone()
            } else {
                let indices: Vec<usize> = selection
                    .split_whitespace()
                    .map(|s| {
                        s.parse::<usize>()
                            .map_err(|_| "Invalid number format".to_string())
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                let mut selected = Vec::new();
                for idx in indices {
                    if idx < 1 || idx > executables.len() {
                        return Err(format!("Invalid selection: {}", idx).into());
                    }
                    selected.push(executables[idx - 1].clone());
                }
                selected
            }
        }
    };

    if selected_for_path.is_empty() {
        println!("[7] Skipped adding to PATH ✔");
    } else {
        if install_type.is_some() {
            println!("[7] Adding all executables to PATH...");
        }
        create_path_symlinks(&selected_for_path, is_user_level)?;
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
    let test_file = opt_path.join(".tarsmith_test_write_permissions");

    match fs::write(&test_file, "test") {
        Ok(_) => {
            fs::remove_file(&test_file).ok();
            true
        }
        Err(_) => {
            let sudo_check = Command::new("sudo").arg("-n").arg("true").output();

            if let Ok(output) = sudo_check {
                output.status.success()
            } else {
                false
            }
        }
    }
}

/// Extracts a clean directory name from archive stem, removing version/platform suffixes
fn extract_dir_name_from_stem(stem: &str) -> String {
    stem.split(&['-', '_'][..])
        .take_while(|p| {
            !p.chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
        })
        .collect::<Vec<_>>()
        .join("-")
}

/// Removes existing target path if it exists (handles both files and directories)
fn remove_existing_target(target_path: &Path) -> Result<(), Box<dyn Error>> {
    if target_path.exists() {
        match fs::metadata(target_path) {
            Ok(metadata) => {
                if metadata.is_dir() {
                    fs::remove_dir_all(target_path)?;
                } else {
                    fs::remove_file(target_path)?;
                }
            }
            Err(_) => {
                fs::remove_file(target_path).ok();
                fs::remove_dir_all(target_path).ok();
            }
        }
    }
    Ok(())
}

/// Analyzes the temporary extraction and moves it to the final location
/// Handles both cases: single directory extracted OR files extracted directly
fn analyze_and_move_extraction(
    temp_dir: &Path,
    install_dir: &Path,
    archive: &Path,
) -> Result<PathBuf, Box<dyn Error>> {
    let entries: Vec<_> = fs::read_dir(temp_dir)?.collect::<Result<_, _>>()?;

    if entries.is_empty() {
        return Err("Archive appears to be empty".into());
    }

    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();

    for entry in &entries {
        let path = entry.path();
        match fs::metadata(&path) {
            Ok(metadata) => {
                if metadata.is_dir() {
                    dirs.push(path);
                } else if metadata.is_file() {
                    files.push(path);
                }
            }
            Err(_) => continue,
        }
    }

    let final_path = if dirs.len() == 1 && files.is_empty() {
        let extracted_dir = &dirs[0];
        let dir_name = extracted_dir
            .file_name()
            .ok_or("Cannot get directory name")?
            .to_string_lossy()
            .to_string();
        let target_path = install_dir.join(&dir_name);
        remove_existing_target(&target_path)?;
        fs::rename(extracted_dir, &target_path)?;
        target_path
    } else if dirs.is_empty() && !files.is_empty() {
        let stem = archive
            .file_stem()
            .ok_or("Cannot find archive name")?
            .to_string_lossy()
            .replace(".tar", "");

        let dir_name = extract_dir_name_from_stem(&stem);
        let target_path = if dir_name.is_empty() {
            install_dir.join(&stem)
        } else {
            install_dir.join(&dir_name)
        };

        remove_existing_target(&target_path)?;
        fs::create_dir_all(&target_path)?;

        for file_path in &files {
            let file_name = file_path.file_name().ok_or("Cannot get file name")?;
            let dest = target_path.join(file_name);
            fs::rename(file_path, &dest)?;
        }

        target_path
    } else {
        let stem = archive
            .file_stem()
            .ok_or("Cannot find archive name")?
            .to_string_lossy()
            .replace(".tar", "");

        let dir_name = extract_dir_name_from_stem(&stem);
        let target_path = if dir_name.is_empty() {
            install_dir.join(&stem)
        } else {
            install_dir.join(&dir_name)
        };

        remove_existing_target(&target_path)?;
        fs::create_dir_all(&target_path)?;

        for dir_path in &dirs {
            let dir_name = dir_path.file_name().ok_or("Cannot get directory name")?;
            let dest = target_path.join(dir_name);
            fs::rename(dir_path, &dest)?;
        }
        for file_path in &files {
            let file_name = file_path.file_name().ok_or("Cannot get file name")?;
            let dest = target_path.join(file_name);
            fs::rename(file_path, &dest)?;
        }

        target_path
    };

    Ok(final_path)
}

/// Finds all executable files in a directory (bin/ or root) by checking file permissions
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

    common_icon_paths
        .into_iter()
        .find(|icon_path| icon_path.exists())
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
        let file = dirs::home_dir().unwrap().join(".bashrc");
        let export = "export PATH=\"$HOME/.local/bin:$PATH\"";
        (file, export)
    };

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
