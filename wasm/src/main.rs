use std::pin::pin;
use std::{cell::RefCell, rc::Rc, sync::{LazyLock, Mutex}};
use yew::functional;
use yew_autoprops::autoprops;

use gloo_console::log;
use image::RgbImage;
use ipp::Rgb8;
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
    pub image: RgbImage,
    pub config: Config,
}

#[autoprops]
#[function_component]
fn Initializer() -> Html {
    unimplemented!()
}

#[function_component]
fn Main() -> Html {
    async fn file_callback(
        files: Option<web_sys::FileList>,
        state: UseStateHandle<Rc<RefCell<AppState>>>) {
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
            let new_state = AppState::Initializing(InitializationState {
                    image: img,
                    config: Config::load(file.name()),
            });
            APP.with_borrow_mut(|state| *state = new_state);
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

    html! {
        <div style="background-color: red;" ref={drop_ref} ondrop={ondrop} ondragover={ondragover}>
        {
            match &**state {
                AppState::Uninitialized => html! { <Landing /> },
                AppState::Initializing(_init_state) => unimplemented!(),
                AppState::Running(_app) => unimplemented!(),
            }
        }
        </div>
    }
}


#[function_component]
fn Landing() -> Html {
    html! {
        <h1>{ "DROP IMAGE HERE" }</h1>
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
