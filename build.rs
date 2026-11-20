use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=linker/firmware.ld");
    println!("cargo:rerun-if-changed=mixed-fallback/fallback.c");
    println!("cargo:rerun-if-env-changed=CLANG");

    if env::var_os("CARGO_FEATURE_FIRMWARE_INTEGRATION").is_some() {
        configure_firmware_integration();
    }

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

fn configure_firmware_integration() {
    const REQUIRED_POLICY: &[(&str, &str, bool)] = &[
        ("UVK5_ENABLE_SWD", "UVK5_POLICY_ENABLE_SWD", true),
        ("UVK5_ENABLE_FMRADIO", "UVK5_POLICY_ENABLE_FMRADIO", true),
        ("UVK5_ENABLE_UART", "UVK5_POLICY_ENABLE_UART", true),
        ("UVK5_ENABLED_AIRCOPY", "UVK5_POLICY_ENABLED_AIRCOPY", true),
        ("UVK5_ENABLE_NOAA", "UVK5_POLICY_ENABLE_NOAA", true),
        ("UVK5_ENABLE_VOX", "UVK5_POLICY_ENABLE_VOX", true),
        (
            "UVK5_ENABLE_BLMIN_TMP_OFF",
            "UVK5_POLICY_ENABLE_BLMIN_TMP_OFF",
            true,
        ),
        ("UVK5_ADC_EXTTRIG_SEL", "UVK5_POLICY_ADC_EXTTRIG_SEL", false),
        (
            "UVK5_ST7565_FIFO_POLL_LIMIT",
            "UVK5_POLICY_ST7565_FIFO_POLL_LIMIT",
            false,
        ),
        (
            "UVK5_BATTERY_POLL_BUDGET",
            "UVK5_POLICY_BATTERY_POLL_BUDGET",
            false,
        ),
        (
            "UVK5_BATTERY_CALIBRATION",
            "UVK5_POLICY_BATTERY_CALIBRATION",
            false,
        ),
        (
            "UVK5_SYSTICK_TICKS_PER_MICROSECOND",
            "UVK5_POLICY_SYSTICK_TICKS_PER_MICROSECOND",
            false,
        ),
    ];

    assert_eq!(
        env::var("TARGET").as_deref(),
        Ok("thumbv6m-none-eabi"),
        "firmware-integration is only selectable for thumbv6m-none-eabi"
    );
    for (input, output, is_bool) in REQUIRED_POLICY {
        println!("cargo:rerun-if-env-changed={input}");
        let value = env::var(input).unwrap_or_else(|_| {
            panic!("firmware-integration requires {input}; no policy defaults exist")
        });
        if *is_bool {
            assert!(
                matches!(value.as_str(), "true" | "false"),
                "{input} must be exactly true or false"
            );
            assert!(
                *input != "UVK5_ENABLE_SWD" || value == "false",
                "firmware-integration requires UVK5_ENABLE_SWD=false because PB11 is the display reset pin"
            );
        } else {
            let parsed = value
                .parse::<u32>()
                .unwrap_or_else(|_| panic!("{input} must be an unsigned integer"));
            if *input != "UVK5_ADC_EXTTRIG_SEL" {
                assert!(parsed > 0, "{input} must be nonzero");
            }
            if matches!(*input, "UVK5_ADC_EXTTRIG_SEL" | "UVK5_BATTERY_CALIBRATION") {
                assert!(parsed <= u32::from(u16::MAX), "{input} exceeds u16");
            }
            if *input == "UVK5_SYSTICK_TICKS_PER_MICROSECOND" {
                assert!(
                    parsed <= u32::MAX / 120_000,
                    "{input} makes the 120 ms startup delay overflow u32 ticks"
                );
            }
        }
        println!("cargo:rustc-env={output}={value}");
    }
}
