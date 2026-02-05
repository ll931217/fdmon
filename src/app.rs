use crate::proc::{read_fd_details, read_system_stats, scan_processes, FdDetail, ProcessInfo, SystemStats};
use crate::tree::{build_tree, TreeNode};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewMode {
    Table,
    Tree,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputMode {
    Normal,
    Search,
    KillConfirm,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortColumn {
    FdCount,
    Pid,
    Owner,
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StatusLevel {
    Info,
    Error,
}

pub struct App {
    pub processes: Vec<ProcessInfo>,
    pub tree_nodes: Vec<TreeNode>,
    pub view: ViewMode,
    pub selected: usize,
    pub system_stats: SystemStats,
    pub filter: Option<String>,
    pub input_mode: InputMode,
    pub search_input: String,
    pub confirm_kill: Option<u32>,
    pub status_message: Option<(String, StatusLevel)>,
    pub refresh_interval: Duration,
    pub sort_column: SortColumn,
    pub detail_open: bool,
    pub fd_details: Vec<FdDetail>,
    pub detail_scroll_offset: usize,
    pub last_detail_pid: Option<u32>,
    pub current_uid: u32,
    pub running: bool,
}

impl App {
    pub fn new(refresh_interval: Duration) -> Result<Self> {
        let current_uid = uzers::get_current_uid();
        let processes = scan_processes(current_uid)?;
        let system_stats = read_system_stats(current_uid)?;
        let tree_nodes = build_tree(&processes);

        Ok(Self {
            processes,
            tree_nodes,
            view: ViewMode::Table,
            selected: 0,
            system_stats,
            filter: None,
            input_mode: InputMode::Normal,
            search_input: String::new(),
            confirm_kill: None,
            status_message: None,
            refresh_interval,
            sort_column: SortColumn::FdCount,
            detail_open: false,
            fd_details: Vec::new(),
            detail_scroll_offset: 0,
            last_detail_pid: None,
            current_uid,
            running: true,
        })
    }

    /// Refreshes process data
    pub fn tick(&mut self) -> Result<()> {
        let processes = scan_processes(self.current_uid)?;
        let system_stats = read_system_stats(self.current_uid)?;

        // Apply sort
        let mut sorted_processes = processes;
        self.sort_processes(&mut sorted_processes);

        self.processes = sorted_processes;
        self.system_stats = system_stats;
        self.tree_nodes = build_tree(&self.processes);

        // Refresh FD details if panel is open
        if self.detail_open {
            if let Some(process) = self.get_selected_process() {
                self.fd_details = read_fd_details(process.pid).unwrap_or_default();
            }
        }

        Ok(())
    }

    /// Handles keyboard input
    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match self.input_mode {
            InputMode::Normal => self.handle_normal_mode(key),
            InputMode::Search => self.handle_search_mode(key),
            InputMode::KillConfirm => self.handle_kill_confirm_mode(key),
        }
    }

    fn handle_normal_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.running = false;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.move_up();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.move_down();
            }
            KeyCode::Tab => {
                self.toggle_view();
            }
            KeyCode::Enter => {
                self.toggle_detail_panel();
            }
            KeyCode::Char('K') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.initiate_kill();
            }
            KeyCode::Char('/') => {
                self.input_mode = InputMode::Search;
                self.search_input.clear();
            }
            KeyCode::Char('s') => {
                self.cycle_sort();
            }
            KeyCode::Char('+') => {
                self.adjust_refresh_interval(1);
            }
            KeyCode::Char('-') => {
                self.adjust_refresh_interval(-1);
            }
            KeyCode::PageUp | KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll_detail_up();
            }
            KeyCode::PageDown | KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll_detail_down();
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_search_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.search_input.clear();
                self.filter = None;
            }
            KeyCode::Enter => {
                self.input_mode = InputMode::Normal;
                self.filter = if self.search_input.is_empty() {
                    None
                } else {
                    Some(self.search_input.clone())
                };
            }
            KeyCode::Char(c) => {
                self.search_input.push(c);
            }
            KeyCode::Backspace => {
                self.search_input.pop();
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_kill_confirm_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.confirm_kill = None;
            }
            KeyCode::Char('t') => {
                if let Some(pid) = self.confirm_kill {
                    self.send_signal(pid, Signal::SIGTERM)?;
                }
                self.input_mode = InputMode::Normal;
                self.confirm_kill = None;
            }
            KeyCode::Char('k') => {
                if let Some(pid) = self.confirm_kill {
                    self.send_signal(pid, Signal::SIGKILL)?;
                }
                self.input_mode = InputMode::Normal;
                self.confirm_kill = None;
            }
            _ => {}
        }

        Ok(())
    }

    fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.refresh_detail_if_needed();
        }
    }

    fn move_down(&mut self) {
        let max = self.get_visible_count().saturating_sub(1);
        if self.selected < max {
            self.selected += 1;
            self.refresh_detail_if_needed();
        }
    }

    fn toggle_view(&mut self) {
        self.view = match self.view {
            ViewMode::Table => ViewMode::Tree,
            ViewMode::Tree => ViewMode::Table,
        };
        self.selected = 0;
        self.refresh_detail_if_needed();
    }

    fn toggle_detail_panel(&mut self) {
        self.detail_open = !self.detail_open;

        if self.detail_open {
            // Extract PID first to avoid borrow checker issues
            let pid = self.get_selected_process().map(|p| p.pid);
            if let Some(pid) = pid {
                self.fd_details = read_fd_details(pid).unwrap_or_default();
                self.last_detail_pid = Some(pid);
                self.detail_scroll_offset = 0;
            }
        }
    }

    fn initiate_kill(&mut self) {
        // Extract needed values to avoid borrow checker issues
        let process_info = self.get_selected_process().map(|p| (p.pid, p.uid, p.owner.clone()));

        if let Some((pid, uid, owner)) = process_info {
            if uid != self.current_uid {
                self.status_message = Some((
                    format!("Cannot kill: process owned by {}", owner),
                    StatusLevel::Error,
                ));
            } else {
                self.input_mode = InputMode::KillConfirm;
                self.confirm_kill = Some(pid);
            }
        }
    }

    fn send_signal(&mut self, pid: u32, signal: Signal) -> Result<()> {
        let result = signal::kill(Pid::from_raw(pid as i32), signal);

        match result {
            Ok(_) => {
                let signal_name = match signal {
                    Signal::SIGTERM => "SIGTERM",
                    Signal::SIGKILL => "SIGKILL",
                    _ => "signal",
                };
                self.status_message = Some((
                    format!("Sent {} to PID {}", signal_name, pid),
                    StatusLevel::Info,
                ));
            }
            Err(e) => {
                self.status_message = Some((
                    format!("Failed to kill PID {}: {}", pid, e),
                    StatusLevel::Error,
                ));
            }
        }

        Ok(())
    }

    fn cycle_sort(&mut self) {
        self.sort_column = match self.sort_column {
            SortColumn::FdCount => SortColumn::Pid,
            SortColumn::Pid => SortColumn::Owner,
            SortColumn::Owner => SortColumn::Command,
            SortColumn::Command => SortColumn::FdCount,
        };

        // Re-sort (inline to avoid borrow checker issues)
        match self.sort_column {
            SortColumn::FdCount => {
                self.processes.sort_by(|a, b| b.fd_count.cmp(&a.fd_count));
            }
            SortColumn::Pid => {
                self.processes.sort_by_key(|p| p.pid);
            }
            SortColumn::Owner => {
                self.processes.sort_by(|a, b| a.owner.cmp(&b.owner));
            }
            SortColumn::Command => {
                self.processes.sort_by(|a, b| a.command.cmp(&b.command));
            }
        }
    }

    fn sort_processes(&self, processes: &mut [ProcessInfo]) {
        match self.sort_column {
            SortColumn::FdCount => {
                processes.sort_by(|a, b| b.fd_count.cmp(&a.fd_count));
            }
            SortColumn::Pid => {
                processes.sort_by_key(|p| p.pid);
            }
            SortColumn::Owner => {
                processes.sort_by(|a, b| a.owner.cmp(&b.owner));
            }
            SortColumn::Command => {
                processes.sort_by(|a, b| a.command.cmp(&b.command));
            }
        }
    }

    fn adjust_refresh_interval(&mut self, delta: i32) {
        let current_secs = self.refresh_interval.as_secs();
        let new_secs = (current_secs as i32 + delta).clamp(1, 10) as u64;
        self.refresh_interval = Duration::from_secs(new_secs);

        self.status_message = Some((
            format!("Refresh interval: {}s", new_secs),
            StatusLevel::Info,
        ));
    }

    fn get_visible_count(&self) -> usize {
        match self.view {
            ViewMode::Table => self.get_filtered_processes().len(),
            ViewMode::Tree => self.tree_nodes.len(),
        }
    }

    pub fn get_filtered_processes(&self) -> Vec<&ProcessInfo> {
        let mut filtered: Vec<&ProcessInfo> = self.processes.iter().collect();

        if let Some(filter) = &self.filter {
            let filter_lower = filter.to_lowercase();
            filtered.retain(|p| {
                p.command.to_lowercase().contains(&filter_lower)
                    || p.owner.to_lowercase().contains(&filter_lower)
                    || p.pid.to_string().contains(&filter_lower)
            });
        }

        filtered
    }

    fn get_selected_process(&self) -> Option<&ProcessInfo> {
        match self.view {
            ViewMode::Table => {
                let filtered = self.get_filtered_processes();
                filtered.get(self.selected).copied()
            }
            ViewMode::Tree => self
                .tree_nodes
                .get(self.selected)
                .map(|node| &node.process),
        }
    }

    /// Refreshes FD details if the detail panel is open and the selected process changed
    fn refresh_detail_if_needed(&mut self) {
        if !self.detail_open {
            return;
        }

        // Extract PID first to avoid borrow checker issues
        let pid = self.get_selected_process().map(|p| p.pid);

        if let Some(pid) = pid {
            // Only refresh if the PID changed
            if self.last_detail_pid != Some(pid) {
                self.fd_details = read_fd_details(pid).unwrap_or_default();
                self.last_detail_pid = Some(pid);
                self.detail_scroll_offset = 0;
            }
        }
    }

    /// Scrolls the detail panel up
    fn scroll_detail_up(&mut self) {
        if self.detail_open && self.detail_scroll_offset > 0 {
            self.detail_scroll_offset = self.detail_scroll_offset.saturating_sub(5);
        }
    }

    /// Scrolls the detail panel down
    fn scroll_detail_down(&mut self) {
        if self.detail_open && !self.fd_details.is_empty() {
            let max_scroll = self.fd_details.len().saturating_sub(1);
            self.detail_scroll_offset = (self.detail_scroll_offset + 5).min(max_scroll);
        }
    }
}
