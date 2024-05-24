use std::{
    fs,
    io,
    time::{Duration, Instant},
    path::PathBuf,
    error::Error,
    collections::HashMap,
};
use serde::{Serialize, Deserialize};
use image::{
    io::Reader as ImageReader,
    Rgb,
    RgbImage,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::*, symbols::scrollbar, widgets::*};
use colored::Colorize;
use directories::ProjectDirs;

// The "Outline" color. Default is this.
const SEPARATOR_COLOR: Rgb8 = Rgb8([32, 32, 32]);

fn rgb8_to_true(rgb: Rgb8) -> colored::Color {
    colored::Color::TrueColor {
        r: rgb.0[0],
        g: rgb.0[1],
        b: rgb.0[2],
    }
}

#[derive(Serialize, Deserialize, Hash, Eq, PartialEq, PartialOrd, Clone, Copy, Debug)]
struct Rgb8([u8; 3]);
trait ToRgb8 {
    fn to_rgb8(self) -> Rgb8;
}
impl ToRgb8 for Rgb<u8> {
    fn to_rgb8(self) -> Rgb8 {
        Rgb8(self.0)
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct ColorMap {
    color_map: HashMap<Rgb8, String>,
}

impl ColorMap {
    fn new() -> ColorMap {
        ColorMap {
            color_map: HashMap::new(),
        }
    }

    fn ensure_mapped(&mut self, color: Rgb8) -> Result<(), Box<dyn Error>> {
        use io::Write;

        if self.color_map.contains_key(&color) {
            return Ok(())
        }
        let colored_rgb = format!("{:?}", color)
            .color(rgb8_to_true(color))
            .on_color(rgb8_to_true(SEPARATOR_COLOR));
        println!("Found new color: {}", colored_rgb);
        print!("Please give it a name:");
        io::stdout().flush()?;
        let mut name = String::new();
        io::stdin().read_line(&mut name)?;
        self.color_map.insert(color, name.trim().to_owned());
        Ok(())
    }

    fn map(&self, color: Rgb8) -> &str {
        &self.color_map[&color]
    }
}

#[derive(Serialize, Deserialize, Hash, Eq, PartialEq, PartialOrd, Clone, Debug)]
struct Progress {
    row: usize,
    col: usize,
}

struct Config {
    color_path: PathBuf,
    progress_path: PathBuf,
    color_map: ColorMap,
    progress: Progress,
}

impl Config {
    fn load(project_dir: PathBuf) -> Result<Config, Box<dyn Error>> {
        let color_path = project_dir.join("colors.ron");
        let progress_path = project_dir.join("progress.ron");

        if !project_dir.exists() {
            fs::create_dir_all(project_dir)?;
        }

        let color_map = if color_path.exists() {
            let cm_str = fs::read_to_string(&color_path)?;
            ron::from_str(&cm_str)?
        } else {
            ColorMap::new()
        };

        let progress = if progress_path.exists() {
            let prog_str = fs::read_to_string(&progress_path)?;
            ron::from_str(&prog_str)?
        } else {
            Progress {
                row: 3, col: 0,
            }
        };

        Ok(Config {
            color_path,
            progress_path,
            color_map,
            progress
        })
    }

    fn save(&self) -> Result<(), Box<dyn Error>> {
        fs::write(&self.color_path, ron::to_string(&self.color_map)?)?;
        fs::write(&self.progress_path, ron::to_string(&self.progress)?)?;
        Ok(())

    }
}

struct App<'a> {
    lines: [Vec<Rgb8>; 3],
    scroll: ScrollbarState,
    scroll_amount: usize,
    progress: &'a mut Progress
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args();
    args.next();
    let file = match args.next() {
        Some(f) => f,
        None => return Err("File argument required.".into()),
    };
    println!("Opening file {}", file);

    let project_dir = match ProjectDirs::from("page", "adno", "igp_pattern_printer") {
        Some(proj_dirs) => proj_dirs.config_dir().to_owned(),
        None => return Err("Could not find config directory".into()),
    };
    let mut config = Config::load(project_dir)?;

    let img = ImageReader::open(file)?.decode()?.to_rgb8();

    let rows = build_rows(img, &mut config.color_map)?;
    //print_grid(rows, &mut config.color_map);
    let mut term = setup_tui()?;
    run_app(&mut term, &mut config, rows)?;
    teardown_tui(term)?;
    Ok(())
}

fn build_rows(mut img: RgbImage, color_map: &mut ColorMap) -> Result<Vec<Vec<Rgb8>>, Box<dyn Error>> {
    let mut rows: Vec<Vec<Rgb8>> = vec![];
    let mut current_row: Vec<Rgb8> = vec![];
    for y in 0..(img.height()) {
        for x in 0..(img.width()) {
            if img[(x, y)].to_rgb8() == SEPARATOR_COLOR {
                continue;
            }
            current_row.push(img[(x, y)].to_rgb8());
            color_map.ensure_mapped(img[(x, y)].to_rgb8())?;
            flood_fill(&mut img, (x, y));
        }
        if !current_row.is_empty() {
            rows.push(current_row);
            current_row = vec![];
        }
    }
    Ok(rows)
}

fn setup_tui() -> Result<Terminal<impl Backend + io::Write>, Box<dyn Error>>{
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn teardown_tui(mut term: Terminal<impl Backend + io::Write>) -> Result<(), Box<dyn Error>> {
    disable_raw_mode()?;
    execute!(term.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    term.show_cursor()?;

    Ok(())
}

fn run_app(term: &mut Terminal<impl Backend>, config: &mut Config, rows: Vec<Vec<Rgb8>>) -> Result<(), Box<dyn Error>> {
    let lines = {
        let mut lines: [Vec<Rgb8>; 3] = [rows[config.progress.row - 2].clone(), rows[config.progress.row - 1].clone(), vec![]];
        let row = &rows[config.progress.row];
        for x in 0..=(config.progress.col) {
            if x >= row.len() {
                continue
            }
            lines[2].push(row[x]);
        }
        lines
    };
    let mut app = App {
        scroll: ScrollbarState::default(),
        scroll_amount: lines[2].len(),
        lines,
        progress: &mut config.progress
    };
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();

    loop {
        term.draw(|f| ui(f, &mut app, &config.color_map))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('h') => app.scroll_amount -= 1,
                    KeyCode::Char('l') => app.scroll_amount += 1,
                    _ => {}
                }
                // handle input
            }
        }
        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }
}

