use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserCommand {
    pub id: String,
    pub name: String,
    pub description: String,
    pub exec: String,              // shell command, supports {path} {file} {dir} placeholders
    pub icon: String,              // icon name or emoji
    pub show_in_context_menu: bool,
    pub show_in_toolbar: bool,
    pub shortcut: Option<String>,  // e.g. "ctrl+shift+t"
    pub working_dir: Option<String>, // None = use current dir
    pub terminal: bool,            // open in terminal?
    pub category: String,          // "git", "tools", "media", etc.
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

fn commands_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("arch-explorer")
        .join("commands.toml")
}

fn default_commands() -> Vec<UserCommand> {
    vec![
        UserCommand {
            id: "open-terminal".into(),
            name: "Open Terminal Here".into(),
            description: "Open terminal in current directory".into(),
            exec: "kitty --directory {dir}".into(),
            icon: "terminal".into(),
            show_in_context_menu: true,
            show_in_toolbar: true,
            shortcut: Some("ctrl+t".into()),
            working_dir: None,
            terminal: false,
            category: "tools".into(),
        },
        UserCommand {
            id: "git-status".into(),
            name: "Git Status".into(),
            description: "Show git status in current directory".into(),
            exec: "git status".into(),
            icon: "git".into(),
            show_in_context_menu: true,
            show_in_toolbar: false,
            shortcut: None,
            working_dir: None,
            terminal: true,
            category: "git".into(),
        },
        UserCommand {
            id: "git-log".into(),
            name: "Git Log".into(),
            description: "Show git log".into(),
            exec: "git log --oneline --graph --color".into(),
            icon: "git".into(),
            show_in_context_menu: true,
            show_in_toolbar: false,
            shortcut: None,
            working_dir: None,
            terminal: true,
            category: "git".into(),
        },
        UserCommand {
            id: "copy-path".into(),
            name: "Copy Path".into(),
            description: "Copy file path to clipboard".into(),
            exec: "echo -n {path} | xclip -selection clipboard".into(),
            icon: "copy".into(),
            show_in_context_menu: true,
            show_in_toolbar: false,
            shortcut: Some("ctrl+shift+c".into()),
            working_dir: None,
            terminal: false,
            category: "tools".into(),
        },
        UserCommand {
            id: "compress-tar".into(),
            name: "Compress (tar.gz)".into(),
            description: "Compress selected files".into(),
            exec: "tar -czf {file}.tar.gz {file}".into(),
            icon: "archive".into(),
            show_in_context_menu: true,
            show_in_toolbar: false,
            shortcut: None,
            working_dir: None,
            terminal: true,
            category: "tools".into(),
        },
        UserCommand {
            id: "open-as-root".into(),
            name: "Open as Root".into(),
            description: "Open file manager as root in this directory".into(),
            exec: "pkexec arch-explorer {dir}".into(),
            icon: "shield".into(),
            show_in_context_menu: true,
            show_in_toolbar: false,
            shortcut: None,
            working_dir: None,
            terminal: false,
            category: "tools".into(),
        },
    ]
}

#[tauri::command]
pub fn load_user_commands() -> Vec<UserCommand> {
    let path = commands_path();
    if !path.exists() {
        let defaults = default_commands();
        let _ = save_commands_to_disk(&defaults);
        return defaults;
    }

    let data = fs::read_to_string(&path).unwrap_or_default();
    // Try TOML first, then JSON fallback
    toml::from_str::<CommandsFile>(&data)
        .map(|f| f.commands)
        .unwrap_or_else(|_| {
            serde_json::from_str::<Vec<UserCommand>>(&data)
                .unwrap_or_else(|_| default_commands())
        })
}

#[derive(Serialize, Deserialize)]
struct CommandsFile {
    commands: Vec<UserCommand>,
}

