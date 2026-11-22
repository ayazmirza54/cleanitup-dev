//! Embeds the app icon and file details into the Windows executable.

fn main() {
    println!("cargo:rerun-if-changed=assets/cleanitup.rc");
    println!("cargo:rerun-if-changed=assets/cleanitup.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("assets/cleanitup.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("embedding the app icon");
    }
}
