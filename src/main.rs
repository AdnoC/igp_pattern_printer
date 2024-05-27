use std::{
    ffi::OsStr,
    fs,
    io,
    time::{Duration, Instant},
    path::{Path, PathBuf},
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
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
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
    full_names: HashMap<Rgb8, String>,
    short_char: HashMap<Rgb8, String>,
}

impl ColorMap {
    fn new() -> ColorMap {
        ColorMap {
            full_names: HashMap::new(),
            short_char: HashMap::new(),
        }
    }

    fn ensure_mapped(&mut self, color: Rgb8) -> Result<(), Box<dyn Error>> {
        use io::Write;

        if self.full_names.contains_key(&color) {
            return Ok(())
        }
        let colored_rgb = format!("{:?}", color)
            .color(rgb8_to_true(color))
            .on_color(rgb8_to_true(SEPARATOR_COLOR));
        println!("Found new color: {}", colored_rgb);
        print!("Please give it a name: ");
        io::stdout().flush()?;
        let mut name = String::new();
        io::stdin().read_line(&mut name)?;
        self.full_names.insert(color, name.trim().to_owned());
        print!("Please give it a 1 character description: ");
        io::stdout().flush()?;
        io::stdin().read_line(&mut name)?;
        self.short_char.insert(color, name.trim().chars().nth(0).unwrap().to_string());
        Ok(())
    }

    fn full_name(&self, color: Rgb8) -> &str {
        &self.full_names[&color]
    }

    fn one_char(&self, color: Rgb8) -> &str {
        &self.short_char[&color]
    }
}

#[derive(Serialize, Deserialize, Hash, Eq, PartialEq, PartialOrd, Clone, Debug)]
struct Progress {
    row: usize,
    col: usize,
}
impl Progress {
    fn new() -> Self {
        Progress {
            row: 7, col: 10
        }
    }
    fn reset(&mut self) {
        self.row = 7;
        self.col = 10;
    }
}

#[derive(Serialize, Deserialize)]
struct Config {
    config_path: PathBuf,
    color_map: ColorMap,
    progress: Progress,
}

impl Config {
    fn load(project_dir: PathBuf, pattern_file: impl AsRef<Path>) -> Result<Config, Box<dyn Error>> {
        let pattern_path = pattern_file.as_ref();
        let mut config_filename = pattern_path.file_name().unwrap().to_owned();
        config_filename.push(OsStr::new(".config.ron"));
        let config_file = pattern_path.with_file_name(config_filename);
        let config_path = project_dir.join(config_file);

        if !project_dir.exists() {
            fs::create_dir_all(project_dir)?;
        }

        let mut config: Config = fs::read_to_string(&config_path).ok()
            .and_then(|s| ron::from_str(&s).ok())
            .unwrap_or(Config {
                config_path: config_path.clone(),
                color_map: ColorMap::new(),
                progress: Progress::new(),
            });
        config.config_path = config_path;


        Ok(config)
    }

    fn save(&self) -> Result<(), Box<dyn Error>> {
        fs::write(&self.config_path, ron::to_string(&self)?)?;
        Ok(())

    }
}

struct App<'a> {
    lines: Vec<Vec<Rgb8>>,
    vertical_scroll: ScrollbarState,
    vertical_scroll_amount: usize,
    horizontal_scroll: ScrollbarState,
    horizontal_scroll_amount: usize,
    ensure_current_on_screen: bool,
    progress: &'a mut Progress,
}
impl<'a> App<'a> {
    fn tick(&mut self, rows: &Vec<Vec<Rgb8>>) {
        self.ensure_current_on_screen = true;
        self.progress.col += 1;
        if self.progress.col >= rows[self.progress.row].len() {
            self.progress.row += 1;
            self.progress.col = 0;
            self.lines.push(vec![]);
        }
        self.lines.last_mut().unwrap().push(rows[self.progress.row][self.progress.col]);
    }

    fn is_done(&self, rows: &Vec<Vec<Rgb8>>) -> bool {
        self.progress.row >= (rows.len() - 1) && self.progress.col >= rows.last().map(|r| r.len()).unwrap_or(1) - 1
    }
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
    let mut config = Config::load(project_dir, Path::new(&file))?;

    let img = ImageReader::open(file)?.decode()?.to_rgb8();

    let rows = build_rows(img, &mut config.color_map)?;
    config.save()?;

