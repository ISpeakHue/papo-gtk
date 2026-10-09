use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=assets/papo.gresource.xml");
    println!("cargo:rerun-if-changed=assets/icons/hicolor");

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
