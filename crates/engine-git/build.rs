fn main() {
    // Defensive: on some MSVC toolchain/SDK combinations, libgit2-sys's
    // vendored build doesn't automatically pull in advapi32.lib, which
    // provides the Windows security/crypto APIs libgit2 itself calls
    // (OpenProcessToken, CryptAcquireContextA, RegOpenKeyExW, and friends)
    // — surfacing as LNK2019 unresolved-external errors at link time, not
    // a compile error, so it only shows up on an actual Windows build/CI
    // run. Cargo dedupes repeated `-lib` link directives, so declaring
    // this explicitly is a no-op when the toolchain already links it
    // correctly, and a real fix when it doesn't.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rustc-link-lib=advapi32");
    }
}
