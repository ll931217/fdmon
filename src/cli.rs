use crate::proc::{read_fd_details, read_system_stats, scan_processes, FdDetail, ProcessInfo};
use crate::tree::build_tree;
use crate::{Command, OutputFormat, SortField};
use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::HashMap;

/// Wrapper struct for process list entries with computed fields
#[derive(Debug, Serialize)]
struct ProcessEntry {
    pid: u32,
    ppid: u32,
    owner: String,
    command: String,
    fds: usize,
    limit: u64,
    usage_percent: f64,
}

impl ProcessEntry {
    fn from_process_info(info: &ProcessInfo) -> Self {
        let limit = info.fd_soft_limit.unwrap_or(1024);
        let usage_percent = (info.fd_count as f64 / limit as f64) * 100.0;

        ProcessEntry {
            pid: info.pid,
            ppid: info.ppid,
            owner: info.owner.clone(),
            command: info.command.clone(),
            fds: info.fd_count,
            limit,
            usage_percent,
        }
    }
}

/// Wrapper struct for detail command output
#[derive(Debug, Serialize)]
struct ProcessDetail {
    pid: u32,
    command: String,
    owner: String,
    fd_count: usize,
    fd_soft_limit: u64,
    usage_percent: f64,
    cwd: String,
    fds: Vec<FdDetail>,
    timestamp: String,
}

/// Wrapper struct for stats command output
#[derive(Debug, Serialize)]
struct StatsOutput {
    system: SystemStatsEntry,
    user: UserStatsEntry,
    timestamp: String,
}

#[derive(Debug, Serialize)]
struct SystemStatsEntry {
    allocated_fds: u64,
    max_fds: u64,
    usage_percent: f64,
}

#[derive(Debug, Serialize)]
struct UserStatsEntry {
    username: String,
    total_fds: u64,
    soft_limit: u64,
    hard_limit: u64,
    usage_percent: f64,
}

/// Wrapper struct for summary command output
#[derive(Debug, Serialize)]
struct UserSummary {
    username: String,
    uid: u32,
    total_fds: usize,
    process_count: usize,
}

/// Main CLI execution dispatcher
pub fn execute(cmd: Command, format: OutputFormat, current_uid: u32) -> Result<()> {
    match cmd {
        Command::List { sort, filter, user, min_fds, limit } => {
            execute_list(format, sort, filter, user, min_fds, limit, current_uid)
        }
        Command::Tree => execute_tree(format, current_uid),
        Command::Detail { pid } => execute_detail(format, pid, current_uid),
        Command::Stats => execute_stats(format, current_uid),
        Command::Top { n } => execute_top(format, n, current_uid),
        Command::Summary => execute_summary(format, current_uid),
    }
}

/// Helper: Sort processes by specified field
fn sort_processes(processes: &mut [ProcessEntry], field: &SortField) {
    match field {
        SortField::Fds => processes.sort_by(|a, b| b.fds.cmp(&a.fds)),
        SortField::Pid => processes.sort_by(|a, b| a.pid.cmp(&b.pid)),
        SortField::Owner => processes.sort_by(|a, b| a.owner.cmp(&b.owner)),
        SortField::Command => processes.sort_by(|a, b| a.command.cmp(&b.command)),
    }
}

/// Helper: Filter processes by various criteria
fn filter_processes(
    processes: Vec<ProcessEntry>,
    filter: Option<String>,
    user: Option<String>,
    min_fds: Option<usize>,
) -> Vec<ProcessEntry> {
    processes
        .into_iter()
        .filter(|p| {
            if let Some(ref f) = filter {
                let f_lower = f.to_lowercase();
                p.command.to_lowercase().contains(&f_lower)
                    || p.owner.to_lowercase().contains(&f_lower)
                    || p.pid.to_string().contains(&f_lower)
            } else {
                true
            }
        })
        .filter(|p| {
            if let Some(ref u) = user {
                &p.owner == u
            } else {
                true
            }
        })
        .filter(|p| {
            if let Some(min) = min_fds {
                p.fds >= min
            } else {
                true
            }
        })
        .collect()
}

