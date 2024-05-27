use itertools::Itertools;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use directories::ProjectDirs;
use image::{io::Reader as ImageReader, Rgb, RgbImage};
use ratatui::{prelude::*, symbols::scrollbar, widgets::*};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    error::Error,
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

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
        use colored::Colorize;
        use io::Write;

        if self.full_names.contains_key(&color) {
            return Ok(());
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
        self.short_char
            .insert(color, name.trim().chars().nth(0).unwrap().to_string());
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
        Progress { row: 3, col: 0 }
    }
    fn reset(&mut self) {
        self.row = 3;
        self.col = 0;
    }
}

#[derive(Serialize, Deserialize)]
struct Config {
    config_path: PathBuf,
    color_map: ColorMap,
    progress: Progress,
}

impl Config {
    fn load(
        project_dir: PathBuf,
        pattern_file: impl AsRef<Path>,
    ) -> Result<Config, Box<dyn Error>> {
        let pattern_path = pattern_file.as_ref();
        let mut config_filename = pattern_path.file_name().unwrap().to_owned();
        config_filename.push(OsStr::new(".config.ron"));
        let config_file = pattern_path.with_file_name(config_filename);
        let config_path = project_dir.join(config_file);

        if !project_dir.exists() {
            fs::create_dir_all(project_dir)?;
        }

        let mut config: Config = fs::read_to_string(&config_path)
            .ok()
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
    next_pixel: Option<Rgb8>,
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
        self.lines
            .last_mut()
            .unwrap()
            .push(rows[self.progress.row][self.progress.col]);
        self.next_pixel = if self.progress.col + 1 < rows[self.progress.row].len() {
            Some(rows[self.progress.row][self.progress.col + 1])
        } else {
            None
        };
    }

    fn is_done(&self, rows: &Vec<Vec<Rgb8>>) -> bool {
        self.progress.row >= (rows.len() - 1)
            && self.progress.col >= rows.last().map(|r| r.len()).unwrap_or(1) - 1
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

fn build_rows(
    mut img: RgbImage,
    color_map: &mut ColorMap,
) -> Result<Vec<Vec<Rgb8>>, Box<dyn Error>> {
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

fn setup_tui() -> Result<Terminal<impl Backend + io::Write>, Box<dyn Error>> {
    enable_raw_mode()?;
    let stdout = io::stdout();
    let mut backend = CrosstermBackend::new(stdout);
    execute!(backend, EnterAlternateScreen, EnableMouseCapture)?;
    backend.hide_cursor()?;
    Ok(Terminal::new(backend)?)
}

fn teardown_tui(mut term: Terminal<impl Backend + io::Write>) -> Result<(), Box<dyn Error>> {
    disable_raw_mode()?;
    execute!(
        term.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    term.show_cursor()?;

    Ok(())
}

fn run_app(
    term: &mut Terminal<impl Backend>,
    config: &mut Config,
    rows: Vec<Vec<Rgb8>>,
) -> Result<(), Box<dyn Error>> {
    fn initialize_lines(progress: &Progress, rows: &Vec<Vec<Rgb8>>) -> Vec<Vec<Rgb8>> {
        let mut lines: Vec<Vec<Rgb8>> = rows.iter().take(progress.row).cloned().collect();
        lines.push(
            rows[progress.row - 1]
                .iter()
                .take(progress.col + 1)
                .cloned()
                .collect(),
        );
        lines
    }
    let lines = initialize_lines(&config.progress, &rows);
    let next_pixel = if config.progress.col + 1 < rows[config.progress.row].len() {
        Some(rows[config.progress.row][config.progress.col + 1])
    } else {
        None
    };
    let mut app = App {
        horizontal_scroll: ScrollbarState::new(rows.iter().map(|r| r.len()).max().unwrap()),
        horizontal_scroll_amount: 0, //lines.last().unwrap().len(),
        vertical_scroll: ScrollbarState::default(),
        vertical_scroll_amount: 0, //lines.len(),
        ensure_current_on_screen: false,
        lines,
        next_pixel,
        progress: &mut config.progress,
    };
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();

    loop {
        term.draw(|f| ui(f, &mut app, &config.color_map))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('h') => {
                        if app.horizontal_scroll_amount > 0 {
                            app.horizontal_scroll_amount -= 1
                        }
                    }
                    KeyCode::Char('j') => app.vertical_scroll_amount += 1,
                    KeyCode::Char('k') => {
                        if app.vertical_scroll_amount > 0 {
                            app.vertical_scroll_amount -= 1
                        }
                    }
                    KeyCode::Char('l') => app.horizontal_scroll_amount += 1,
                    KeyCode::Char('r') => {
                        app.progress.reset();
                        app.lines = initialize_lines(&app.progress, &rows);
                    }
                    KeyCode::Char(' ') => {
                        if !app.is_done(&rows) {
                            app.tick(&rows)
                        }
                    }
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
    use ratatui::widgets::canvas::{Canvas, Rectangle, Map, MapResolution};

    let main_layout = Layout::vertical([
        Constraint::Percentage(70),
        Constraint::Percentage(30),
        Constraint::Min(1),
    ]);
    let [image_frame, color_frame, instruction_line] = main_layout.areas(f.size());
    let colors_layout = Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]);
    let [current_color_box, next_color_box] = colors_layout.areas(color_frame);

    {
        if app.ensure_current_on_screen {
            // vertical
            {
                let visible_rows = image_frame.height as usize;
                let total_rows = app.lines.len();
                let current_scroll = app.vertical_scroll_amount;
                let top_visible_row = current_scroll;
                let bottom_visible_row = visible_rows + current_scroll;
                // If the current row is above the screen
                if top_visible_row + 2 > total_rows {
                    app.vertical_scroll_amount = total_rows - 1;
                // If the current row is below the screen
                } else if bottom_visible_row < total_rows + 2 {
                    app.vertical_scroll_amount = total_rows - visible_rows + 2;
                }
            }
            // horizontal
            {
                let visible_rows = image_frame.width as usize;
                let total_rows = app.lines.last().map(|l| l.len()).unwrap_or(0) * 2;
                let current_scroll = app.horizontal_scroll_amount;
                let top_visible_row = current_scroll;
                let bottom_visible_row = visible_rows + current_scroll;
                append_to_log(format!("vis: {}, tot: {}, cur: {}, top: {}, bot: {}", visible_rows, total_rows, current_scroll, top_visible_row, bottom_visible_row)).unwrap();
                // If the current row is above the screen
                if top_visible_row > total_rows {
                    app.horizontal_scroll_amount = total_rows - 1;
                // If the current row is below the screen
                } else if bottom_visible_row < total_rows {
                    app.horizontal_scroll_amount = total_rows - visible_rows + 1;
                }
            }
        }
        app.ensure_current_on_screen = false;
    }

    let create_block = |title: &'static str| Block::bordered().gray().title(title.bold());
    let create_block_owned = |title: String| Block::bordered().gray().title(title.bold());

    let text = app
        .lines
        .iter()
        .enumerate()
        .map(|(row_idx, row)| {
            let mut line = row.iter()
                .map(|c| {
                    Span::styled(color_map.one_char(*c), Color::Rgb(c.0[0], c.0[1], c.0[2]))
                })
                .intersperse(Span::raw(" "))
                .collect::<Vec<_>>();
            if row_idx % 2 == 1 {
                line.insert(0, Span::raw(" "));
            }
            Line::from(line)
        })
        .collect::<Vec<_>>();
    app.vertical_scroll = app
        .vertical_scroll
        .content_length(app.lines.len())
        .position(app.vertical_scroll_amount);
    app.horizontal_scroll = app.horizontal_scroll.position(app.horizontal_scroll_amount);

    let para = Paragraph::new(text).block(create_block("Pattern")).scroll((
        app.vertical_scroll_amount as u16,
        app.horizontal_scroll_amount as u16,
    ));
    f.render_widget(para, image_frame);
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::HorizontalBottom),
        image_frame.inner(&Margin {
            vertical: 0,
            horizontal: 1,
        }),
        &mut app.horizontal_scroll,
    );
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight),
        image_frame.inner(&Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut app.vertical_scroll,
    );

