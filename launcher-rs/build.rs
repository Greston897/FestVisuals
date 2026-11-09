fn main() {
    let out = std::env::var("OUT_DIR").unwrap();
    let include = "vendor/freetype-include/include";
    for (src, obj) in [("vendor/puff.c", "puff.o"), ("vendor/fontbridge.c", "fontbridge.o")] {
        let status = std::process::Command::new("gcc")
            .args(["-c", "-O2", "-fPIC", "-I", include, src, "-o"])
            .arg(format!("{out}/{obj}"))
            .status()
            .expect("gcc is required to build the launcher");
        assert!(status.success(), "{src} failed to compile");
        println!("cargo:rustc-link-arg={out}/{obj}");
        println!("cargo:rerun-if-changed={src}");
    }
    println!("cargo:rustc-link-arg=/usr/lib/x86_64-linux-gnu/libfreetype.so.6");
    println!("cargo:rerun-if-changed=vendor/freetype-include/include/ft2build.h");
}
