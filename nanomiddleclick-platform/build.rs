use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=shim/config_store.c");
    println!("cargo:rerun-if-changed=shim/input_runtime.c");
    println!("cargo:rerun-if-changed=shim/MultitouchSupport.h");
    println!("cargo:rerun-if-changed=shim/nanomiddleclick_shim.h");
    println!("cargo:rerun-if-changed=shim/workspace_monitor.m");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is not set"));
    let config_store_object_path = out_dir.join("config_store.o");
    let input_runtime_object_path = out_dir.join("input_runtime.o");
    let library_path = out_dir.join("libnanomiddleclick_shim.a");
    let workspace_monitor_object_path = out_dir.join("workspace_monitor.o");

    run(Command::new("xcrun")
        .arg("clang")
        .arg("-c")
        .arg("shim/config_store.c")
        .arg("-o")
        .arg(&config_store_object_path)
        .arg("-I")
        .arg("shim")
        .arg("-Wall")
        .arg("-Wextra"));

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
        .arg("clang")
        .arg("-fobjc-arc")
        .arg("-c")
        .arg("shim/workspace_monitor.m")
        .arg("-o")
        .arg(&workspace_monitor_object_path)
        .arg("-I")
        .arg("shim")
        .arg("-Wall")
        .arg("-Wextra"));

    run(Command::new("xcrun")
        .arg("libtool")
        .arg("-static")
        .arg("-o")
        .arg(&library_path)
        .arg(&config_store_object_path)
        .arg(&input_runtime_object_path)
        .arg(&workspace_monitor_object_path));

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-search=framework=/System/Library/PrivateFrameworks");
    println!("cargo:rustc-link-lib=static=nanomiddleclick_shim");
    println!("cargo:rustc-link-lib=framework=AppKit");
    println!("cargo:rustc-link-lib=framework=ApplicationServices");
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=IOKit");
    println!("cargo:rustc-link-lib=framework=MultitouchSupport");
}

fn run(command: &mut Command) {
    let rendered = render_command(command);
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("failed to run `{rendered}`: {error}"));

    assert!(status.success(), "command `{rendered}` exited with status {status}");
}

fn render_command(command: &Command) -> String {
    let mut rendered = command.get_program().to_string_lossy().into_owned();

    for argument in command.get_args() {
        rendered.push(' ');
        rendered.push_str(&shell_escape(argument.as_ref()));
    }

    rendered
}

fn shell_escape(argument: &Path) -> String {
    let rendered = argument.to_string_lossy();
    if rendered.chars().all(|character| {
        character.is_ascii_alphanumeric() || "/._-".contains(character)
    }) {
        rendered.into_owned()
    } else {
        format!("{rendered:?}")
    }
}
