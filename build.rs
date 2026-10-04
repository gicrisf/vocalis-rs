use std::{env, fs, path::Path, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=data/ui");

    let out_dir = env::var("OUT_DIR").unwrap();
    let ui_out = Path::new(&out_dir).join("ui");

    let blueprints: Vec<_> = fs::read_dir("data/ui")
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "blp"))
        .collect();

    if !blueprints.is_empty() {
        fs::create_dir_all(&ui_out).unwrap();
        let ok = Command::new("blueprint-compiler")
            .arg("batch-compile")
            .arg(&ui_out)
            .arg("data/ui")
            .args(&blueprints)
            .status()
            .expect("blueprint-compiler not found")
            .success();
        assert!(ok, "blueprint-compiler failed");
    }

    glib_build_tools::compile_resources(
        &[out_dir.as_str(), "data"],
        "data/app.drey.Vocalis.data.gresource.xml",
        "app.drey.Vocalis.data.gresource",
    );
}
