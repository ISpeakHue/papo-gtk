use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=data/windows");
    println!("cargo:rerun-if-changed=packaging/windows/resources.py");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // EN-US: Embed resources in the actual GTK process too, before any HWND is created.
        // PT-BR: Incorpora recursos também no processo GTK, antes de criar qualquer HWND.
        let directory = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("windows");
        let status = Command::new("python").args(["packaging/windows/resources.py", "--version"])
            .arg(env::var("CARGO_PKG_VERSION").unwrap()).arg("--output").arg(&directory)
            .args(["--original", "papo-gtk.exe"]).status().expect("UCRT64 Python/windres required");
        assert!(status.success(), "Windows resource compilation failed");
        println!("cargo:rustc-link-arg-bin=papo-gtk={}", directory.join("papo.o").display());
    }
    println!("cargo:rerun-if-changed=assets/papo.gresource.xml");
    println!("cargo:rerun-if-changed=assets/icons/hicolor");
    println!("cargo:rerun-if-changed=assets/icons/symbolic");
    println!("cargo:rerun-if-changed=assets/status");

    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo must set OUT_DIR"))
        .join("papo.gresource");
    let status = Command::new("glib-compile-resources")
        .arg("assets/papo.gresource.xml")
        .arg("--sourcedir=assets")
        .arg("--target")
        .arg(output)
        .status()
        .expect("glib-compile-resources is required; install GLib development tools");
    assert!(status.success(), "Failed to compile Papo's application resources");
}
