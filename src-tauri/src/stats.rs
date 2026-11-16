//! System statistics, pushed to the window every 1.5 seconds.

use serde::Serialize;
use std::collections::HashSet;
use std::env;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::{Disks, Networks, System};
use tauri::ipc::Channel;
use tauri::State;

const INTERVAL: Duration = Duration::from_millis(1500);
const DISK_EVERY: u32 = 10; // re-read disks every 10 ticks (15 s)
const TOP_PROCESSES: usize = 8;
const MAX_DISKS: usize = 5;

/// Each subscription gets a generation number. A newer subscription (for
/// example after the window reloads) makes the older thread stop.
#[derive(Default)]
pub struct StatsState {
    generation: Arc<AtomicU64>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskInfo {
    name: String,
    mount: String,
    total: u64,
    available: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcInfo {
    pid: u32,
    name: String,
    cpu: f32,
    mem: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    cpu_global: f32,
    cpus: Vec<f32>,
    mem_used: u64,
    mem_total: u64,
    swap_used: u64,
    swap_total: u64,
    uptime: u64,
    net_rx: u64,
    net_tx: u64,
    disks: Vec<DiskInfo>,
    procs: Vec<ProcInfo>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    host: String,
    user: String,
    os: String,
    kernel: String,
    cpu_brand: String,
    cores: usize,
}

/// Total bytes received and sent across all non-loopback interfaces.
fn net_totals() -> (u64, u64) {
    let networks = Networks::new_with_refreshed_list();
    let mut rx = 0u64;
    let mut tx = 0u64;
    for (name, data) in &networks {
        if name.to_lowercase().starts_with("lo") {
            continue;
        }
        rx = rx.saturating_add(data.total_received());
        tx = tx.saturating_add(data.total_transmitted());
    }
    (rx, tx)
}

fn read_disks() -> Vec<DiskInfo> {
    let disks = Disks::new_with_refreshed_list();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for disk in &disks {
        let name = disk.name().to_string_lossy().into_owned();
        // Image-based systems mount one device many times; show each device once.
        if disk.total_space() == 0 || !seen.insert(name.clone()) {
            continue;
        }
        out.push(DiskInfo {
            name,
            mount: disk.mount_point().to_string_lossy().into_owned(),
            total: disk.total_space(),
            available: disk.available_space(),
        });
        if out.len() == MAX_DISKS {
            break;
        }
    }
    out
}

fn top_processes(sys: &System) -> Vec<ProcInfo> {
    // sysinfo reports per-core percentages (100% = one full core). Divide by
    // the core count so the figure matches the CPU panel's whole-machine scale.
    let cores = sys.cpus().len().max(1) as f32;
    let mut procs: Vec<ProcInfo> = sys
        .processes()
        .iter()
        .map(|(pid, process)| ProcInfo {
            pid: pid.as_u32(),
            name: process.name().to_string_lossy().into_owned(),
            cpu: process.cpu_usage() / cores,
            mem: process.memory(),
        })
        .collect();
    procs.sort_by(|a, b| {
        b.cpu
            .partial_cmp(&a.cpu)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    procs.truncate(TOP_PROCESSES);
    procs
}

#[tauri::command]
pub fn stats_subscribe(state: State<'_, StatsState>, on_event: Channel<Snapshot>) {
    let generation = state.generation.clone();
    let mine = generation.fetch_add(1, Ordering::SeqCst) + 1;

    thread::spawn(move || {
        let mut sys = System::new_all();
        let mut last_net = net_totals();
        let mut last_time = Instant::now();
        let mut disks = read_disks();
        let mut tick = 0u32;

        loop {
            thread::sleep(INTERVAL);
            if generation.load(Ordering::SeqCst) != mine {
                break;
            }

            sys.refresh_all();

            let now = Instant::now();
            let elapsed = now.duration_since(last_time).as_secs_f64().max(0.001);
            let net = net_totals();
            let net_rx = (net.0.saturating_sub(last_net.0) as f64 / elapsed) as u64;
            let net_tx = (net.1.saturating_sub(last_net.1) as f64 / elapsed) as u64;
            last_net = net;
            last_time = now;

            tick += 1;
            if tick % DISK_EVERY == 0 {
                disks = read_disks();
            }

            let snapshot = Snapshot {
                cpu_global: sys.global_cpu_usage(),
                cpus: sys.cpus().iter().map(|cpu| cpu.cpu_usage()).collect(),
                mem_used: sys.used_memory(),
                mem_total: sys.total_memory(),
                swap_used: sys.used_swap(),
                swap_total: sys.total_swap(),
                uptime: System::uptime(),
                net_rx,
                net_tx,
                disks: disks.clone(),
                procs: top_processes(&sys),
            };

            if on_event.send(snapshot).is_err() {
                break;
            }
        }
    });
}

// Runs off the main thread: reading hardware details takes a moment.
#[tauri::command(async)]
pub fn system_info() -> SystemInfo {
    let sys = System::new_all();
    let os = match (System::name(), System::os_version()) {
        (Some(name), Some(version)) => format!("{name} {version}"),
        (Some(name), None) => name,
        _ => "unknown".to_string(),
    };
    SystemInfo {
        host: System::host_name().unwrap_or_else(|| "unknown".to_string()),
        user: env::var("USER")
            .or_else(|_| env::var("USERNAME"))
            .unwrap_or_else(|_| "unknown".to_string()),
        os,
        kernel: System::kernel_version().unwrap_or_else(|| "unknown".to_string()),
        cpu_brand: sys
            .cpus()
            .first()
            .map(|cpu| cpu.brand().trim().to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        cores: sys.cpus().len(),
    }
}
