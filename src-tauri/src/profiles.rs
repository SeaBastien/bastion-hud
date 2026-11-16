//! Shell profiles: the fixed list of programs the terminal may start.

use serde::Serialize;
use std::env;
use std::path::PathBuf;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub label: String,
    // The command line stays in the backend; the page never sees or sets it.
    #[serde(skip)]
    pub program: String,
    #[serde(skip)]
    pub args: Vec<String>,
}

/// Returns true when `name` is an executable file somewhere on PATH.
fn on_path(name: &str) -> bool {
    let Some(paths) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&paths).any(|dir: PathBuf| dir.join(name).is_file())
}

pub fn all() -> Vec<Profile> {
    let mut profiles = Vec::new();

    if cfg!(windows) {
        profiles.push(Profile {
            id: "shell".into(),
            label: "PowerShell".into(),
            program: "powershell.exe".into(),
            args: vec!["-NoLogo".into()],
        });
        if on_path("wsl.exe") {
            profiles.push(Profile {
                id: "kali".into(),
                label: "Kali".into(),
                program: "wsl.exe".into(),
                args: vec!["-d".into(), "kali-linux".into()],
            });
        }
    } else {
        let fallback = if cfg!(target_os = "macos") {
            "/bin/zsh"
        } else {
            "/bin/bash"
        };
        let shell = env::var("SHELL").unwrap_or_else(|_| fallback.to_string());
        profiles.push(Profile {
            id: "shell".into(),
            label: "Shell".into(),
            program: shell,
            args: Vec::new(),
        });
        if on_path("distrobox") {
            profiles.push(Profile {
                id: "kali".into(),
                label: "Kali".into(),
                program: "distrobox".into(),
                args: vec!["enter".into(), "kali".into()],
            });
        }
    }

    profiles
}

pub fn find(id: &str) -> Option<Profile> {
    all().into_iter().find(|profile| profile.id == id)
}

#[tauri::command]
pub fn list_profiles() -> Vec<Profile> {
    all()
}