/// Helper: Get current timestamp in ISO 8601 format
fn get_timestamp() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Helper: Escape CSV field
fn escape_csv_field(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// Execute list command
fn execute_list(
    format: OutputFormat,
    sort: SortField,
    filter: Option<String>,
    user: Option<String>,
    min_fds: Option<usize>,
    limit: Option<usize>,
    current_uid: u32,
) -> Result<()> {
    let processes = scan_processes(current_uid)?;
    let mut entries: Vec<ProcessEntry> = processes.iter().map(ProcessEntry::from_process_info).collect();

    // Filter
    entries = filter_processes(entries, filter, user, min_fds);

    // Sort
    sort_processes(&mut entries, &sort);

    // Limit
    if let Some(lim) = limit {
        entries.truncate(lim);
    }

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&entries)?;
            println!("{}", json);
        }
        OutputFormat::Csv => {
            println!("fds,pid,owner,limit,usage_percent,command");
            for entry in entries {
                println!(
                    "{},{},{},{},{:.2},{}",
                    entry.fds,
                    entry.pid,
                    escape_csv_field(&entry.owner),
                    entry.limit,
                    entry.usage_percent,
                    escape_csv_field(&entry.command)
                );
            }
        }
        OutputFormat::Table => {
            println!("{:<8} {:<8} {:<12} {:<8} {:<8} {}", "FDS", "PID", "OWNER", "LIMIT", "USAGE%", "COMMAND");
            println!("{}", "-".repeat(72));
            for entry in entries {
                println!(
                    "{:<8} {:<8} {:<12} {:<8} {:<8.2} {}",
                    entry.fds, entry.pid, entry.owner, entry.limit, entry.usage_percent, entry.command
                );
            }
        }
    }

    Ok(())
}

/// Execute tree command
fn execute_tree(format: OutputFormat, current_uid: u32) -> Result<()> {
    let processes = scan_processes(current_uid)?;
    let tree = build_tree(&processes);

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&tree)?;
            println!("{}", json);
        }
        OutputFormat::Csv => {
            println!("depth,pid,owner,fds,command");
            for node in tree {
                println!(
                    "{},{},{},{},{}",
                    node.depth,
                    node.process.pid,
                    escape_csv_field(&node.process.owner),
                    node.process.fd_count,
                    escape_csv_field(&node.process.command)
                );
            }
        }
        OutputFormat::Table => {
            for node in tree {
                let indent = "  ".repeat(node.depth);
                let marker = if node.has_children { "+" } else { "-" };
                println!(
                    "{}{} {} {} ({} fds){}",
                    indent,
                    marker,
                    node.process.pid,
                    node.process.command,
                    node.process.fd_count,
                    if node.is_ancestor_only { " [ancestor]" } else { "" }
                );
            }
        }
    }

    Ok(())
}

/// Execute detail command
fn execute_detail(format: OutputFormat, pid: u32, current_uid: u32) -> Result<()> {
    let processes = scan_processes(current_uid)?;
    let process = processes
        .iter()
        .find(|p| p.pid == pid)
        .with_context(|| format!("Process {} not found", pid))?;

    let fds = read_fd_details(pid)?;
    let limit = process.fd_soft_limit.unwrap_or(1024);
    let usage_percent = (process.fd_count as f64 / limit as f64) * 100.0;

    let detail = ProcessDetail {
        pid: process.pid,
        command: process.command.clone(),
        owner: process.owner.clone(),
        fd_count: process.fd_count,
        fd_soft_limit: limit,
        usage_percent,
        cwd: process.cwd.clone(),
        fds,
        timestamp: get_timestamp(),
    };

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&detail)?;
            println!("{}", json);
        }
        OutputFormat::Csv => {
            println!("fd_num,target,fd_type");
            for fd in detail.fds {
                println!(
                    "{},{},{}",
                    fd.fd_num,
                    escape_csv_field(&fd.target),
                    fd.fd_type
                );
            }
        }
        OutputFormat::Table => {
            println!("Process: {} (PID {})", detail.command, detail.pid);
            println!("Owner: {}", detail.owner);
            println!("CWD: {}", detail.cwd);
            println!("FD Count: {} / {} ({:.2}%)", detail.fd_count, detail.fd_soft_limit, detail.usage_percent);
            println!("Timestamp: {}", detail.timestamp);
            println!();
            println!("{:<6} {:<12} {}", "FD", "TYPE", "TARGET");
            println!("{}", "-".repeat(72));
            for fd in detail.fds {
                println!("{:<6} {:<12} {}", fd.fd_num, fd.fd_type, fd.target);
            }
        }
    }

    Ok(())
}

