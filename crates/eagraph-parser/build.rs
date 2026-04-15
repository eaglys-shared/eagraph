use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let test_grammars_dir = out_dir.join("test_grammars");
    std::fs::create_dir_all(&test_grammars_dir).unwrap();

    // Paths relative to workspace root
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();
    let grammars_src = workspace_root
        .join("tests")
        .join("fixtures")
        .join("grammars-src");
    let grammars_config = workspace_root.join("grammars");

    // Build each grammar whose C sources exist in tests/fixtures/grammars-src/
    let python_src = grammars_src.join("python");
    if python_src.join("parser.c").exists() {
        build_shared_lib(&python_src, &test_grammars_dir, "python");

        // Copy .scm and .toml from grammars/ into the test output
        copy_if_exists(
            &grammars_config.join("python.scm"),
            &test_grammars_dir.join("python.scm"),
        );
        copy_if_exists(
            &grammars_config.join("python.toml"),
            &test_grammars_dir.join("python.toml"),
        );

        println!("cargo:rerun-if-changed={}", python_src.display());
        println!(
            "cargo:rerun-if-changed={}",
            grammars_config.join("python.scm").display()
        );
        println!(
            "cargo:rerun-if-changed={}",
            grammars_config.join("python.toml").display()
        );
    } else {
        println!(
            "cargo:warning=Grammar sources not found at {}. Tests will skip dynamic loading.",
            python_src.display()
        );
    }
}

fn build_shared_lib(src_dir: &Path, out_dir: &Path, name: &str) {
    let parser_c = src_dir.join("parser.c");
    let scanner_c = src_dir.join("scanner.c");

    let lib_name = if cfg!(target_os = "macos") {
        format!("{}.dylib", name)
    } else if cfg!(target_os = "windows") {
        format!("{}.dll", name)
    } else {
        format!("{}.so", name)
    };

    let output_path = out_dir.join(&lib_name);
    let obj_dir = out_dir.join(format!("{}_obj", name));
    std::fs::create_dir_all(&obj_dir).unwrap();

    let mut objects = vec![];
    for src in [&parser_c, &scanner_c] {
        if !src.exists() {
            continue;
        }
        let obj = obj_dir.join(src.file_stem().unwrap()).with_extension("o");
        let status = Command::new("cc")
            .args(["-c", "-fPIC", "-O2"])
            .arg("-I")
            .arg(src_dir)
            .arg(src)
            .arg("-o")
            .arg(&obj)
            .status()
            .expect("failed to run cc");
        assert!(status.success(), "failed to compile {}", src.display());
        objects.push(obj);
    }

    let mut link_cmd = Command::new("cc");
    link_cmd.arg("-shared");
    if cfg!(target_os = "macos") {
        link_cmd.arg("-dynamiclib");
    }
    for obj in &objects {
        link_cmd.arg(obj);
    }
    link_cmd.arg("-o").arg(&output_path);
    let status = link_cmd.status().expect("failed to link");
    assert!(status.success(), "failed to link {}", lib_name);
}

fn copy_if_exists(src: &Path, dst: &Path) {
    if src.exists() {
        std::fs::copy(src, dst).unwrap();
    }
}
