use implicit_clone::unsync::IArray;
use implicit_clone::ImplicitClone;
use std::pin::pin;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{LazyLock, Mutex},
};
use web_sys::HtmlInputElement;
use yew::functional;
use yew_autoprops::autoprops;

use gloo_console::log;
use image::RgbImage;
use ipp::{ColorEntry, Rgb8};
use wasm_bindgen_futures::JsFuture;
use web_sys::{wasm_bindgen::UnwrapThrowExt, HtmlElement};
use yew::{platform::spawn_local, prelude::*};
use yew_hooks::prelude::*;

use gloo::events::EventListener;

mod opfs;
mod svg;

thread_local! {
    static APP: RefCell<AppState> = const { RefCell::new(AppState::Uninitialized) };
}

const HEX_MARGIN: u32 = 2;
#[derive(Debug)]
enum AppState {
    Uninitialized,
    Initializing(InitializationState),
    Running(ipp::App, Config),
}

#[derive(Debug)]
enum AppView {
    Uninitialized,
    Initializing { new_color: Rgb8 },
    Running(AppSnapshot),
}
#[derive(Debug, PartialEq, Clone, ImplicitClone)]
struct AppSnapshot {
    pub rows: IArray<IArray<Pixel>>,
    pub current_pixel: NextPreview,
    pub next_pixel: NextPreview,
    pub ensure_current_on_screen: bool,
    pub hex_size: u32,
}
#[derive(Debug, PartialEq, Clone, ImplicitClone)]
struct Pixel {
    color: Rgb8,
    descriptor: Rc<str>,
}
#[derive(Clone, Debug, PartialEq, ImplicitClone)]
enum NextPreview {
    Pixel(Option<Pixel>),
    Tri([Option<Pixel>; 3]),
}
impl NextPreview {
    fn from_ipp(preview: ipp::NextPreview, color_map: &ipp::ColorMap) -> NextPreview {
        let map_pixel = |pixel: Rgb8| Pixel {
            color: pixel,
            descriptor: color_map.full_name(pixel),
        };
        match preview {
            ipp::NextPreview::Tri(pixels) => NextPreview::Tri([
                pixels[0].map(map_pixel),
                pixels[1].map(map_pixel),
                pixels[2].map(map_pixel),
            ]),
            ipp::NextPreview::Pixel(pixel) => NextPreview::Pixel(pixel.map(map_pixel)),
        }
    }
}

enum Direction {
    Up,
    Down,
}
#[derive(PartialEq, Clone, ImplicitClone)]
struct ControlCallbacks {
    change_hex_size: Callback<Direction>,
    next_tick: Callback<()>,
    reset_progress: Callback<()>,
}
fn get_view(app: &AppState) -> AppView {
    match app {
        AppState::Uninitialized => AppView::Uninitialized,
        AppState::Initializing(init_state) => {
            unimplemented!()
        }
        AppState::Running(app, config) => AppView::Running(AppSnapshot {
            rows: rows_to_iarray(&app.lines, &config.color_map),
            current_pixel: NextPreview::from_ipp(app.current_pixel, &config.color_map),
            next_pixel: NextPreview::from_ipp(app.next_pixel, &config.color_map),
            ensure_current_on_screen: app.ensure_current_on_screen,
            hex_size: config.hex_size,
        }),
    }
}

