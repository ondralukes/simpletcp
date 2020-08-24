extern crate bindgen;
extern crate cc;

use std::env;
use std::path::PathBuf;

#[cfg(unix)]
fn main() {
    println!("cargo:rerun-if-changed=src/utils/platform/unix.c");
    println!("cargo:rerun-if-changed=src/utils/platform/interface.h");
    let bindings = bindgen::Builder::default()
        .header("src/utils/platform/interface.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate().unwrap();
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings.write_to_file(out_path.join("bindings.rs")).unwrap();
    cc::Build::new().file("src/utils/platform/unix.c").compile("unix.a");
}

#[cfg(windows)]
fn main(){
    println!("cargo:rerun-if-changed=src/utils/platform/windows.c");
    println!("cargo:rerun-if-changed=src/utils/platform/interface.h");
    let bindings = bindgen::Builder::default()
        .header("src/utils/platform/interface.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate().unwrap();
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings.write_to_file(out_path.join("bindings.rs")).unwrap();
    cc::Build::new().file("src/utils/platform/windows.c").compile("windows.a");
}