    let current_color = app.lines.last().and_then(|r| r.last()).unwrap();
    let current_canvas = Canvas::default()
        .block(create_block_owned(format!("Current link: {}", color_map.full_name(*current_color))))
        .background_color(Color::Rgb(current_color.0[0], current_color.0[1], current_color.0[2]))
        .x_bounds([
            0., current_color_box.width as f64
        ])
        .y_bounds([
            0., current_color_box.height as f64
        ])
        .paint(|_| { });
    f.render_widget(current_canvas, current_color_box);

    if let Some(next_color) = app.next_pixel {
        let nc = next_color.0.clone();
        let next_canvas = Canvas::default()
            .block(create_block_owned(format!("Next link: {}", color_map.full_name(next_color))))
            .background_color(Color::Rgb(nc[0], nc[1], nc[2]))
            .x_bounds([
            0., next_color_box.width as f64
            ])
            .y_bounds([
            0., next_color_box.height as f64
            ])
            .paint(move |_| { });
        f.render_widget(next_canvas, next_color_box);
    } else {
        let next_para = Paragraph::new("End of line")
            .block(create_block("Next link"));
        f.render_widget(next_para, next_color_box);
    }


    let controls = Line::from(
        "q: Quit | Space: Next link | h/j/k/l: Scroll left/down/up/right | r: Reset progress",
    );
    f.render_widget(controls, instruction_line);
}

fn print_grid(rows: Vec<Vec<Rgb8>>, color_map: &mut ColorMap) {
    use colored::Colorize;
    for (row_idx, row) in rows.into_iter().enumerate() {
        if row_idx % 2 == 1 {
            print!(" ");
        }
        for p in row {
            let colored_p = color_map
                .one_char(p)
                .color(rgb8_to_true(p))
                .on_color(rgb8_to_true(SEPARATOR_COLOR));
            print!("{} ", colored_p);
        }
        println!();
    }
}

fn flood_fill(img: &mut RgbImage, (x, y): (u32, u32)) {
    if img[(x, y)].to_rgb8() == SEPARATOR_COLOR {
        return;
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
