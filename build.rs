use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=DEFMT_RTT_BUFFER_SIZE");
    println!("cargo:rerun-if-env-changed=DEFMT_RTT_UP_CHANNELS");
    println!("cargo:rerun-if-changed=build.rs");

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
        .unwrap_or(17_usize);

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

            /// Number of RTT UP channels (default: 17).
            ///
            /// Can be customized by setting the `DEFMT_RTT_UP_CHANNELS` environment variable.
            /// This should typically be 2^(NVIC priority bits) + 1
            pub(crate) const UP_CHANNELS: usize = {up_channels};
            ",
        ),
    )
    .unwrap();
}
