use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use oxidebpf::ProgramBlueprint;
use std::{error::Error, io};
use tui::text::Spans;
use tui::widgets::{Paragraph, Wrap};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders},
    Frame, Terminal,
};

/// Open a file and display its hex.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// A file to open and parse.
    #[clap(short, long)]
    file: Option<String>,
}

struct App<'a> {
    file_as_bytes: Vec<u8>,
    file_blueprint: Option<ProgramBlueprint>,
    file_as_hex: Vec<Spans<'a>>,
    file_as_ascii: Vec<Spans<'a>>,
}

impl<'a> App<'_> {
    fn new() -> App<'a> {
        App {
            file_as_bytes: vec![],
            file_as_hex: vec![],
            file_as_ascii: vec![],
            file_blueprint: None,
        }
    }

    fn move_up(&mut self, number: Option<usize>) {
        let number = number.unwrap_or(1);
        self.file_as_ascii.rotate_right(number);
        self.file_as_hex.rotate_right(number);
    }

    fn move_down(&mut self, number: Option<usize>) {
        let number = number.unwrap_or(1);
        self.file_as_ascii.rotate_left(number);
        self.file_as_hex.rotate_left(number);
    }

    fn parse_blueprint(&mut self, file_name: &str) {
        let program_bytes = std::fs::read(file_name).unwrap();
        let program_blueprint = ProgramBlueprint::new(&program_bytes, None).unwrap();
        println!("{:#?}", program_blueprint);
        self.file_blueprint = Some(program_blueprint);
    }

    fn open_file(&mut self, file_name: &str) {
        self.parse_blueprint(file_name);
        if let Ok(v) = std::fs::read(file_name) {
            self.file_as_bytes = v.clone();
            let mut vc = v;
            while !vc.is_empty() {
                let mut hex_str: String = "".to_owned();
                let mut ascii_str: String = "".to_owned();
                for bt in vc.drain(..8) {
                    hex_str.push_str(format!("{:02X} ", bt).as_str());
                    ascii_str.push(match bt {
                        0x20..=0x7E => bt as char,
                        _ => '.',
                    });
                }
                hex_str.push_str("   ");
                ascii_str.push(' ');
                if vc.len() < 8 {
                    self.file_as_hex.push(Spans::from(hex_str));
                    self.file_as_ascii.push(Spans::from(ascii_str));
                    break;
                }
                for bt in vc.drain(..8) {
                    hex_str.push_str(format!("{:02X} ", bt).as_str());
                    ascii_str.push(match bt {
                        0x20..=0x7E => bt as char,
                        _ => '.',
                    });
                }
                self.file_as_hex.push(Spans::from(hex_str));
                self.file_as_ascii.push(Spans::from(ascii_str));
            }
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // parse args and create app
    let args = Args::parse();
    let mut app = App::new();

    // open the file
    if let Some(file_name) = args.file {
        println!("Analyzing your file...");
        app.open_file(&file_name);
    }

    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // run the app
    let res = run_app(&mut terminal, app);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &app))?;

        if let Event::Key(key) = event::read()? {
            if let KeyCode::Char('q') = key.code {
                return Ok(());
            }
            if let KeyCode::Up = key.code {
                app.move_up(None);
            }
            if let KeyCode::Down = key.code {
                app.move_down(None);
            }
        }
    }
}

fn create_block(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black).fg(Color::White))
        .title(Span::styled(
            title,
            Style::default().add_modifier(Modifier::BOLD),
        ))
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &App) {
    // Wrapping block for a group
    // Just draw the block and the group on the same area and build the group
    // with at least a margin of 1
    let size = f.size();

    // Surrounding block
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" eBELt - eBPF & ELF terminal meddler ")
        .title_alignment(Alignment::Center)
        .border_type(BorderType::Rounded);
    f.render_widget(block, size);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .margin(1)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
        .split(f.size());

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(80), Constraint::Percentage(20)].as_ref())
        .split(chunks[0]);

    // help menu
    let paragraph = Paragraph::new(
        "[q] - [Q]uit the application\n\
            [↑] - move up in the file\n\
            [↓] - move down in the file\n\
            [g] - display [g]oblin analysis\n\
            [o] - display [o]xidebpf analysis\n\
            [h] - display [h]exdump
        ",
    )
    .style(Style::default().bg(Color::Black).fg(Color::White))
    .block(create_block("Usage"))
    .wrap(Wrap { trim: true })
    .alignment(Alignment::Left);
    f.render_widget(paragraph, left_chunks[1]);

    // file hexdump
    let paragraph = Paragraph::new(app.file_as_hex.clone())
        .style(Style::default().bg(Color::Black).fg(Color::White))
        .block(create_block("hexdump"))
        .alignment(Alignment::Left);
    f.render_widget(paragraph, left_chunks[0]);

    // file ascii
    let paragraph = Paragraph::new(app.file_as_ascii.clone())
        .style(Style::default().bg(Color::Black).fg(Color::White))
        .block(create_block("ASCII"))
        .alignment(Alignment::Left);
    f.render_widget(paragraph, chunks[1]);
}
