use std::env;
use std::path::{Path, PathBuf};

fn harfbuzz() {
    use std::fs;

    if !Path::new("harfbuzz/.git").exists() {
        git_submodule_update();
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let mut cfg = cc::Build::new();
    cfg.warnings(false);
    cfg.cpp(true);
    cfg.flag_if_supported("-fno-rtti");
    cfg.flag_if_supported("-fno-exceptions");
    cfg.flag_if_supported("-fno-threadsafe-statics");
    cfg.flag_if_supported("-std=c++11");
    cfg.flag_if_supported("-fno-stack-check");
    cfg.flag_if_supported("-Wno-format-overflow");

    let build_dir = out_dir.join("harfbuzz-build");
    fs::create_dir_all(&build_dir).unwrap();
    cfg.out_dir(&build_dir);

    let target = env::var("TARGET").unwrap();

    cfg.file("harfbuzz/src/harfbuzz.cc");
    cfg.define("HB_NO_MT", None);

    if !target.contains("windows") {
        cfg.define("HAVE_UNISTD_H", None);
        cfg.define("HAVE_SYS_MMAN_H", None);
    }

    cfg.define("HAVE_FREETYPE", Some("1"));
    cfg.define("HAVE_FT_GET_VAR_BLEND_COORDINATES", Some("1"));
    cfg.define("HAVE_FT_SET_VAR_BLEND_COORDINATES", Some("1"));
    cfg.define("HAVE_FT_DONE_MM_VAR", Some("1"));
    cfg.define("HAVE_FT_GET_TRANSFORM", Some("1"));

    if env::var("CARGO_FEATURE_SYS_FREETYPE").is_ok() {
        let inc = find_freetype_sys_include();
        cfg.include(&inc);
        // Do not `rustc-link-lib` here. GPUI's `freetype-sys` already owns
        // `links = "freetype"` and emits the `freetype2` link.
    } else {
        // Import the include dirs exported from deps/freetype/build.rs
        for inc in std::env::var("DEP_FREETYPE_INCLUDE").unwrap().split(';') {
            cfg.include(inc);
        }

        println!(
            "cargo:rustc-link-search={}",
            std::env::var("DEP_FREETYPE_LIB").unwrap()
        );
        println!("cargo:rustc-link-lib=freetype");
        println!("cargo:rustc-link-lib=png");
        println!("cargo:rustc-link-lib=z");
    }

    cfg.compile("harfbuzz");
}

fn find_freetype_sys_include() -> PathBuf {
    if let Ok(p) = env::var("WEZTERM_FREETYPE_SYS_INCLUDE") {
        return PathBuf::from(p);
    }
    let cargo_home = env::var("CARGO_HOME").map(PathBuf::from).unwrap_or_else(|_| {
        let home = env::var("USERPROFILE")
            .or_else(|_| env::var("HOME"))
            .expect("HOME or USERPROFILE");
        PathBuf::from(home).join(".cargo")
    });
    let src_root = cargo_home.join("registry").join("src");
    if let Ok(indexes) = std::fs::read_dir(&src_root) {
        for index in indexes.flatten() {
            let candidate = index
                .path()
                .join("freetype-sys-0.20.1")
                .join("freetype2")
                .join("include");
            if candidate.join("ft2build.h").is_file() {
                return candidate;
            }
        }
    }
    panic!(
        "harfbuzz sys-freetype: could not find freetype-sys-0.20.1 headers under {}. \
         Set WEZTERM_FREETYPE_SYS_INCLUDE to .../freetype-sys-0.20.1/freetype2/include",
        src_root.display()
    );
}

fn git_submodule_update() {
    let _ = std::process::Command::new("git")
        .args(&["submodule", "update", "--init"])
        .status();
}

fn main() {
    harfbuzz();
    let out_dir = env::var("OUT_DIR").unwrap();
    println!("cargo:outdir={}", out_dir);
    println!("cargo:rustc-env=MACOSX_DEPLOYMENT_TARGET=10.12");
}
