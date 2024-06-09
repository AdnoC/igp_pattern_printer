use itertools::Itertools;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
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
        Progress { row: 2, col: 1 }
    }
    fn reset(&mut self) {
        self.row = 2;
        self.col = 1;
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

#[derive(Clone, Copy)]
enum NextPreview {
    Pixel(Option<Rgb8>),
    Tri([Option<Rgb8>; 3])
}
struct App<'a> {
    lines: Vec<Vec<Rgb8>>,
    rows: Vec<Vec<Rgb8>>,
    current_pixel: NextPreview,
    next_pixel: NextPreview,
    ensure_current_on_screen: bool,
    progress: &'a mut Progress,
}
impl<'a> App<'a> {
    fn initialize_lines(rows: &Vec<Vec<Rgb8>>, progress: &Progress) -> Vec<Vec<Rgb8>> {
        if progress.row < 3 {
            vec![
                rows[0].iter().take(progress.col + 1).cloned().collect(),
                rows[1].iter().take(progress.col).cloned().collect(),
                rows[2].iter().take(progress.col + 1).cloned().collect(),
            ]

        } else {
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
    }

    fn new(rows: Vec<Vec<Rgb8>>, progress: &'a mut Progress) -> App<'a> {
        use NextPreview::*;
        let lines = App::initialize_lines(&rows, progress);
        let next_pixel = if progress.row >= 3 {
            Pixel(rows[progress.row].get(progress.col).copied())
        } else {
            Tri([
                rows[0].get(progress.col + 1).copied(),
                rows[1].get(progress.col).copied(),
                rows[2].get(progress.col + 1).copied(),
            ])
        };
        let current_pixel = if progress.row >= 3 {
            Pixel(rows[progress.row].get(progress.col - 1).copied())
        } else {
            Tri([
                rows[0].get(progress.col).copied(),
                rows[1].get(progress.col - 1).copied(),
                rows[2].get(progress.col).copied(),
            ])
        };
        App {
            ensure_current_on_screen: false,
            lines,
            rows,
            current_pixel,
            next_pixel,
            progress,
        }

    }
}

// Lifecycle methods
impl<'a> App<'a> {
    fn tick(&mut self) {
        self.ensure_current_on_screen = true;
        self.progress.col += 1;
        self.current_pixel = self.next_pixel;
        if self.is_done_with_line() {
            self.progress.row += 1;
            self.progress.col = 0;
            self.lines.push(vec![]);
            self.current_pixel = NextPreview::Pixel(self.rows.get(self.progress.row).and_then(|row| row.get(0).copied()));
        }
        if self.progress.row < 3 {
            self.rows[0].get(self.lines[0].len()).map(|val| self.lines[0].push(*val));
            self.rows[1].get(self.lines[1].len()).map(|val| self.lines[1].push(*val));
            self.rows[2].get(self.lines[2].len()).map(|val| self.lines[2].push(*val));
        } else {
            if let Some(line) = self.lines.last_mut() {
                self.rows[self.progress.row]
                     .get(line.len())
                     .map(|val| line.push(*val));
            }
        }

        self.next_pixel = if self.progress.row >= 3 {
            NextPreview::Pixel(self.rows[self.progress.row].get(self.progress.col).copied())
        } else {
            NextPreview::Tri([
                self.rows[0].get(self.progress.col + 1).copied(),
                self.rows[1].get(self.progress.col).copied(),
                self.rows[2].get(self.progress.col + 1).copied(),
            ])
        };
    }

    fn reset(&mut self) {
        self.progress.reset();
        self.lines = App::initialize_lines(&self.rows, &self.progress);

    }

    fn is_done(&self) -> bool {
        self.progress.row >= (self.rows.len() - 1)
            && self.progress.col >= self.rows.last().map(|r| r.len()).unwrap_or(1) - 1
    }

    fn is_done_with_line(&self) -> bool {
        if self.progress.row < 3 {
            let max_len = self.rows[0].len().max(self.rows[1].len()).max(self.rows[2].len());
            self.progress.col >= max_len
        } else {
            self.progress.col >= self.rows[self.progress.row].len()
        }
    }
}

struct UIState {
    vertical_scroll: ScrollbarState,
    vertical_scroll_amount: usize,
    horizontal_scroll: ScrollbarState,
    horizontal_scroll_amount: usize,
}
impl UIState {
    fn new(app: &App) -> UIState {
        UIState {
            horizontal_scroll: ScrollbarState::new(app.rows.iter().map(|r| r.len()).max().unwrap()),
            horizontal_scroll_amount: (app.lines.last().unwrap().len() * 2).max(2) - 2,
            vertical_scroll: ScrollbarState::default(),
            vertical_scroll_amount: app.lines.len() - 3,
        }
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
    init_panic_hook();
    run_app(&mut term, &mut config, rows)?;
    config.save()?;
    term.show_cursor()?;
    teardown_tui()?;
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

fn teardown_tui() -> Result<(), Box<dyn Error>> {
    disable_raw_mode()?;
    execute!(
        io::stdout(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;

    Ok(())
}
fn init_panic_hook() {
    use std::panic::{set_hook, take_hook};
    let original_hook = take_hook();
    set_hook(Box::new(move |panic_info| {
        let _ = teardown_tui();
        original_hook(panic_info);
    }));
}

fn run_app(
    term: &mut Terminal<impl Backend>,
    config: &mut Config,
    rows: Vec<Vec<Rgb8>>,
) -> Result<(), Box<dyn Error>> {
    let mut app = App::new(rows, &mut config.progress);
    let mut ui_state = UIState::new(&app);
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();

    loop {
        term.draw(|f| ui(f, &mut app, &mut ui_state, &config.color_map))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Left | KeyCode::Char('h') => {
                        if ui_state.horizontal_scroll_amount > 0 {
                            ui_state.horizontal_scroll_amount -= 1
                        }
                    },
                    KeyCode::Down | KeyCode::Char('j') => ui_state.vertical_scroll_amount += 1,
                    KeyCode::Up | KeyCode::Char('k') => {
                        if ui_state.vertical_scroll_amount > 0 {
                            ui_state.vertical_scroll_amount -= 1
                        }
                    },
                    KeyCode::Right | KeyCode::Char('l') => ui_state.horizontal_scroll_amount += 1,
                    KeyCode::Char('r') => {
                        app.reset();
                    },
                    KeyCode::Char(' ') => {
                        if !app.is_done() {
                            app.tick()
                        }
                    },
                    KeyCode::Char('P') => { for _ in 0..30 { app.tick();} },
                    _ => {},
                }
                // handle input
            }
        }
        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }
}

fn ui(f: &mut Frame, app: &mut App, ui_state: &mut UIState, color_map: &ColorMap) {
    use ratatui::widgets::canvas::{Canvas, Rectangle, Map, MapResolution};
    use NextPreview::*;

    let main_layout = Layout::vertical([
        Constraint::Percentage(70),
        Constraint::Percentage(30),
        Constraint::Min(1),
    ]);
    let [image_frame, color_frame, instruction_line] = main_layout.areas(f.size());
    let colors_layout = Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]);
    let [current_color_box, next_color_box] = colors_layout.areas(color_frame);
    let tri_box_layout = Layout::vertical([Constraint::Ratio(1, 3), Constraint::Ratio(1, 3), Constraint::Ratio(1, 3)]);

    {
        if app.ensure_current_on_screen {
            // vertical
            {
                // Subtract 2 because we use 2 chars for the border
                let frame_size = image_frame.height as usize - 2;
                let content_length = app.lines.len();
                // Add 1 because we can't see whats behind the top-most border
                let current_scroll = ui_state.vertical_scroll_amount + 1;
                // Subtract 1 to account for the 1 we added earlier
                ui_state.vertical_scroll_amount = ensure_scroll_to_visible(frame_size, content_length, current_scroll) - 1;
            }
            // horizontal
            {
                // Subtract 2 because we use 2 chars for the border
                let frame_size = image_frame.width as usize - 2;
                let content_length = app.lines.last().map(|l| l.len()).unwrap_or(0) * 2;
                // Add 1 because we can't see whats behind the left-most border
                let current_scroll = ui_state.horizontal_scroll_amount + 1;
                // Subtract 1 to account for the 1 we added earlier
                ui_state.horizontal_scroll_amount = ensure_scroll_to_visible(frame_size, content_length, current_scroll) - 1;
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
    ui_state.vertical_scroll = ui_state
        .vertical_scroll
        .content_length(app.lines.len())
        .position(ui_state.vertical_scroll_amount);
    ui_state.horizontal_scroll = ui_state.horizontal_scroll.position(ui_state.horizontal_scroll_amount);

    let para = Paragraph::new(text).block(create_block("Pattern")).scroll((
        ui_state.vertical_scroll_amount as u16,
        ui_state.horizontal_scroll_amount as u16,
    ));
    f.render_widget(para, image_frame);
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::HorizontalBottom),
        image_frame.inner(&Margin {
            vertical: 0,
            horizontal: 1,
        }),
        &mut ui_state.horizontal_scroll,
    );
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight),
        image_frame.inner(&Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut ui_state.vertical_scroll,
    );

    let render_color_box = |f: &mut Frame, color: &Rgb8, bounds: &Rect, color_map: &ColorMap| {
        let canvas = Canvas::default()
            .block(create_block_owned(format!("Current link: {}", color_map.full_name(*color))))
            .background_color(Color::Rgb(color.0[0], color.0[1], color.0[2]))
            .x_bounds([
                0., bounds.width as f64
            ])
            .y_bounds([
                0., bounds.height as f64
            ])
            .paint(|_| { });
        f.render_widget(canvas, *bounds);
    };

    let render_single_pixel_preview = |f: &mut Frame, pixel: Option<Rgb8>, bounds: &Rect, empty_block_name: &'static str| {
        if let Some(current_color) = pixel {
            render_color_box(f, &current_color, bounds, color_map);
        } else {
            let para = Paragraph::new("End of line")
                .block(create_block(empty_block_name));
            f.render_widget(para, *bounds);
        }
    };
    let render_tri_pixel_preview = |f: &mut Frame, pixels: [Option<Rgb8>; 3], base_bounds: &Rect| {
        let tri_box: [Rect; 3] = tri_box_layout.areas(*base_bounds);

        for (bound, pixel) in tri_box.iter().zip(pixels.iter()) {
            if let Some(pixel) = pixel {
                render_color_box(f, pixel, bound, color_map);
            } else {
                let para = Paragraph::new("End of line")
                    .block(create_block("Link"));
                f.render_widget(para, *bound);
            }
        }
    };
    match app.current_pixel {
        Pixel(pixel) => render_single_pixel_preview(f, pixel, &current_color_box, "Current link"),
        Tri(pixels) => render_tri_pixel_preview(f, pixels, &current_color_box),
    }
    match app.next_pixel {
        Pixel(pixel) => render_single_pixel_preview(f, pixel, &next_color_box, "Next link"),
        Tri(pixels) => render_tri_pixel_preview(f, pixels, &next_color_box),
    }

    let controls = Line::from(
        "q: Quit | Space: Next link | arrows/h/j/k/l: Scroll left/down/up/right | r: Reset progress",
    );
    f.render_widget(controls, instruction_line);
}


fn ensure_scroll_to_visible(frame_size: usize, content_length: usize, current_scroll: usize) -> usize {
    let lowest_visible = current_scroll;
    let highest_visible = frame_size + current_scroll;
    let overscroll_padding = 2;
    // If the current char is below the scroll
    if lowest_visible > content_length {
        content_length - 1
    // If the current char is above the scroll
    // Add
    } else if highest_visible < content_length {
        content_length + 1 + overscroll_padding - frame_size
    } else {
        current_scroll
    }
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