fn ui(f: &mut Frame, app: &mut App, color_map: &ColorMap) {
    let layout = Layout::vertical([Constraint::Percentage(75), Constraint::Percentage(25)])
        .split(f.size());
    let text = app.lines.iter().map(
        |row| Line::from(row.iter().map(
                |c| Span::styled(color_map.map(*c), Color::Rgb(c.0[0], c.0[1], c.0[2]))
            ).collect::<Vec<_>>()
        )).collect::<Vec<_>>();
    app.scroll = app.scroll.content_length(app.lines.iter().map(|row| row.len()).max().unwrap_or(0));
    let para = Paragraph::new(text)
        .scroll((app.scroll_amount as u16, 0));
    f.render_widget(para, layout[0]);
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::HorizontalBottom),
        layout[0].inner(&Margin {
            vertical: 0,horizontal: 1,
        }),&mut app.scroll
    );

    f.render_widget(Paragraph::new("Hellow"), layout[1]);
}


fn print_grid(rows: Vec<Vec<Rgb8>>, color_map: &mut ColorMap) {
    for (row_idx, row) in rows.into_iter().enumerate() {
        if row_idx % 2 == 1 {
            print!(" ");
        }
        for p in row {
            let colored_p = color_map.map(p)
                .color(rgb8_to_true(p))
                .on_color(rgb8_to_true(SEPARATOR_COLOR));
            print!("{} ", colored_p);
        }
        println!();
    }
}

fn flood_fill(img: &mut RgbImage, (x, y): (u32, u32)) {
    if img[(x, y)].to_rgb8() == SEPARATOR_COLOR {
        return
    }
    img[(x, y)] = Rgb(SEPARATOR_COLOR.0);

    if x > 0 {
        flood_fill(img, (x - 1, y));
    }
    if y > 0 {
        flood_fill(img, (x, y - 1));
    }
    if x + 1 < img.width() {
        flood_fill(img, (x + 1, y));
    }
    if y + 1 < img.height() {
        flood_fill(img, (x, y + 1));
    }
}
