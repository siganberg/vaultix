fn main() {
    // Share the web app's file-type icons and logo with the desktop UI.
    for f in ["fileicons.js", "icon.svg"] {
        let src = format!("../web/{f}");
        println!("cargo:rerun-if-changed={src}");
        std::fs::copy(&src, format!("ui/{f}")).expect("copy shared web asset");
    }
    tauri_build::build()
}
