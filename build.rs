fn main() {
    #[cfg(target_os = "windows")]
    let _ = embed_resource::compile("resource/embed_icon.rc", embed_resource::NONE);
}
