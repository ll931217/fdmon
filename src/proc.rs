use anyhow::{Context, Result};
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub owner: String,
    pub uid: u32,
    pub command: String,
    pub fd_count: usize,
    pub fd_soft_limit: Option<u64>,
    pub cwd: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FdType {
    Socket,
    Pipe,
    File,
    Device,
    Other,
}

impl std::fmt::Display for FdType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FdType::Socket => write!(f, "socket"),
            FdType::Pipe => write!(f, "pipe"),
            FdType::File => write!(f, "file"),
            FdType::Device => write!(f, "device"),
            FdType::Other => write!(f, "other"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FdDetail {
    pub fd_num: u32,
    pub target: String,
    pub fd_type: FdType,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemStats {
    pub allocated_fds: u64,
    pub max_fds: u64,
    pub user_fds: u64,
    pub user_soft_limit: u64,
    pub user_hard_limit: u64,
    pub username: String,
}

/// Scans /proc for all processes and returns a Vec of ProcessInfo
pub fn scan_processes(current_uid: u32) -> Result<Vec<ProcessInfo>> {
    let proc_dir = fs::read_dir("/proc").context("Failed to read /proc")?;
    let mut processes = Vec::new();

    for entry in proc_dir.flatten() {
        let file_name = entry.file_name();
        let pid_str = file_name.to_string_lossy();

        // Only process numeric directories
        if let Ok(pid) = pid_str.parse::<u32>() {
            if let Ok(process_info) = read_process_info(pid, current_uid) {
                processes.push(process_info);
            }
        }
    }

    Ok(processes)
}

/// Reads information about a single process
fn read_process_info(pid: u32, _current_uid: u32) -> Result<ProcessInfo> {
    let proc_path = PathBuf::from(format!("/proc/{}", pid));

    // Read PPID from /proc/{pid}/stat
    let ppid = read_ppid(&proc_path)?;

    // Read UID and convert to username
    let uid = read_uid(&proc_path)?;
    let owner = uid_to_username(uid);

    // Read command from /proc/{pid}/cmdline
    let command = read_command(&proc_path)?;

    // Count file descriptors
    let fd_count = count_fds(&proc_path);

    // Read soft limit
    let fd_soft_limit = read_soft_limit(&proc_path);

    // Read CWD
    let cwd = read_cwd(&proc_path);

    Ok(ProcessInfo {
        pid,
        ppid,
        owner,
        uid,
        command,
        fd_count,
        fd_soft_limit,
        cwd,
    })
}

/// Reads PPID from /proc/{pid}/stat (field 4)
fn read_ppid(proc_path: &PathBuf) -> Result<u32> {
    let stat_path = proc_path.join("stat");
    let content = fs::read_to_string(stat_path).context("Failed to read stat")?;

    // Parse: pid (comm) state ppid ...
    // We need to handle command names with spaces/parens
    let closing_paren = content.rfind(')').context("Invalid stat format")?;
    let after_comm = &content[closing_paren + 1..];
    let fields: Vec<&str> = after_comm.split_whitespace().collect();

    // state is fields[0], ppid is fields[1]
    fields
        .get(1)
        .and_then(|s| s.parse().ok())
        .context("Failed to parse ppid")
}

/// Reads UID from /proc/{pid}/status
fn read_uid(proc_path: &PathBuf) -> Result<u32> {
    let status_path = proc_path.join("status");
    let content = fs::read_to_string(status_path).context("Failed to read status")?;

    for line in content.lines() {
        if line.starts_with("Uid:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(uid_str) = parts.get(1) {
                return uid_str.parse().context("Failed to parse UID");
            }
        }
    }

    anyhow::bail!("UID not found in status")
}

/// Converts UID to username using uzers crate
fn uid_to_username(uid: u32) -> String {
    uzers::get_user_by_uid(uid)
        .map(|u| u.name().to_string_lossy().to_string())
        .unwrap_or_else(|| uid.to_string())
}

/// Reads command from /proc/{pid}/cmdline
fn read_command(proc_path: &PathBuf) -> Result<String> {
    let cmdline_path = proc_path.join("cmdline");
    let content = fs::read(cmdline_path).context("Failed to read cmdline")?;

    if content.is_empty() {
        // Kernel threads have empty cmdline, try comm instead
        let comm_path = proc_path.join("comm");
        return fs::read_to_string(comm_path)
            .map(|s| format!("[{}]", s.trim()))
            .context("Failed to read comm");
    }

    // Replace null bytes with spaces
    let command = content
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s))
        .collect::<Vec<_>>()
        .join(" ");

    Ok(command)
}

