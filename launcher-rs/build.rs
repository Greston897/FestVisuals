fn main() {
    let out = std::env::var("OUT_DIR").unwrap();
    let includes = freetype_includes();
    for (src, stem) in [("vendor/puff.c", "puff"), ("vendor/fontbridge.c", "fontbridge")] {
        let obj = if cfg!(windows) {
            format!("{out}\\{stem}.obj")
        } else {
            format!("{out}/{stem}.o")
        };
        let status = compile(src, &obj, &includes);
        let status = status.unwrap_or_else(|error| panic!("C compiler failed for {src}: {error}"));
        assert!(status.success(), "{src} failed to compile");
        println!("cargo:rustc-link-arg={obj}");
        println!("cargo:rerun-if-changed={src}");
    }
    if cfg!(windows) {
        let dir = std::env::var("FREETYPE_LIB_DIR")
            .expect("FREETYPE_LIB_DIR must point at the directory containing freetype.lib");
        let dir = dir.trim().trim_end_matches(['\\', '/']);
        println!("cargo:rustc-link-search=native={dir}");
        println!("cargo:rustc-link-lib=static=freetype");
        println!("cargo:rerun-if-env-changed=FREETYPE_LIB_DIR");
        println!("cargo:rerun-if-env-changed=FREETYPE_INCLUDE");
    } else {
        println!("cargo:rustc-link-arg=/usr/lib/x86_64-linux-gnu/libfreetype.so.6");
    }
    println!("cargo:rerun-if-changed=vendor/freetype-include/include/ft2build.h");
}

fn freetype_includes() -> Vec<String> {
    match std::env::var("FREETYPE_INCLUDE") {
        Ok(value) if !value.is_empty() => value.split(';').filter(|part| !part.is_empty()).map(|part| part.to_string()).collect(),
        _ => vec!["vendor/freetype-include/include".into()],
    }
}

fn compile(src: &str, obj: &str, includes: &[String]) -> std::io::Result<std::process::ExitStatus> {
    if cfg!(windows) {
        let mut cmd = std::process::Command::new("cl");
        cmd.arg("/nologo").arg("/c").arg("/O2");
        for inc in includes {
            cmd.arg(format!("/I{inc}"));
        }
        cmd.arg(src).arg(format!("/Fo:{obj}"));
        cmd.status()
    } else {
        let mut cmd = std::process::Command::new("gcc");
        cmd.args(["-c", "-O2", "-fPIC"]);
        for inc in includes {
            cmd.arg("-I").arg(inc);
        }
        cmd.arg(src).arg("-o").arg(obj);
        cmd.status()
    }
}
