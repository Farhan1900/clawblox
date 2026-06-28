use std::env;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=LUAU_SRC_DIR");
    println!("cargo:rerun-if-changed=native/bridge.cpp");

    let luau_dir = find_luau_source_dir();
    let vm_include = luau_dir.join("luau").join("VM").join("include");
    let vm_src = luau_dir.join("luau").join("VM").join("src");
    let common_include = luau_dir.join("luau").join("Common").join("include");

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .warnings(false)
        .include(vm_include)
        .include(vm_src)
        .include(common_include)
        .define("LUA_API", "extern \"C\"")
        .define("LUAI_MAXCSTACK", "1000000")
        .define(
            "LUA_VECTOR_SIZE",
            if cfg!(feature = "luau-vector4") {
                "4"
            } else {
                "3"
            },
        )
        .file("native/bridge.cpp")
        .compile("luau_persist_bridge");
}

fn find_luau_source_dir() -> PathBuf {
    if let Ok(path) = env::var("LUAU_SRC_DIR") {
        let path = PathBuf::from(path);
        validate_luau_dir(&path);
        return path;
    }

    let cargo_home = env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")))
        .expect("CARGO_HOME or HOME must be set, or set LUAU_SRC_DIR");

    let registry_src = cargo_home.join("registry").join("src");
    let entries = registry_src
        .read_dir()
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", registry_src.display()));

    let mut candidates = Vec::new();
    for entry in entries {
        let entry = entry.expect("failed to read cargo registry source entry");
        let Ok(children) = entry.path().read_dir() else {
            continue;
        };
        for child in children {
            let child = child.expect("failed to read cargo registry crate entry");
            let path = child.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if name.starts_with("luau0-src-") && path.join("luau").join("VM").exists() {
                candidates.push(path);
            }
        }
    }

    candidates.sort();
    let path = candidates
        .pop()
        .expect("failed to locate luau0-src; set LUAU_SRC_DIR to the luau0-src crate directory");
    validate_luau_dir(&path);
    path
}

fn validate_luau_dir(path: &Path) {
    let required = [
        path.join("luau").join("VM").join("include").join("lua.h"),
        path.join("luau").join("VM").join("src").join("lobject.h"),
        path.join("luau").join("VM").join("src").join("lstate.h"),
    ];
    for file in required {
        assert!(
            file.exists(),
            "LUAU_SRC_DIR does not look like luau0-src: missing {}",
            file.display()
        );
    }
}