/// Execute stats command
fn execute_stats(format: OutputFormat, current_uid: u32) -> Result<()> {
    let stats = read_system_stats(current_uid)?;

    let system_usage = (stats.allocated_fds as f64 / stats.max_fds as f64) * 100.0;
    let user_usage = (stats.user_fds as f64 / stats.user_soft_limit as f64) * 100.0;

    let output = StatsOutput {
        system: SystemStatsEntry {
            allocated_fds: stats.allocated_fds,
            max_fds: stats.max_fds,
            usage_percent: system_usage,
        },
        user: UserStatsEntry {
            username: stats.username.clone(),
            total_fds: stats.user_fds,
            soft_limit: stats.user_soft_limit,
            hard_limit: stats.user_hard_limit,
            usage_percent: user_usage,
        },
        timestamp: get_timestamp(),
    };

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&output)?;
            println!("{}", json);
        }
        OutputFormat::Csv => {
            println!("category,metric,value");
            println!("system,allocated_fds,{}", output.system.allocated_fds);
            println!("system,max_fds,{}", output.system.max_fds);
            println!("system,usage_percent,{:.2}", output.system.usage_percent);
            println!("user,username,{}", escape_csv_field(&output.user.username));
            println!("user,total_fds,{}", output.user.total_fds);
            println!("user,soft_limit,{}", output.user.soft_limit);
            println!("user,hard_limit,{}", output.user.hard_limit);
            println!("user,usage_percent,{:.2}", output.user.usage_percent);
        }
        OutputFormat::Table => {
            println!("System Statistics:");
            println!("  Allocated FDs: {} / {} ({:.2}%)",
                output.system.allocated_fds, output.system.max_fds, output.system.usage_percent);
            println!();
            println!("User Statistics ({}):", output.user.username);
            println!("  Total FDs: {} / {} ({:.2}%)",
                output.user.total_fds, output.user.soft_limit, output.user.usage_percent);
            println!("  Hard Limit: {}", output.user.hard_limit);
            println!();
            println!("Timestamp: {}", output.timestamp);
        }
    }

    Ok(())
}

/// Execute top command
fn execute_top(format: OutputFormat, n: usize, current_uid: u32) -> Result<()> {
    execute_list(
        format,
        SortField::Fds,
        None,
        None,
        None,
        Some(n),
        current_uid,
    )
}

