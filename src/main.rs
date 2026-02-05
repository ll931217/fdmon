mod app;
mod event;
mod proc;
mod tree;
mod ui;

use anyhow::Result;
use app::App;
use clap::Parser;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use event::{Event, EventHandler};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "fdmon")]
#[command(about = "File Descriptor Monitor - Interactive TUI for monitoring process file descriptors", long_about = None)]
struct Cli {
    /// Refresh interval in seconds (1-10)
    #[arg(short, long, default_value = "2", value_parser = clap::value_parser!(u64).range(1..=10))]
    interval: u64,

    /// Exit after N refreshes (0 = run forever)
    #[arg(short, long, default_value = "0")]
    count: usize,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app
    let refresh_interval = Duration::from_secs(cli.interval);
    let result = run_app(&mut terminal, refresh_interval, cli.count);

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
) -> Result<()> {
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