    //print_grid(rows, &mut config.color_map);
    let mut term = setup_tui()?;
    run_app(&mut term, &mut config, rows)?;
    config.save()?;
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
    let stdout = io::stdout();
    let mut backend = CrosstermBackend::new(stdout);
    execute!(backend, EnterAlternateScreen, EnableMouseCapture)?;
    backend.hide_cursor()?;
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
    fn initialize_lines(progress: &Progress, rows: &Vec<Vec<Rgb8>>) -> Vec<Vec<Rgb8>> {
        let mut lines: Vec<Vec<Rgb8>> = rows.iter().take(progress.row - 1).cloned().collect();
        lines.push(rows[progress.row - 1].iter().take(progress.col).cloned().collect());
        lines
    }
    let lines = initialize_lines(&config.progress, &rows);
    let mut app = App {
        horizontal_scroll: ScrollbarState::new(rows.iter().map(|r| r.len()).max().unwrap()),
        horizontal_scroll_amount: 0, //lines.last().unwrap().len(),
        vertical_scroll: ScrollbarState::default(),
        vertical_scroll_amount: 0, //lines.len(),
        ensure_current_on_screen: false,
        lines,
        progress: &mut config.progress
    };
    let tick_rate = Duration::from_millis(550);
    let mut last_tick = Instant::now();

    loop {
        term.draw(|f| ui(f, &mut app, &config.color_map))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('h') => if app.horizontal_scroll_amount > 0{
                        app.horizontal_scroll_amount -= 1
                    },
                    KeyCode::Char('j') => app.vertical_scroll_amount += 1,
                    KeyCode::Char('k') => if app.vertical_scroll_amount > 0 {
                        app.vertical_scroll_amount -= 1
                    },
                    KeyCode::Char('l') => app.horizontal_scroll_amount += 1,
                    KeyCode::Char('r') => {
                        app.progress.reset();
                        app.lines = initialize_lines(&app.progress, &rows);
                    },
                    KeyCode::Char(' ') => if key.kind == KeyEventKind::Press && !app.is_done(&rows) { app.tick(&rows) },
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

    {
        if app.ensure_current_on_screen {
            let visible_rows = layout[0].height as usize;
            let total_rows = app.lines.len();
            let current_scroll = app.vertical_scroll_amount;
            let top_visible_row = current_scroll;
            let bottom_visible_row = visible_rows + current_scroll;
            // If the current row is above the screen
            if top_visible_row > total_rows {
                app.vertical_scroll_amount = total_rows - 1;
            // If the current row is below the screen
            } else if bottom_visible_row < total_rows {
                app.vertical_scroll_amount = total_rows - visible_rows + 1;
            }

        }
        app.ensure_current_on_screen = false;
    }

    let text = app.lines.iter().map(
        |row| Line::from(row.iter().map(
                |c| Span::styled(color_map.one_char(*c), Color::Rgb(c.0[0], c.0[1], c.0[2]))
            ).collect::<Vec<_>>()
        )).collect::<Vec<_>>();
    app.vertical_scroll = app.vertical_scroll
        .content_length(app.lines.len())
        .position(app.vertical_scroll_amount);
    app.horizontal_scroll = app.horizontal_scroll.position(app.horizontal_scroll_amount);

    let para = Paragraph::new(text)
        .scroll((app.vertical_scroll_amount as u16, app.horizontal_scroll_amount as u16));
    f.render_widget(para, layout[0]);
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::HorizontalBottom),
        layout[0].inner(&Margin {
            vertical: 0,horizontal: 1,
        }),&mut app.horizontal_scroll
    );
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight),
        layout[0].inner(&Margin {
            vertical: 1,horizontal: 0,
        }),&mut app.vertical_scroll
    );

    f.render_widget(Paragraph::new("Hellow"), layout[1]);
}


fn print_grid(rows: Vec<Vec<Rgb8>>, color_map: &mut ColorMap) {
    for (row_idx, row) in rows.into_iter().enumerate() {
        if row_idx % 2 == 1 {
            print!(" ");
        }
        for p in row {
            let colored_p = color_map.one_char(p)
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

fn append_to_log<T: ToString>(s: T) -> Result<(), Box<dyn Error>> {
    use std::fs::OpenOptions;
    use std::io::prelude::*;

    let mut file = OpenOptions::new()
        .write(true)
        .append(true)
        .create(true)
        .open("log")?;


    writeln!(file, "{}", s.to_string())?;
    Ok(())
}
