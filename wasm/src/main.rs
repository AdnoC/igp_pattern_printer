use std::{cell::RefCell, sync::{LazyLock, Mutex}};

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

#[function_component(Main)]
fn app() -> Html {
    let drop_ref = use_node_ref();
    let image = use_state(|| None::<RgbImage>);

    let image_clone = image.clone();
    let file_callback = async move |files: Option<web_sys::FileList>| {
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
            image_clone.set(Some(img));
        }
    };

    use_event_with_window("keypress", move |e: KeyboardEvent| {
        log!("{} is pressed!", e.key());
    });
    let ondrop = {
        move |e: DragEvent| {
            e.prevent_default();
            log!("D2");
            spawn_local(file_callback(e.data_transfer().expect_throw("no file").files()));
        }
    };
    let ondragover = move |e: DragEvent| e.prevent_default();

    html! {
        <div style="background-color: red;" ref={drop_ref} ondrop={ondrop} ondragover={ondragover}>
            if let Some(img) = &*image {
                <h1>{"HAVE IME"}</h1>
            }
            else {
                <Landing />
            }
        </div>
    }
}

#[function_component(Landing)]
fn landing() -> Html {
    html! {
        <h1>{ "DROP IMAGE HERE" }</h1>
    }
}


fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<Main>::new().render();
}
