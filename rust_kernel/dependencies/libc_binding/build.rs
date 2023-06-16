fn main() {
    println!(r#"cargo:rerun-if-changed={}"#, "../../../libc/include");
    println!(r#"cargo:rerun-if-changed={}"#, "../../../libc/include/sys");

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header("all_includes.h")
        .use_core()
        .derive_default(true)
        .layout_tests(false)
        .impl_debug(true)
        .ctypes_prefix("super")
        .ignore_functions()
        .blocklist_type("u8")
        .blocklist_type("u16")
        .blocklist_type("u32")
        .clang_arg("--target=x86_64-pc-linux-gnu")
        .clang_arg("--sysroot=/toolchain_turbofish/sysroot")
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    bindings
        .write_to_file("src/libc.rs")
        .expect("Couldn't write bindings!");
}
