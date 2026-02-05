mod app;
mod cli;
mod event;
mod proc;
mod tree;
mod ui;

use anyhow::Result;
use app::App;
use clap::{Parser, Subcommand, ValueEnum};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use event::{Event, EventHandler};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

#[derive(Debug, Clone, ValueEnum)]
pub enum OutputFormat {
    Table,
    Json,
    Csv,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum SortField {
    Fds,
    Pid,
    Owner,
    Command,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// List all processes with FD counts
    List {
        /// Sort by field
        #[arg(long, default_value = "fds")]
        sort: SortField,

        /// Filter by command, owner, or PID
        #[arg(long)]
        filter: Option<String>,

        /// Filter by username
        #[arg(long)]
        user: Option<String>,

        /// Minimum FD count threshold
        #[arg(long)]
        min_fds: Option<usize>,

        /// Maximum number of results
        #[arg(long)]
        limit: Option<usize>,
    },

    /// Show process tree (FD > 0 + ancestors)
    Tree,

    /// Show FD details for a specific process
    Detail {
        /// Process ID
        pid: u32,
    },

    /// Show system-wide FD statistics
    Stats,

    /// Show top N processes by FD count
    Top {
        /// Number of processes to show
        #[arg(default_value = "10")]
        n: usize,
    },

    /// Show per-user FD usage breakdown
    Summary,
}

#[derive(Parser, Debug)]
#[command(name = "fdmon")]
#[command(about = "File Descriptor Monitor - Interactive TUI for monitoring process file descriptors", long_about = None)]
struct Cli {
    /// Output format (applies to CLI mode only)
    #[arg(long, global = true, default_value = "table")]
    format: OutputFormat,

    /// Subcommand (if none, runs TUI mode)
    #[command(subcommand)]
    command: Option<Command>,

    /// Refresh interval in seconds (TUI mode only)
    #[arg(short, long, default_value = "2", value_parser = clap::value_parser!(u64).range(1..=10))]
    interval: u64,

    /// Exit after N refreshes (TUI mode only)
    #[arg(short, long, default_value = "0")]
    count: usize,
}

fn main() -> Result<()> {
    let args = Cli::parse();

    match args.command {
        Some(cmd) => {
            // CLI mode: execute command and print to stdout
            let current_uid = nix::unistd::getuid().as_raw();
            cli::execute(cmd, args.format, current_uid)
        }
        None => {
            // TUI mode: run interactive interface
            run_tui(args.interval, args.count)
        }
    }
}

fn run_tui(interval: u64, count: usize) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app
    let refresh_interval = Duration::from_secs(interval);
    let result = run_app(&mut terminal, refresh_interval, count);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    refresh_interval: Duration,
    max_count: usize,
) -> Result<()>
where
    <B as ratatui::backend::Backend>::Error: Send + Sync + 'static,
{
    let mut app = App::new(refresh_interval)?;
    let mut event_handler = EventHandler::new(refresh_interval);
    let mut tick_count = 0;

    loop {
        // Render
        terminal.draw(|f| ui::render(f, &app))?;

        // Handle events
        match event_handler.next()? {
            Event::Key(key) => {
                app.handle_key(key)?;
            }
            Event::Tick => {
                app.tick()?;
                tick_count += 1;

                // Update event handler tick rate if changed
                if event_handler.tick_rate != app.refresh_interval {
                    event_handler.set_tick_rate(app.refresh_interval);
                }

                // Check exit condition
                if max_count > 0 && tick_count >= max_count {
                    break;
                }
            }
        }

        if !app.running {
            break;
        }
    }

    Ok(())
}
