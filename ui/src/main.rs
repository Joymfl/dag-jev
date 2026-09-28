use std::{
    error::Error,
    io::{self, ErrorKind, Stderr},
};

use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
};

pub mod ui;

// TODO: make it more generic, this constrains it to crossterm backend and stderr writer
fn setup() -> Result<Terminal<CrosstermBackend<Stderr>>, Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stderr = io::stderr();

    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stderr);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

struct App {
    pub current_prompt: String,
    pub history: Vec<String>,
    pub run: bool,
}

impl App {
    fn new() -> Self {
        Self {
            current_prompt: String::new(),
            history: Vec::new(),
            run: true,
        }
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    let mut terminal = setup()?;

    let mut app = App::new();

    run_app(&mut terminal, &mut app)?;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<bool>
where
    io::Error: From<B::Error>,
{
    // render loop
    loop {
        terminal.draw(|f| ui::ui(f, app))?;
        handle_event(app).map_err(|err| io::Error::other(err.to_string()))?;
        if !app.run {
            return Ok(true);
        }
    }
}

fn handle_event(app: &mut App) -> Result<(), Box<dyn Error>> {
    if let Event::Key(key) = event::read()? {
        if key.kind == event::KeyEventKind::Release {
            return Ok(());
        }
        if let KeyCode::Char('q') = key.code {
            app.run = false;
            return Ok(());
        }
    }
    Ok(())
}