/// Counts file descriptors in /proc/{pid}/fd/
fn count_fds(proc_path: &PathBuf) -> usize {
    let fd_path = proc_path.join("fd");
    fs::read_dir(fd_path)
        .map(|entries| entries.filter_map(Result::ok).count())
        .unwrap_or(0)
}

/// Reads soft limit from /proc/{pid}/limits
fn read_soft_limit(proc_path: &PathBuf) -> Option<u64> {
    let limits_path = proc_path.join("limits");
    let content = fs::read_to_string(limits_path).ok()?;

    for line in content.lines() {
        if line.contains("Max open files") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            // Format: "Max open files" <soft> <hard> <units>
            if let Some(soft_str) = parts.get(3) {
                return soft_str.parse().ok();
            }
        }
    }

    None
}

/// Reads CWD from /proc/{pid}/cwd symlink
fn read_cwd(proc_path: &PathBuf) -> String {
    let cwd_path = proc_path.join("cwd");
    fs::read_link(cwd_path)
        .ok()
        .and_then(|p| p.to_str().map(String::from))
        .unwrap_or_else(|| "?".to_string())
}

/// Reads FD details for a specific process
pub fn read_fd_details(pid: u32) -> Result<Vec<FdDetail>> {
    let fd_dir = PathBuf::from(format!("/proc/{}/fd", pid));
    let entries = fs::read_dir(&fd_dir).context("Failed to read fd directory")?;

    let mut details = Vec::new();

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let fd_str = file_name.to_string_lossy();

        if let Ok(fd_num) = fd_str.parse::<u32>() {
            if let Ok(target_path) = fs::read_link(entry.path()) {
                let target = target_path.to_string_lossy().to_string();
                let fd_type = classify_fd_type(&target);

                details.push(FdDetail {
                    fd_num,
                    target,
                    fd_type,
                });
            }
        }
    }

    // Sort by FD number
    details.sort_by_key(|d| d.fd_num);

    Ok(details)
}

/// Classifies FD type based on target string
fn classify_fd_type(target: &str) -> FdType {
    if target.starts_with("socket:") {
        FdType::Socket
    } else if target.starts_with("pipe:") {
        FdType::Pipe
    } else if target.starts_with("/dev/") {
        FdType::Device
    } else if target.starts_with('/') {
        FdType::File
    } else {
        FdType::Other
    }
}

/// Reads system-wide FD statistics
pub fn read_system_stats(current_uid: u32) -> Result<SystemStats> {
    // Read system-wide FD stats from /proc/sys/fs/file-nr
    let file_nr = fs::read_to_string("/proc/sys/fs/file-nr")
        .context("Failed to read /proc/sys/fs/file-nr")?;
    let parts: Vec<&str> = file_nr.split_whitespace().collect();

    let allocated_fds = parts
        .first()
        .and_then(|s| s.parse().ok())
        .context("Failed to parse allocated FDs")?;

    let max_fds = parts
        .get(2)
        .and_then(|s| s.parse().ok())
        .context("Failed to parse max FDs")?;

    // Get username
    let username = uid_to_username(current_uid);

    // Count user FDs
    let processes = scan_processes(current_uid)?;
    let user_fds = processes
        .iter()
        .filter(|p| p.uid == current_uid)
        .map(|p| p.fd_count as u64)
        .sum();

    // Get user limits from /proc/self/limits
    let limits_path = PathBuf::from("/proc/self/limits");
    let limits_content = fs::read_to_string(limits_path).context("Failed to read limits")?;

    let (mut user_soft_limit, mut user_hard_limit) = (1024, 1024);

    for line in limits_content.lines() {
        if line.contains("Max open files") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(soft_str) = parts.get(3) {
                user_soft_limit = soft_str.parse().unwrap_or(1024);
            }
            if let Some(hard_str) = parts.get(4) {
                user_hard_limit = hard_str.parse().unwrap_or(1024);
            }
            break;
        }
    }

    Ok(SystemStats {
        allocated_fds,
        max_fds,
        user_fds,
        user_soft_limit,
        user_hard_limit,
        username,
    })
}
