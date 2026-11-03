use std::path::Path;

fn main() {
    // tauri's context macro needs frontendDist to exist. On a fresh checkout
    // that has not run `npm run ui:build` yet, put a placeholder there so
    // `cargo build --workspace` still works.
    let dist = Path::new("../ui/dist");
    if !dist.join("index.html").exists() {
        println!("cargo:warning=ui/dist is missing; run `npm run ui:build` for the real dashboard");
        std::fs::create_dir_all(dist).expect("create ui/dist");
        std::fs::write(
            dist.join("index.html"),
            "<!doctype html><title>activity_nagger</title><p>Dashboard not built. Run <code>npm run ui:build</code>.</p>",
        )
        .expect("write placeholder");
    }
    tauri_build::build();
}
