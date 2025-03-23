use std::pin::pin;
use std::{cell::RefCell, rc::Rc, sync::{LazyLock, Mutex}};
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

thread_local! {
    static APP: RefCell<AppState> = const { RefCell::new(AppState::Uninitialized) };
}

enum AppState {
    Uninitialized,
    Initializing(InitializationState),
    Running(ipp::App),
}

enum AppView {
    Uninitialized,
    Initializing{ new_color: Rgb8 },
    Running(AppSnapshot),
}
struct AppSnapshot {
    pub rows: Vec<Vec<Rgb8>>,
    pub current_pixel: ipp::NextPreview,
    pub next_pixel: ipp::NextPreview,
    pub ensure_current_on_screen: bool,
}
fn get_view() -> AppView {
    APP.with_borrow(|app| match app {
        AppState::Uninitialized => AppView::Uninitialized,
        AppState::Initializing(init_state) => {
            unimplemented!()
        },
        AppState::Running(app) => AppView::Running(AppSnapshot {
            rows: app.rows.clone(),
            current_pixel: app.current_pixel,
            next_pixel: app.next_pixel,
            ensure_current_on_screen: app.ensure_current_on_screen,
        }),
    })
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Config {
    name: String,
    pub color_map: ipp::ColorMap,
    pub progress: ipp::Progress,
}
impl Config {
    fn get_storage() -> Option<web_sys::Storage> {
        let window = web_sys::window()?;
        window.local_storage().expect_throw("Could not access local storage")
    }
    fn try_load(file: &str) -> Option<Config> {
        let storage = Config::get_storage()?;
        let config_str = storage.get_item(file).expect_throw("Could not retrieve value")?;
        ron::from_str(&config_str).ok()
    }
    pub fn load(file: String) -> Config {
        if let Some(config) = Config::try_load(&file) {
            config
        } else {
            Config {
                name: file,
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
struct InitializationState {
    pub row_builder: ipp::row_builder::RowBuilder,
    pub config: Config,
}

#[function_component]
fn Main() -> Html {
    async fn file_callback(
        files: Option<web_sys::FileList>,
        state: UseStateHandle<AppView>) {
        use ipp::row_builder::BuildState;

        let files = gloo::file::FileList::from(files.expect_throw("empty files"));
        for file in files.iter() {
            log!("File:", file.name());
            let data = gloo_file::futures::read_as_bytes(file)
                .await
                .expect_throw("read file");
            log!("Got data, {:?}", data.len());
            let img = image::load_from_memory(&data[..])
                .expect_throw("Could not load image");
            log!("img: {} x {}", img.width(), img.height());
            let img = img.to_rgb8();
            let mut row_builder = ipp::row_builder::RowBuilder::new(img);
            let mut config = Config::load(file.name());
            let (app_state, app_view) = match row_builder.build(&mut config.color_map) {
                BuildState::Complete(rows) => {
                    let app = ipp::App::new(rows, config.progress);
                    let snapshot = AppSnapshot {
                            rows: app.rows.clone(),
                            current_pixel: app.current_pixel,
                            next_pixel: app.next_pixel,
                            ensure_current_on_screen: app.ensure_current_on_screen,
                        };
                    (
                        AppState::Running(app),
                        AppView::Running(snapshot)
                    )
            },
                BuildState::NewColor(color) => (
                    AppState::Initializing(InitializationState {
                            row_builder,
                            config,
                    }),
                    AppView::Initializing { new_color: color }
                    ),
            };
            APP.with_borrow_mut(|state| *state = app_state);
            state.set(app_view);
        }
    }
    let drop_ref = use_node_ref();
    let state = use_state(|| get_view());


    use_event_with_window("keypress", move |e: KeyboardEvent| {
        log!("{} is pressed!", e.key());
    });
    let ondrop = {
        // let image = Rc::new(image.clone());
        let state = state.clone();
        move |e: DragEvent| {
            let state = state.clone();
            e.prevent_default();
            log!("D2");
            let load_future = Box::pin(file_callback(e.data_transfer().expect_throw("no file").files(), state));
            spawn_local(load_future);
        }
    };
    let ondragover = move |e: DragEvent| e.prevent_default();
    let initialize_color = {
        let state = state.clone();
        let initialize_color = move |entry: ColorEntry| {
            let state = state.clone();

            APP.with_borrow_mut(|app_state| {
                use ipp::row_builder::BuildState;

                match app_state {
                    AppState::Initializing(init_state) => {
                        let app_view = match init_state.row_builder.continue_build(entry, &mut init_state.config.color_map) {
                            BuildState::Complete(rows) => {
                                let app = ipp::App::new(rows, init_state.config.progress.clone());
                                let snapshot = AppSnapshot {
                                    rows: app.rows.clone(),
                                    current_pixel: app.current_pixel,
                                    next_pixel: app.next_pixel,
                                    ensure_current_on_screen: app.ensure_current_on_screen,
                                };
                                *app_state = AppState::Running(app);
                                AppView::Running(snapshot)
                            },
                            BuildState::NewColor(color) => AppView::Initializing { new_color: color },
                        };
                    },
                    _ => return,
                }
            });
        };
        Callback::from(initialize_color)
    };

    html! {
        <div style="background-color: red;" ref={drop_ref} ondrop={ondrop} ondragover={ondragover}>
        {
            match &*state {
                AppView::Uninitialized => html! { <Landing /> },
                AppView::Initializing{ new_color } => html! { <ColorPrompt color={*new_color} set_color={initialize_color} /> },
                AppView::Running(_app) => unimplemented!(),
            }
        }
        </div>
    }
}

#[autoprops]
#[function_component]
fn Hexagon(color: &Rgb8) -> Html {
    let size = 50;
    let style = vec![
        format!("background-color: {};", color.to_hex()),
        "clip-path: polygon(75% 0, 100% 50%, 75% 100%, 25% 100%, 0 50%, 25% 0);".to_string(),
        format!("height: {}px;", size * 9 / 10),
        format!("width: {}px;", size),
    ].join(" ");
    html! {
        <div style={style} />
    }
}

#[function_component]
fn Landing() -> Html {
    html! {
        <div>
            <h1>{ "DROP IMAGE HERE" }</h1>
            <Hexagon color={Rgb8([0, 0, 255])} />
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
                    let one_char = input
                        .value()
                        .chars()
                        .nth(0);
                    let one_char = if let Some(ch) = one_char {
                        ch.to_string()
                    } else {
                        log!("One-char descriptor empty");
                        return;
                    };

                    let entry = ColorEntry {
                        full_name: full_name.to_string(),
                        one_char,
                    };
                    fullname.set(None);
                    log!("Setting new color: {}, {}", &entry.full_name, &entry.one_char);
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
            <Hexagon color={*color} />
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
fn Ipp_App(image: Rc<RgbImage>) -> Html {
    
    unimplemented!()
}


fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<Main>::new().render();
}
