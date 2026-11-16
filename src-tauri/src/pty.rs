//! Pseudo-terminal sessions, streamed to the window over a Tauri channel.

use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::collections::HashMap;
use std::env;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::profiles;

struct Session {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
}

#[derive(Default)]
pub struct PtyState {
    sessions: Mutex<HashMap<u32, Session>>,
    next_id: AtomicU32,
}

#[derive(Clone, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "event",
    content = "data"
)]
pub enum PtyEvent {
    Output { text: String },
    Exit { code: u32 },
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        rows: rows.max(1),
        cols: cols.max(2),
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// Takes the longest valid UTF-8 prefix out of `pending`. A character split
/// across two reads stays in `pending` until its remaining bytes arrive.
fn take_valid_utf8(pending: &mut Vec<u8>) -> String {
    match std::str::from_utf8(pending) {
        Ok(text) => {
            let text = text.to_owned();
            pending.clear();
            text
        }
        Err(err) if err.error_len().is_none() => {
            let valid = err.valid_up_to();
            let text = String::from_utf8_lossy(&pending[..valid]).into_owned();
            pending.drain(..valid);
            text
        }
        Err(_) => {
            let text = String::from_utf8_lossy(pending).into_owned();
            pending.clear();
            text
        }
    }
}

#[tauri::command]
pub fn pty_spawn(
    app: AppHandle,
    state: State<'_, PtyState>,
    profile: String,
    cols: u16,
    rows: u16,
    on_event: Channel<PtyEvent>,
) -> Result<u32, String> {
    let profile =
        profiles::find(&profile).ok_or_else(|| format!("unknown shell profile: {profile}"))?;

    let pair = native_pty_system()
        .openpty(size(cols, rows))
        .map_err(|err| err.to_string())?;

    let mut cmd = CommandBuilder::new(&profile.program);
    cmd.args(&profile.args);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    if let Some(home) = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE")) {
        cmd.cwd(home);
    }

    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|err| err.to_string())?;
    // The child owns the slave side now. Dropping ours lets reads end when it exits.
    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|err| err.to_string())?;
    let writer = pair.master.take_writer().map_err(|err| err.to_string())?;
    let killer = child.clone_killer();

    let id = state.next_id.fetch_add(1, Ordering::SeqCst) + 1;
    state
        .sessions
        .lock()
        .map_err(|err| err.to_string())?
        .insert(
            id,
            Session {
                master: pair.master,
                writer,
                killer,
            },
        );

    // Reader thread: shell output -> window.
    let output = on_event.clone();
    thread::spawn(move || {
        let mut buf = [0u8; 8192];
        let mut pending: Vec<u8> = Vec::new();
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    pending.extend_from_slice(&buf[..n]);
                    let text = take_valid_utf8(&mut pending);
                    if !text.is_empty() && output.send(PtyEvent::Output { text }).is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Waiter thread: reports the exit code and forgets the session.
    thread::spawn(move || {
        let code = child.wait().map(|status| status.exit_code()).unwrap_or(1);
        // Give the reader a moment to flush the last output first.
        thread::sleep(Duration::from_millis(100));
        let _ = on_event.send(PtyEvent::Exit { code });
        if let Ok(mut sessions) = app.state::<PtyState>().sessions.lock() {
            sessions.remove(&id);
        }
    });

    Ok(id)
}

#[tauri::command]
pub fn pty_write(state: State<'_, PtyState>, id: u32, data: String) -> Result<(), String> {
    let mut sessions = state.sessions.lock().map_err(|err| err.to_string())?;
    let session = sessions
        .get_mut(&id)
        .ok_or_else(|| "terminal session not found".to_string())?;
    session
        .writer
        .write_all(data.as_bytes())
        .and_then(|_| session.writer.flush())
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn pty_resize(state: State<'_, PtyState>, id: u32, cols: u16, rows: u16) -> Result<(), String> {
    let sessions = state.sessions.lock().map_err(|err| err.to_string())?;
    let session = sessions
        .get(&id)
        .ok_or_else(|| "terminal session not found".to_string())?;
    session
        .master
        .resize(size(cols, rows))
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn pty_kill(state: State<'_, PtyState>, id: u32) -> Result<(), String> {
    let session = state
        .sessions
        .lock()
        .map_err(|err| err.to_string())?
        .remove(&id);
    if let Some(mut session) = session {
        // Already-exited processes return an error here; that is fine.
        let _ = session.killer.kill();
    }
    Ok(())
}