fn save_commands_to_disk(cmds: &[UserCommand]) -> Result<(), String> {
    let path = commands_path();
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;

    let file = CommandsFile { commands: cmds.to_vec() };
    let data = toml::to_string_pretty(&file).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_user_command(cmd: UserCommand) -> Result<(), String> {
    let mut cmds = load_user_commands();

    if let Some(existing) = cmds.iter_mut().find(|c| c.id == cmd.id) {
        *existing = cmd;
    } else {
        cmds.push(cmd);
    }

    save_commands_to_disk(&cmds)
}

#[tauri::command]
pub fn delete_user_command(id: String) -> Result<(), String> {
    let cmds: Vec<UserCommand> = load_user_commands()
        .into_iter()
        .filter(|c| c.id != id)
        .collect();
    save_commands_to_disk(&cmds)
}

/// Run a user command with path substitutions
/// {path}  = full path of selected file/dir
/// {file}  = filename only
/// {dir}   = parent directory
/// {name}  = filename without extension
#[tauri::command]
pub fn run_user_command(
    command_id: String,
    selected_path: Option<String>,
    current_dir: String,
) -> Result<CommandResult, String> {
    let cmds = load_user_commands();
    let cmd = cmds.iter()
        .find(|c| c.id == command_id)
        .ok_or_else(|| format!("Command not found: {}", command_id))?;

    let path = selected_path.clone().unwrap_or_else(|| current_dir.clone());
    let p = PathBuf::from(&path);

    let file_name = p.file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();

    let dir = if p.is_dir() {
        path.clone()
    } else {
        p.parent()
            .map(|d| d.to_string_lossy().to_string())
            .unwrap_or_else(|| current_dir.clone())
    };

    let name_no_ext = p.file_stem()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();

    // Substitute placeholders
    let exec = cmd.exec
        .replace("{path}", &path)
        .replace("{file}", &file_name)
        .replace("{dir}", &dir)
        .replace("{name}", &name_no_ext)
        .replace("{current_dir}", &current_dir);

    let working_dir = cmd.working_dir.clone().unwrap_or(dir.clone());

    if cmd.terminal {
        // Run in terminal (kitty, alacritty, xterm...)
        let terminal = detect_terminal();
        let out = Command::new(&terminal)
            .args(get_terminal_exec_args(&terminal, &exec))
            .current_dir(&working_dir)
            .output()
            .map_err(|e| format!("Terminal launch failed: {}", e))?;

        return Ok(CommandResult {
            success: out.status.success(),
            stdout: String::from_utf8_lossy(&out.stdout).to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).to_string(),
            exit_code: out.status.code().unwrap_or(-1),
        });
    }

    // Run directly via shell
    let out = Command::new("sh")
        .args(["-c", &exec])
        .current_dir(&working_dir)
        .output()
        .map_err(|e| format!("Command execution failed: {}", e))?;

    Ok(CommandResult {
        success: out.status.success(),
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        exit_code: out.status.code().unwrap_or(-1),
    })
}

fn detect_terminal() -> String {
    let terminals = ["kitty", "alacritty", "foot", "wezterm", "xterm", "xfce4-terminal"];
    for t in &terminals {
        if Command::new("which").arg(t).output()
            .map(|o| o.status.success()).unwrap_or(false)
        {
            return t.to_string();
        }
    }
    "xterm".to_string()
}

fn get_terminal_exec_args(terminal: &str, exec: &str) -> Vec<String> {
    match terminal {
        "kitty" => vec!["-e".to_string(), "sh".to_string(), "-c".to_string(),
                        format!("{}; read -p 'Press enter...'", exec)],
        "alacritty" => vec!["-e".to_string(), "sh".to_string(), "-c".to_string(),
                            format!("{}; read -p 'Press enter...'", exec)],
        "foot" => vec!["sh".to_string(), "-c".to_string(),
                       format!("{}; read -p 'Press enter...'", exec)],
        _ => vec!["-e".to_string(), "sh".to_string(), "-c".to_string(),
                  format!("{}; read -p 'Press enter...'", exec)],
    }
}

#[tauri::command]
pub fn open_external_url(url: String) -> Result<(), String> {
    open::that(&url).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn send_crash_report(title: String, body: String) -> Result<String, String> {
    // Optional automatic crash report sender via GitHub Issue or curl
    // If GitHub token or issue endpoint is configured, this can post or log directly
    let log_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("arch-explorer")
        .join("crash-reports");
    let _ = fs::create_dir_all(&log_dir);
    let now = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let report_file = log_dir.join(format!("crash_{}.log", now));
    let content = format!("Title: {}\nDate: {}\n\n{}", title, now, body);
    fs::write(&report_file, content).map_err(|e| e.to_string())?;

    // Also attempt to open pre-filled GitHub issue in browser
    let encoded_title = urlencoding(&title);
    let encoded_body = urlencoding(&body);
    let issue_url = format!(
        "https://github.com/basihamam/arch-explorer-tauri/issues/new?title={}&body={}",
        encoded_title, encoded_body
    );
    let _ = open::that(&issue_url);

    Ok(report_file.to_string_lossy().to_string())
}

fn urlencoding(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push_str("%20"),
            b'\n' => out.push_str("%0A"),
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}
