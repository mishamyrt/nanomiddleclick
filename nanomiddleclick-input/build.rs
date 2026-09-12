use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=shim/input_runtime.c");
    println!("cargo:rerun-if-changed=shim/MultitouchSupport.h");
    println!("cargo:rerun-if-changed=shim/nanomiddleclick_input.h");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is not set"));
    let input_runtime_object_path = out_dir.join("input_runtime.o");
    let library_path = out_dir.join("libnanomiddleclick_input_shim.a");

    run(Command::new("xcrun")
        .arg("clang")
        .arg("-c")
        .arg("shim/input_runtime.c")
        .arg("-o")
        .arg(&input_runtime_object_path)
        .arg("-I")
        .arg("shim")
        .arg("-Wall")
        .arg("-Wextra"));

    run(Command::new("xcrun")
        .arg("libtool")
        .arg("-static")
        .arg("-o")
        .arg(&library_path)
        .arg(&input_runtime_object_path));

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-search=framework=/System/Library/PrivateFrameworks");
    println!("cargo:rustc-link-lib=static=nanomiddleclick_input_shim");
    println!("cargo:rustc-link-lib=framework=ApplicationServices");
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
    println!("cargo:rustc-link-lib=framework=IOKit");
    println!("cargo:rustc-link-lib=framework=MultitouchSupport");
}

fn run(command: &mut Command) {
    let rendered = format!("{command:?}");
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("failed to run `{rendered}`: {error}"));

    assert!(status.success(), "command `{rendered}` exited with status {status}");
}
