use std::pin::pin;
use std::{cell::RefCell, rc::Rc, sync::{LazyLock, Mutex}};
use yew_autoprops::autoprops;

use gloo_console::log;
use image::RgbImage;
use ipp::App;
use wasm_bindgen_futures::JsFuture;
use web_sys::{wasm_bindgen::UnwrapThrowExt, HtmlElement};
use yew::{platform::spawn_local, prelude::*};
use yew_hooks::prelude::*;

use gloo::events::EventListener;

thread_local! {
    static APP: Mutex<Option<App>> = Mutex::new(None);
}

struct Config {
    name: String,
    pub color_map: ipp::ColorMap,
    pub progress: ipp::Progress,
}
impl Config {
    pub fn load(file: Sting) -> Config {
        unimplemented!()
    }
}
struct InitializationState {
    pub image: RgbImage,
    pub config: Config,
}

#[function_component]
fn Main() -> Html {
    async fn file_callback(
        files: Option<web_sys::FileList>,
        image: UseStateHandle<Option<Rc<InitializationState>>>,
        reset_app: impl Fn()) {
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
            image.set(Some(Rc::new(InitializationState {
                    image: img,
                    config: Config::load(file.name()),
            })));
            reset_app();
        }
    }
    let drop_ref = use_node_ref();
    let image = use_state(|| None::<Rc<InitializationState>>);
    let ipp_app = use_state(|| None::<Rc<ipp::App>>);


    use_event_with_window("keypress", move |e: KeyboardEvent| {
        log!("{} is pressed!", e.key());
    });
    let ondrop = {
        // let image = Rc::new(image.clone());
        let image = image.clone();
        move |e: DragEvent| {
            let image = image.clone();
            e.prevent_default();
            log!("D2");
            let load_future = Box::pin(file_callback(e.data_transfer().expect_throw("no file").files(), image));
            spawn_local(load_future);
        }
    };
    let ondragover = move |e: DragEvent| e.prevent_default();

    html! {
        <div style="background-color: red;" ref={drop_ref} ondrop={ondrop} ondragover={ondragover}>
            if let Some(ref img) = &*image {
                <Ipp_App image={img} />
            }
            else {
                <Landing />
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
