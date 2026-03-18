#[cfg(not(any(
    feature = "prio_bits_2",
    feature = "prio_bits_3",
    feature = "prio_bits_4",
    feature = "prio_bits_5",
    feature = "prio_bits_6",
    feature = "prio_bits_7",
    feature = "prio_bits_8"
)))]
fn main() {}

#[cfg(any(
    feature = "prio_bits_2",
    feature = "prio_bits_3",
    feature = "prio_bits_4",
    feature = "prio_bits_5",
    feature = "prio_bits_6",
    feature = "prio_bits_7",
    feature = "prio_bits_8"
))]
fn main() {
    use std::{env, path::PathBuf};

    let target = env::var("TARGET").unwrap();

    println!("cargo:rerun-if-env-changed=DEFMT_RTT_BUFFER_SIZE");
    println!("cargo:rerun-if-env-changed=DEFMT_RTT_UP_CHANNELS");
    println!("cargo:rerun-if-changed=build.rs");

    println!("cargo:rustc-check-cfg=cfg(armv6m)");
    println!("cargo:rustc-check-cfg=cfg(armv7m)");
    println!("cargo:rustc-check-cfg=cfg(armv7em)");
    println!("cargo:rustc-check-cfg=cfg(armv8m)");
    println!("cargo:rustc-check-cfg=cfg(armv8m_base)");
    println!("cargo:rustc-check-cfg=cfg(armv8m_main)");
    println!("cargo:rustc-check-cfg=cfg(cortex_m)");
    println!("cargo:rustc-check-cfg=cfg(has_fpu)");
    println!("cargo:rustc-check-cfg=cfg(native)");

    if target.starts_with("thumbv6m-") {
        println!("cargo:rustc-cfg=cortex_m");
        println!("cargo:rustc-cfg=armv6m");
    } else if target.starts_with("thumbv7m-") {
        println!("cargo:rustc-cfg=cortex_m");
        println!("cargo:rustc-cfg=armv7m");
    } else if target.starts_with("thumbv7em-") {
        println!("cargo:rustc-cfg=cortex_m");
        println!("cargo:rustc-cfg=armv7m");
        println!("cargo:rustc-cfg=armv7em"); // (not currently used)
    } else if target.starts_with("thumbv8m.base") {
        println!("cargo:rustc-cfg=cortex_m");
        println!("cargo:rustc-cfg=armv8m");
        println!("cargo:rustc-cfg=armv8m_base");
    } else if target.starts_with("thumbv8m.main") {
        println!("cargo:rustc-cfg=cortex_m");
        println!("cargo:rustc-cfg=armv8m");
        println!("cargo:rustc-cfg=armv8m_main");
    }

    #[cfg(feature = "prio_bits_2")]
    let prio_bits = 2;
    #[cfg(feature = "prio_bits_3")]
    let prio_bits = 3;
    #[cfg(feature = "prio_bits_4")]
    let prio_bits = 4;
    #[cfg(feature = "prio_bits_5")]
    let prio_bits = 5;
    #[cfg(feature = "prio_bits_6")]
    let prio_bits = 6;
    #[cfg(feature = "prio_bits_7")]
    let prio_bits = 7;
    #[cfg(feature = "prio_bits_8")]
    let prio_bits = 8;

    let size = env::var("DEFMT_RTT_BUFFER_SIZE")
        .map(|s| {
            s.parse()
                .expect("could not parse DEFMT_RTT_BUFFER_SIZE as usize")
        })
        .unwrap_or(1024_usize);

    let up_channels = env::var("DEFMT_RTT_UP_CHANNELS")
        .map(|s| {
            s.parse()
                .expect("could not parse DEFMT_RTT_UP_CHANNELS as usize")
        })
        .unwrap_or((1 << prio_bits) + 2);

    let out_dir_path = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let out_file_path = out_dir_path.join("consts.rs");

    std::fs::write(
        out_file_path,
        format!(
            "/// RTT buffer size (default: 1024).
            ///
            /// Can be customized by setting the `DEFMT_RTT_BUFFER_SIZE` environment variable.
            /// Use a power of 2 for best performance.
            pub(crate) const BUF_SIZE: usize = {size};

            /// Number of RTT UP channels (default: 2^PRIO_BITS + 2).
            ///
            /// Can be customized by setting the `DEFMT_RTT_UP_CHANNELS` environment variable.
            pub(crate) const UP_CHANNELS: usize = {up_channels};

            /// Number of priority bits
            ///
            /// Can be customized by setting `prio_bits_X` features
            pub(crate) const PRIO_BITS: usize = {prio_bits};
            ",
        ),
    )
    .unwrap();
}
