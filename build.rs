fn main() {
    #[cfg(target_os = "windows")]
    embed_resource::compile("resource/embed_icon.rc", embed_resource::NONE);
}
