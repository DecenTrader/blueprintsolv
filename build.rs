/// Build script: link Apple Accelerate framework on aarch64-apple-darwin targets (FR-027).
///
/// On Apple Silicon Macs, `-framework Accelerate` provides vDSP SIMD-accelerated
/// math operations (patch-distance sums, Sobel convolution) used in NLM denoising.
/// No effect on other platforms — all Accelerate-specific code is cfg-gated.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("aarch64")
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos")
    {
        println!("cargo:rustc-link-lib=framework=Accelerate");
    }
}