fn rows_to_iarray(rows: &Vec<Vec<Rgb8>>, color_map: &ipp::ColorMap) -> IArray<IArray<Pixel>> {
    IArray::from(
        rows.iter()
            .map(|row| {
                IArray::from(
                    row.iter()
                        .map(|c| Pixel {
                            color: *c,
                            descriptor: color_map.one_char(*c),
                        })
                        .collect::<Vec<Pixel>>(),
                )
            })
            .collect::<Vec<IArray<Pixel>>>(),
    )
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct Config {
    name: String,
    hex_size: u32,
    pub color_map: ipp::ColorMap,
    pub progress: ipp::Progress,
}
impl Config {
    fn get_storage() -> Option<web_sys::Storage> {
        let window = web_sys::window()?;
        window
            .local_storage()
            .expect_throw("Could not access local storage")
    }
    fn try_load(file: &str) -> Option<Config> {
        let storage = Config::get_storage()?;
        let config_str = storage
            .get_item(file)
            .expect_throw("Could not retrieve value")?;
        ron::from_str(&config_str).ok()
    }
    pub fn load(file: String) -> Config {
        if let Some(config) = Config::try_load(&file) {
            config
        } else {
            Config {
                name: file,
                hex_size: 50,
                color_map: ipp::ColorMap::new(),
                progress: ipp::Progress::new(),
            }
        }
    }
    fn try_save(&self) -> Option<()> {
        let storage = Config::get_storage()?;
        let config_str = ron::to_string(self).ok()?;
        storage.set_item(&self.name, &config_str).ok()
    }
    pub fn save(&self) {
        self.try_save().expect_throw("Could not save");
    }
}

#[derive(Debug)]
struct InitializationState {
    pub row_builder: ipp::row_builder::RowBuilder,
    pub config: Config,
}

fn load_file(data: &[u8], file_name: String, set_view: Callback<AppView>) {
    use ipp::row_builder::BuildState;

    let img = image::load_from_memory(data).expect_throw("Could not load image");
    log!("img: {} x {}", img.width(), img.height());
    let img = img.to_rgb8();
    let mut row_builder = ipp::row_builder::RowBuilder::new(img);
    let mut config = Config::load(file_name);
    let (app_state, app_view) = match row_builder.build(&mut config.color_map) {
        BuildState::Complete(rows) => {
            config.save();
            let app = ipp::App::new(rows, config.progress.clone());
            let snapshot = AppSnapshot {
                rows: rows_to_iarray(&app.lines, &config.color_map),
                current_pixel: NextPreview::from_ipp(app.current_pixel, &config.color_map),
                next_pixel: NextPreview::from_ipp(app.next_pixel, &config.color_map),
                ensure_current_on_screen: app.ensure_current_on_screen,
                hex_size: config.hex_size,
            };
            (AppState::Running(app, config), AppView::Running(snapshot))
        }
        BuildState::NewColor(color) => (
            AppState::Initializing(InitializationState {
                row_builder,
                config,
            }),
            AppView::Initializing { new_color: color },
        ),
    };
    APP.with_borrow_mut(|state| *state = app_state);
    set_view.emit(app_view)
}
#[function_component]
fn Main() -> Html {
    async fn file_callback(files: Option<web_sys::FileList>, set_view: Callback<AppView>) {
        let files = gloo::file::FileList::from(files.expect_throw("empty files"));
        for file in files.iter() {
            log!("File:", file.name());
            let data = gloo_file::futures::read_as_bytes(file)
                .await
                .expect_throw("read file");
            log!("Got data, {:?}", data.len());
            load_file(&data[..], file.name(), set_view.clone());
            opfs::save_file(&data[..], file.name()).await;
        }
    }
    let drop_ref = use_node_ref();
    let state = use_state(|| APP.with_borrow(|app| get_view(app)));

    let set_view = {
        let state = state.clone();
        Callback::from(move |view: AppView| state.set(view))
    };

    let ondrop = {
        // let image = Rc::new(image.clone());
        let set_view = set_view.clone();
        move |e: DragEvent| {
            let set_view = set_view.clone();
            e.prevent_default();
            log!("D2");
            let load_future = Box::pin(file_callback(
                e.data_transfer().expect_throw("no file").files(),
                set_view,
            ));
            spawn_local(load_future);
        }
    };
    let ondragover = move |e: DragEvent| e.prevent_default();
    let initialize_color = {
        let state = state.clone();
        let initialize_color = move |entry: ColorEntry| {
            let state = state.clone();
            log!("Initializing with entry: ", &entry.full_name);

            APP.with_borrow_mut(|app_state| {
                use ipp::row_builder::BuildState;

                match app_state {
                    AppState::Initializing(init_state) => {
                        let app_view = match init_state
                            .row_builder
                            .continue_build(entry, &mut init_state.config.color_map)
                        {
                            BuildState::Complete(rows) => {
                                init_state.config.save();
                                let app = ipp::App::new(rows, init_state.config.progress.clone());
                                let snapshot = AppSnapshot {
                                    rows: rows_to_iarray(&app.lines, &init_state.config.color_map),
                                    current_pixel: NextPreview::from_ipp(
                                        app.current_pixel,
                                        &init_state.config.color_map,
                                    ),
                                    next_pixel: NextPreview::from_ipp(
                                        app.next_pixel,
                                        &init_state.config.color_map,
                                    ),
                                    ensure_current_on_screen: app.ensure_current_on_screen,
                                    hex_size: init_state.config.hex_size,
                                };
                                *app_state = AppState::Running(app, init_state.config.clone());
                                AppView::Running(snapshot)
                            }
                            BuildState::NewColor(color) => {
                                AppView::Initializing { new_color: color }
                            }
                        };
                        state.set(app_view);
                    }
                    _ => return,
                }
            });
        };
        Callback::from(initialize_color)
    };

    let step_app = {
        let state = state.clone();
        let step_app = move |_| {
            APP.with_borrow_mut(|app_state| match app_state {
                AppState::Running(app, config) => {
                    app.tick();
                    config.progress = app.progress.clone();
                    config.save();
                    state.set(get_view(app_state));
                }
                _ => return,
            });
        };
        Callback::from(step_app)
    };

    let controls_callback = ControlCallbacks {
        change_hex_size: {
            let state = state.clone();
            Callback::from(move |dir: Direction| {
                APP.with_borrow_mut(|app_state| match app_state {
                    AppState::Running(_, config) => {
                        match dir {
                            Direction::Up => config.hex_size += 1,
                            Direction::Down => config.hex_size -= 1,
                        };
                        state.set(get_view(app_state));
                    },
                    _ => (),
                });
            })
        },
        next_tick: step_app,
        reset_progress: Callback::from(|_| {}),
    };

    html! {
        <div style="width: 100vw; height: 100vh;" ref={drop_ref} ondrop={ondrop} ondragover={ondragover}>
        {
            match &*state {
                AppView::Uninitialized => html! { <Landing set_view={set_view.clone()} /> },
                AppView::Initializing{ new_color } => html! { <ColorPrompt color={*new_color} set_color={initialize_color} /> },
                AppView::Running(app) => html! {
                    <IppApp
                        controls_callbacks={controls_callback}
                        app={app}
                    />
                },
            }
        }
        </div>
    }
}

fn hex_height(size: u32) -> u32 {
    size * 10 / 9
}
#[autoprops]
#[function_component]
fn Hexagon(color: &Rgb8, size: u32, name: Option<Rc<str>>) -> Html {
    // quick and dirty brightness check. Should replace with a more accurate version
    let font_color = if color.0[0] < 50 && color.0[1] < 50 && color.0[2] < 50 {
        "white"
    } else {
        "black"
    };
    let font_size = name
        .as_ref()
        .map(|n| n.len() + 1)
        .map(|mult| size / mult as u32)
        .unwrap_or(0);
    let style = vec![
        "display: inline-flex".to_string(),
        "justify-content: center".to_string(),
        "align-items: center".to_string(),
        format!("font-size: {}pt", font_size),
        format!("background-color: {}", color.to_hex()),
        format!("color: {}", font_color),
        "clip-path: polygon(0 75%, 50% 100%, 100% 75%, 100% 25%, 50% 0, 0 25%)".to_string(),
        format!("height: {}px", hex_height(size)),
        format!("width: {}px", size),
        format!("margin-right: {}px", HEX_MARGIN),
    ]
    .join("; ");
    html! {
        <div style={style} class="hexagon">
            <span clas="hex-text">
                {name.as_ref().map(|s| &**s).unwrap_or("")}
            </span>
        </div>
    }
}

#[autoprops]
#[function_component]
fn Landing(set_view: &Callback<AppView>) -> Html {
    let use_example_image = {
        let set_view = set_view.clone();
        move |_e: MouseEvent| {
            let mario = include_bytes!("../../Mario standing hex.bmp");
            load_file(mario, "Mario_Example.bmp".to_string(), set_view.clone());
        }
    };

    let load_previous_image = {
        let set_view = set_view.clone();
        let load_prev = move |_: MouseEvent| {
            async fn load(set_view: Callback<AppView>) {
                let data = opfs::load_file().await;
                log!("data len: ", data.0.len());
                load_file(&data.0[..], data.1, set_view);
            }
            spawn_local(Box::pin(load(set_view.clone())));
        };
        Callback::from(load_prev)
    };

    html! {
        <div>
            <h1>{ "DROP IMAGE HERE" }</h1>
            <button onclick={load_previous_image}>{"Load previously used image"}</button>
            <br />
            <button onclick={use_example_image}>{"Or click this to use an example image"}</button>
            <Hexagon size={50} color={Rgb8([0, 0, 255])} name={None::<Rc<str>>} />
        </div>
    }
}

#[autoprops]
#[function_component]
fn ColorPrompt(color: &Rgb8, set_color: &Callback<ColorEntry>) -> Html {
    let fullname = use_state(|| None::<String>);
    let onkeydown = {
        let fullname = fullname.clone();
        let set_color = set_color.clone();
        move |ev: KeyboardEvent| {
            if ev.key() == "Enter" {
                let input: HtmlInputElement = ev.target_unchecked_into();
                if let Some(full_name) = &*fullname {
                    let one_char = input.value();
                    if one_char.is_empty() {
                        log!("One-char descriptor empty");
                        return;
                    }

                    let entry = ColorEntry {
                        full_name: full_name.to_string(),
                        one_char,
                    };
                    fullname.set(None);
                    log!(
                        "Setting new color: {}, {}",
                        &entry.full_name,
                        &entry.one_char
                    );
                    set_color.emit(entry);
                } else {
                    fullname.set(Some(input.value()));
                }
            }
        }
    };
    html! {
        <div>
            <p>{"An unknown color was detected. Please give it a name"}</p>
            <p>{format!("Hex code: {}", color.to_hex())}</p>
            <Hexagon size={50} color={*color} name={None::<Rc<str>>} />
            <input type="text" placeholder="Orange, Blue, etc..." onkeydown={onkeydown.clone()} />
            if fullname.is_some() {
                <p>{"Please give a one-letter descriptor for your color"}</p>
                <input type="text" placeholder="O, B, etc..." onkeydown={onkeydown} />
            }
        </div>
    }
}

#[autoprops]
#[function_component]
fn Preview(name: &String, preview: &NextPreview) -> Html {
    let header_style = vec![
        "margin-bottom: 0".to_string(),
        "margin-top: 5px".to_string(),
    ]
    .join("; ");
    match preview {
        NextPreview::Pixel(Some(pixel)) => {
            log!("Have preview");
            html! {
                <div class="preview">
                    <h3>{name}</h3>
                    <div>{pixel.descriptor.clone()}</div>
                    <Hexagon size={30} color={pixel.color} name={None::<Rc<str>>} />
                </div>
            }
        }
        NextPreview::Tri([Some(p1), Some(p2), Some(p3)]) => {
            html! {
                <div class="preview">
                    <h3 style={header_style}>{name}</h3>
                    <div class="preview-tri-container">
                        <div class="preview-tri-content">
                            <div class="preview-color-name">{p1.descriptor.clone()}</div>
                            <div class="preview-color-name">{p2.descriptor.clone()}</div>
                            <div class="preview-color-name">{p3.descriptor.clone()}</div>
                        </div>
                        <div class="preview-tri-content">
                            <Hexagon size={30} color={p1.color} name={None::<Rc<str>>} />
                            <Hexagon size={30} color={p2.color} name={None::<Rc<str>>} />
                            <Hexagon size={30} color={p3.color} name={None::<Rc<str>>} />
                        </div>
                    </div>
                </div>
            }
        }
        _ => {
            log!("No preview pixel");
            html! {
                <div>

                </div>
            }
        }
    }
}

#[autoprops]
#[function_component]
fn IppApp(app: &AppSnapshot, controls_callbacks: &ControlCallbacks) -> Html {
    let step = controls_callbacks.next_tick.clone();
    use_event_with_window("keypress", move |e: KeyboardEvent| {
        log!("Key pressed: ", e.code());
        match e.code().as_str() {
            "Space" => {
                e.prevent_default();
                step.emit(());
            }
            _ => (),
        }
    });
    let next_tick = {
        let controls_callbacks = controls_callbacks.clone();
        let next_tick = move |_: MouseEvent| {
            controls_callbacks.next_tick.emit(());
        };
        Callback::from(next_tick)
    };
    let size_up = {
        let controls_callbacks = controls_callbacks.clone();
        let size_up = move |_: MouseEvent| {
            controls_callbacks.change_hex_size.emit(Direction::Up);
        };
        Callback::from(size_up)
    };
    let size_down = {
        let controls_callbacks = controls_callbacks.clone();
        let size_down = move |_: MouseEvent| {
            controls_callbacks.change_hex_size.emit(Direction::Down);
        };
        Callback::from(size_down)
    };
    html! {
            <BodyWithControls body={ html! { <ImageDisplay hex_size={app.hex_size} rows={app.rows.clone()} /> }}>
                <Preview name="Current" preview={app.current_pixel.clone()} />
                <Preview name="Next" preview={app.next_pixel.clone()} />
                <div class="size-up-down-container">
                <button class="size-up-down-btn" onclick={size_up}>{"+"}</button>
                <button class="size-up-down-btn" onclick={size_down}>{"-"}</button>
                </div>
                <button class="next-step-btn" onclick={next_tick}>{"Next Link"}</button>
            </BodyWithControls>
    }
}

#[autoprops]
#[function_component]
fn BodyWithControls(body: &Html, children: &Html) -> Html {
    let translation = use_state(|| (0, 0));
    let tranform_origin = use_state(|| (0, 0));
    let scale = use_state(|| 1.0);
    let is_mouse_down = use_state(|| false);
    let container_style = vec![
        "overflow: hidden".to_string(),
        "display: flex".to_string(),
        "flex-direction: column".to_string(),
        "height: 100%".to_string(),
    ]
    .join("; ");
    let controls_style = vec![
        "height: 128px".to_string(),
        "position: relative".to_string(),
        "z-index: 5".to_string(),
        "background-color: white".to_string(),
        "display: flex".to_string(),
        "border-style: inset".to_string(),
    ]
    .join("; ");
    let body_style = vec![
        "position: relative".to_string(),
        "flex-grow: 1".to_string(),
    ]
    .join("; ");
    let inner_style = vec![
        "position: relative".to_string(),
        format!("transform: translate3d({}px, {}px, 0px) scale({})", translation.0, translation.1, *scale),
    ]
    .join("; ");

    let onmousedown = {
        let is_mouse_down = is_mouse_down.clone();
        move |e: MouseEvent| {
            e.prevent_default();
            is_mouse_down.set(true);
        }
    };

    let onmouseup = {
        let is_mouse_down = is_mouse_down.clone();
        move |e: MouseEvent| {
            e.prevent_default();
            is_mouse_down.set(false);
        }
    };
    let onmousemove = {
        let translation = translation.clone();
        move |e: MouseEvent| {
            const MOUSE_PRIMARY: u16 = 1;
            e.prevent_default();
            if e.buttons() & MOUSE_PRIMARY == 1 {
                let trans = *translation;
                translation.set((trans.0 + e.movement_x(), trans.1 + e.movement_y()));
            }
        }
    };
    let onwheel = {
        let scale = scale.clone();
        move |e: web_sys::WheelEvent| {
            e.stop_propagation();
            let scroll_scaler = if e.delta_y() > 0. { 0.9 } else { 1.1 };
            scale.set(*scale * scroll_scaler);
        }
    };

    html! {
        <div style={container_style}>
            <div id="controls" style={controls_style}>
                { children.clone() }
            </div>
            <div 
                id="app-body" 
                style={body_style}
                onmousedown={onmousedown}
                onmouseup={onmouseup}
                onmousemove={onmousemove}
                onwheel={onwheel}
            >
                <div style="position: absolute;">
                    <div style={inner_style}>
                        { body.clone() }
                    </div>
                </div>
            </div>
        </div>
    }
}
#[autoprops]
#[function_component]
fn DragableBox(children: &Html) -> Html {
    let pos = use_state(|| (0, 0));
    let start_pos = use_state(|| None::<(i32, i32)>);
    let box_ref = NodeRef::default();
    let container_style = vec![
        "display: flex".to_string(),
        "position: fixed".to_string(),
        format!("left: {}px", pos.0),
        format!("top: {}px", pos.1),
        "background-color: white".to_string(),
        "z-index: 5".to_string(),
        "padding: 5px".to_string(),
        "border: 3px".to_string(),
        "border-style: ridge".to_string(),
    ]
    .join("; ");
    let dragger_style = vec![
        "display: inline-block".to_string(),
        "background-color: rgb(215, 215, 215)".to_string(),
        "padding: 7px".to_string(),
        "margin: 5px".to_string(),
        "width: fit-content".to_string(),
        "height: fit-content".to_string(),
    ]
    .join("; ");

    let onmousedown = {
        let start_pos = start_pos.clone();
        let box_ref = box_ref.clone();
        move |e: MouseEvent| {
            e.prevent_default();
            if let Some(box_elem) = box_ref.cast::<HtmlElement>() {
                let rect = box_elem.get_bounding_client_rect();
                start_pos.set(Some((
                    rect.left() as i32 - e.screen_x(),
                    rect.top() as i32 - e.screen_y(),
                )));
            }
        }
    };

    let onmouseup = {
        let start_pos = start_pos.clone();
        move |e: MouseEvent| {
            e.prevent_default();
            start_pos.set(None);
        }
    };
    let onmousemove = {
        let pos = pos.clone();
        let start_pos = start_pos.clone();
        move |e: MouseEvent| {
            const MOUSE_PRIMARY: u16 = 1;
            e.prevent_default();
            if e.buttons() & MOUSE_PRIMARY == 0 {
                start_pos.set(None);
            }
            if let Some(start_pos) = *start_pos {
                //e.prevent_default();
                /*log!("Dragging plus.", e.type_());
                log!("x=", e.x(), " y=", e.y());
                log!("x=", e.client_x(), " y=", e.client_y());
                log!("x=", e.screen_x(), " y=", e.screen_y());
                log!("x=", e.offset_x(), " y=", e.offset_y());*/
                pos.set((e.screen_x() + start_pos.0, e.screen_y() + start_pos.1));
            }
        }
    };
    html! {
        <div
            onmousemove={onmousemove}
            style={container_style}
            ref={box_ref.clone()}
        >
            <div
                onmousedown={onmousedown}
                onmouseup={onmouseup}
                style={dragger_style}
            >
                <svg::DragPlus size={50} />
            </div>
            { children.clone() }
        </div>
    }
}

fn hex_row_style(hex_size: u32, idx: usize) -> String {
    let hex_height = hex_height(hex_size);
    let position = (hex_height * 3 / 4 + HEX_MARGIN) * idx as u32;
    vec![
        "position: absolute".to_string(),
        format!("top: {}px", position),
        "text-wrap: nowrap".to_string(),
    ]
    .join("; ")
}
#[autoprops]
#[function_component]
fn ImageDisplay(rows: IArray<IArray<Pixel>>, hex_size: u32) -> Html {
    let hex_rows = rows
        .iter()
        .map(|row| row.iter().map(|pixel| html! { <Hexagon size={hex_size} color={pixel.color} name={Some(pixel.descriptor)} /> }));

    let stagger_style = vec![
        "display: inline-block".to_string(),
        format!("width: {}px", hex_size / 2),
    ]
    .join("; ");
    let stagger_style: Rc<str> = Rc::from(stagger_style.as_ref());
    let hex_rows = hex_rows.enumerate().map(|(idx, row)| {
        html! {
            <div class="hex-row" style={hex_row_style(hex_size, idx)}>
                if idx % 2 == 1 {
                    <div style={stagger_style.clone()}>
                    </div>
                }
                {row.collect::<Html>()}
            </div>
        }
    });
    html! {
        <div>
            {hex_rows.collect::<Html>()}
        </div>
    }
}

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<Main>::new().render();
}