/// Execute summary command
fn execute_summary(format: OutputFormat, current_uid: u32) -> Result<()> {
    let processes = scan_processes(current_uid)?;

    // Group by user
    let mut user_map: HashMap<String, (u32, usize, usize)> = HashMap::new();

    for process in &processes {
        let entry = user_map.entry(process.owner.clone()).or_insert((process.uid, 0, 0));
        entry.1 += process.fd_count;
        entry.2 += 1;
    }

    let mut summaries: Vec<UserSummary> = user_map
        .into_iter()
        .map(|(username, (uid, total_fds, process_count))| UserSummary {
            username,
            uid,
            total_fds,
            process_count,
        })
        .collect();

    // Sort by total FDs descending
    summaries.sort_by(|a, b| b.total_fds.cmp(&a.total_fds));

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&summaries)?;
            println!("{}", json);
        }
        OutputFormat::Csv => {
            println!("username,uid,total_fds,process_count");
            for summary in summaries {
                println!(
                    "{},{},{},{}",
                    escape_csv_field(&summary.username),
                    summary.uid,
                    summary.total_fds,
                    summary.process_count
                );
            }
        }
        OutputFormat::Table => {
            println!("{:<16} {:<8} {:<12} {}", "USERNAME", "UID", "TOTAL_FDS", "PROCESSES");
            println!("{}", "-".repeat(72));
            for summary in summaries {
                println!(
                    "{:<16} {:<8} {:<12} {}",
                    summary.username, summary.uid, summary.total_fds, summary.process_count
                );
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_csv_field_simple() {
        assert_eq!(escape_csv_field("simple"), "simple");
        assert_eq!(escape_csv_field("with space"), "with space");
    }

    #[test]
    fn test_escape_csv_field_with_comma() {
        assert_eq!(escape_csv_field("hello,world"), "\"hello,world\"");
    }

    #[test]
    fn test_escape_csv_field_with_quotes() {
        assert_eq!(escape_csv_field("say \"hello\""), "\"say \"\"hello\"\"\"");
    }

    #[test]
    fn test_escape_csv_field_with_newline() {
        assert_eq!(escape_csv_field("line1\nline2"), "\"line1\nline2\"");
    }

    #[test]
    fn test_sort_processes_by_fds() {
        let mut processes = vec![
            create_test_entry(1, 100, "user1", "cmd1"),
            create_test_entry(2, 50, "user2", "cmd2"),
            create_test_entry(3, 200, "user3", "cmd3"),
        ];

        sort_processes(&mut processes, &SortField::Fds);

        assert_eq!(processes[0].fds, 200);
        assert_eq!(processes[1].fds, 100);
        assert_eq!(processes[2].fds, 50);
    }

    #[test]
    fn test_sort_processes_by_pid() {
        let mut processes = vec![
            create_test_entry(300, 100, "user1", "cmd1"),
            create_test_entry(100, 50, "user2", "cmd2"),
            create_test_entry(200, 200, "user3", "cmd3"),
        ];

        sort_processes(&mut processes, &SortField::Pid);

        assert_eq!(processes[0].pid, 100);
        assert_eq!(processes[1].pid, 200);
        assert_eq!(processes[2].pid, 300);
    }

    #[test]
    fn test_sort_processes_by_owner() {
        let mut processes = vec![
            create_test_entry(1, 100, "zebra", "cmd1"),
            create_test_entry(2, 50, "alpha", "cmd2"),
            create_test_entry(3, 200, "beta", "cmd3"),
        ];

        sort_processes(&mut processes, &SortField::Owner);

        assert_eq!(processes[0].owner, "alpha");
        assert_eq!(processes[1].owner, "beta");
        assert_eq!(processes[2].owner, "zebra");
    }

    #[test]
    fn test_sort_processes_by_command() {
        let mut processes = vec![
            create_test_entry(1, 100, "user1", "zsh"),
            create_test_entry(2, 50, "user2", "bash"),
            create_test_entry(3, 200, "user3", "sh"),
        ];

        sort_processes(&mut processes, &SortField::Command);

        assert_eq!(processes[0].command, "bash");
        assert_eq!(processes[1].command, "sh");
        assert_eq!(processes[2].command, "zsh");
    }

    #[test]
    fn test_filter_processes_by_filter() {
        let processes = vec![
            create_test_entry(1, 100, "user1", "claude"),
            create_test_entry(2, 50, "user2", "bash"),
            create_test_entry(3, 200, "user3", "chrome"),
        ];

        let filtered = filter_processes(processes, Some("cla".to_string()), None, None);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].command, "claude");
    }

    #[test]
    fn test_filter_processes_by_user() {
        let processes = vec![
            create_test_entry(1, 100, "alice", "cmd1"),
            create_test_entry(2, 50, "bob", "cmd2"),
            create_test_entry(3, 200, "alice", "cmd3"),
        ];

        let filtered = filter_processes(processes, None, Some("alice".to_string()), None);

        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|p| p.owner == "alice"));
    }

    #[test]
    fn test_filter_processes_by_min_fds() {
        let processes = vec![
            create_test_entry(1, 100, "user1", "cmd1"),
            create_test_entry(2, 50, "user2", "cmd2"),
            create_test_entry(3, 200, "user3", "cmd3"),
        ];

        let filtered = filter_processes(processes, None, None, Some(75));

        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|p| p.fds >= 75));
    }

    #[test]
    fn test_filter_processes_multiple_filters() {
        let processes = vec![
            create_test_entry(1, 100, "alice", "claude"),
            create_test_entry(2, 50, "alice", "bash"),
            create_test_entry(3, 200, "bob", "claude"),
        ];

        let filtered = filter_processes(
            processes,
            Some("claude".to_string()),
            Some("alice".to_string()),
            Some(75),
        );

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].pid, 1);
    }

    fn create_test_entry(pid: u32, fds: usize, owner: &str, command: &str) -> ProcessEntry {
        ProcessEntry {
            pid,
            ppid: 0,
            owner: owner.to_string(),
            command: command.to_string(),
            fds,
            limit: 1024,
            usage_percent: (fds as f64 / 1024.0) * 100.0,
        }
    }
}
