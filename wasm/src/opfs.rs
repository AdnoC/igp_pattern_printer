use gloo_console::log;
use js_sys::Uint8Array;
use wasm_bindgen::{JsCast, UnwrapThrowExt};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    File, FileSystemDirectoryHandle, FileSystemFileHandle, FileSystemGetFileOptions, WritableStream,
};

const LAST_USED_NAME: &str = "PREV_IMAGE";
const LAST_USED_IMAGE_NAME: &str = "PREV_IMAGE_NAME";
async fn get_file(name: &str) -> FileSystemFileHandle {
    let dir = web_sys::window()
        .expect_throw("Could not retrieve window")
        .navigator()
        .storage()
        .get_directory();
    let dir = JsFuture::from(dir)
        .await
        .expect_throw("get_directory promise rejected")
        .dyn_into::<FileSystemDirectoryHandle>()
        .expect_throw("Could not cast into file system directory handle");

    let file_options = FileSystemGetFileOptions::new();
    file_options.set_create(true);
    let file = JsFuture::from(dir.get_file_handle_with_options(name, &file_options))
        .await
        .expect_throw("get_file_handle_with_options promise rejected")
        .dyn_into::<FileSystemFileHandle>()
        .expect_throw("Could not cast into file handle");
    file
}
pub async fn save_file(data: &[u8], name: String) {
    save_to_file(data, LAST_USED_NAME).await;
    save_to_file(name.as_bytes(), LAST_USED_IMAGE_NAME).await;
}
async fn save_to_file(data: &[u8], file_name: &str) {
    log!("Saving");
    let file = get_file(file_name).await;
    let writer = JsFuture::from(file.create_writable())
        .await
        .expect_throw("create_writable threw")
        .dyn_into::<WritableStream>()
        .expect_throw("Unable to cast to writeable stream")
        .get_writer()
        .expect_throw("Unable to get writer");
    let _ = JsFuture::from(writer.ready()).await;
    let data = {
        let js_data = Uint8Array::new_with_length(data.len() as u32);
        js_data.copy_from(data);
        js_data
    };
    JsFuture::from(writer.write_with_chunk(&data))
        .await
        .expect_throw("writing failed");
    log!("saved");
    let _ = JsFuture::from(writer.close()).await;
}
pub async fn load_file() -> (Vec<u8>, String) {
    let data = load_from_file(LAST_USED_NAME).await;
    let name = load_from_file(LAST_USED_IMAGE_NAME).await;
    let name = String::from_utf8(name);
    (data, name.expect_throw("Name was not utf8"))
}
pub async fn load_from_file(file_name: &str) -> Vec<u8> {
    let file = get_file(file_name).await;
    let file = JsFuture::from(file.get_file())
        .await
        .expect_throw("Could not get file")
        .dyn_into::<File>()
        .expect_throw("Could not cast to blob");
    let data = gloo_file::futures::read_as_bytes(&file.into())
        .await
        .expect_throw("read file");
    data
}
