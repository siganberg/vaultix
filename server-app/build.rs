fn main() {
    // Share the desktop client's design system and this app's logo with the UI.
    for (src, dst) in [("../desktop/ui/common.css", "ui/common.css"), ("../desktop/ui/common.js", "ui/common.js"), ("icon.svg", "ui/icon.svg")] {
        println!("cargo:rerun-if-changed={src}");
        std::fs::copy(src, dst).expect("copy shared UI asset");
    }
    tauri_build::build()
}
