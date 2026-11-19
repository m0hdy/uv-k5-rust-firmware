use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=linker/firmware.ld");
    println!("cargo:rerun-if-changed=mixed-fallback/fallback.c");
    println!("cargo:rerun-if-env-changed=CLANG");

    if env::var_os("CARGO_FEATURE_MIXED_FALLBACK").is_none() {
        return;
    }

    let c_selected = env::var_os("CARGO_FEATURE_MIXED_FALLBACK_C").is_some();
    let rust_selected = env::var_os("CARGO_FEATURE_MIXED_FALLBACK_RUST").is_some();
    assert_ne!(
        c_selected, rust_selected,
        "select exactly one of mixed-fallback-c or mixed-fallback-rust"
    );
    assert_eq!(
        env::var("TARGET").as_deref(),
        Ok("thumbv6m-none-eabi"),
        "the opt-in mixed fallback image must target thumbv6m-none-eabi"
    );

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    let object = out_dir.join("mixed-fallback-c.o");
    let clang = env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    let status = Command::new(clang)
        .args([
            "--target=arm-none-eabi",
            "-mcpu=cortex-m0",
            "-mthumb",
            "-ffreestanding",
            "-fno-builtin",
            "-ffunction-sections",
            "-fdata-sections",
            "-std=c11",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-Os",
            "-c",
            "mixed-fallback/fallback.c",
            "-o",
        ])
        .arg(&object)
        .status()
        .expect("failed to start Clang for the opt-in mixed fallback image");
    assert!(status.success(), "Clang failed to compile the C fallback");
    println!(
        "cargo:rustc-link-arg-bin=mixed-fallback={}",
        object.display()
    );
}
