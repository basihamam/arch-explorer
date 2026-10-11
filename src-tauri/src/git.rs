// git.rs — git status --porcelain parse + repo tespiti
// Sadece git repo içindeyken çalışır, dışarıda boş map döner
// Dosya adı → status kodu: "M" modified, "A" added, "D" deleted, "?" untracked, "R" renamed

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Serialize, Deserialize)]
pub struct GitStatus {
    pub is_repo:  bool,
    pub branch:   String,
    pub files:    HashMap<String, String>, // relative path → status char
    pub counts:   GitCounts,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GitCounts {
    pub modified:  u32,
    pub added:     u32,
    pub deleted:   u32,
    pub untracked: u32,
    pub renamed:   u32,
}

/// Verilen dizinin git root'unu bul
fn git_root(path: &str) -> Option<String> {
    let out = Command::new("git")
        .args(["-C", path, "rev-parse", "--show-toplevel"])
        .output().ok()?;
    if !out.status.success() { return None; }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Aktif branch adını al
fn git_branch(path: &str) -> String {
    Command::new("git")
        .args(["-C", path, "branch", "--show-current"])
        .output()
        .ok()
        .and_then(|o| if o.status.success() { Some(o) } else { None })
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "HEAD".to_string())
}

#[tauri::command]
pub async fn get_git_status(path: String) -> GitStatus {
    let p_clone = path.clone();
    let task = tokio::task::spawn_blocking(move || {
        // Git repo mu kontrol et
        let root = match git_root(&p_clone) {
            Some(r) => r,
            None    => return GitStatus {
                is_repo: false,
                branch:  String::new(),
                files:   HashMap::new(),
                counts:  GitCounts::default(),
            },
        };

        let branch = git_branch(&p_clone);

        // git status --porcelain --untracked-files=normal (büyük untracked klasörlerde kilitlenmeyi önler)
        let out = match Command::new("git")
            .args(["-C", &p_clone, "status", "--porcelain", "--untracked-files=normal"])
            .output()
        {
            Ok(o) => o,
            Err(_) => return GitStatus {
                is_repo: true, branch,
                files: HashMap::new(), counts: GitCounts::default(),
            },
        };

        let text = String::from_utf8_lossy(&out.stdout);
        let mut files  = HashMap::new();
        let mut counts = GitCounts::default();

        let root_path = PathBuf::from(&root);

        for line in text.lines() {
            if line.len() < 4 { continue; }

            let xy       = &line[..2];
            let raw_path = line[3..].trim();

            // Rename: "R  old -> new" veya "R old\0new"
            let file_path = if xy.starts_with('R') || xy.starts_with('C') {
                raw_path.split(" -> ").last()
                    .unwrap_or(raw_path)
                    .trim_matches('"')
                    .to_string()
            } else {
                raw_path.trim_matches('"').to_string()
            };

            // Status karakterini belirle — öncelik: staged > unstaged > untracked
            let status_char = if &xy[..1] == "?" {
                counts.untracked += 1;
                "?"
            } else if &xy[..1] != " " {
                let c = &xy[..1];
                match c {
                    "M" => { counts.modified += 1; "M" }
                    "A" => { counts.added    += 1; "A" }
                    "D" => { counts.deleted  += 1; "D" }
                    "R" => { counts.renamed  += 1; "R" }
                    _   => { "M" }
                }
            } else {
                let c = &xy[1..2];
                match c {
                    "M" => { counts.modified += 1; "M" }
                    "D" => { counts.deleted  += 1; "D" }
                    _   => { "M" }
                }
            };

            let abs_path = root_path.join(&file_path)
                .to_string_lossy()
                .to_string();

            let filename = PathBuf::from(&file_path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            files.insert(abs_path, status_char.to_string());
            files.insert(filename, status_char.to_string());
        }

        GitStatus { is_repo: true, branch, files, counts }
    });

    match tokio::time::timeout(std::time::Duration::from_millis(2000), task).await {
        Ok(Ok(res)) => res,
        _ => GitStatus {
            is_repo: false,
            branch: String::new(),
            files: HashMap::new(),
            counts: GitCounts::default(),
        },
    }
}
