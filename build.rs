use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=src/platform/sandbox/windows.cpp");
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
        // EN-US: Compile the narrow Win32 isolation bridge for both application and native tests.
        // PT-BR: Compila a ponte restrita de isolamento Win32 para o aplicativo e os testes nativos.
        let object=directory.join("media.o");
        assert!(Command::new("g++").args(["-std=c++17","-O2","-Wall","-Wextra","-Werror","-c","src/platform/sandbox/windows.cpp","-o"])
            .arg(&object).status().expect("UCRT64 g++ required").success(),"Windows isolation compilation failed");
        assert!(Command::new("ar").arg("crs").arg(directory.join("libpapo_media.a")).arg(object)
            .status().expect("UCRT64 ar required").success(),"Windows isolation archive failed");
        println!("cargo:rustc-link-search=native={}",directory.display());
        for library in ["static=papo_media","dylib=stdc++","dylib=userenv","dylib=advapi32","dylib=ole32"] {
            println!("cargo:rustc-link-lib={library}");
        }
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
