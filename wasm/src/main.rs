use leptos::prelude::*;
use ipp::*;

#[component]
fn App() -> impl IntoView {
    let (image, set_image) = signal(None::<u8>);

    let set_image = move |val: u8| set_image.set(Some(val));
    view! {
        <Unstarted set_image=set_image />
    }
}

#[component]
fn Unstarted(set_image: impl FnOnce(u8)) -> impl IntoView {

    return view! {
        <div>
            Hello!!!!
        </div>
    }
}

fn main() {
    leptos::mount::mount_to_body(App)
}
