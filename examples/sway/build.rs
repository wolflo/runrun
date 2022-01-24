use std::{env, fs, path::Path, process::Command};

const ABI_FILE: &'static str = "main.abi.json";

fn main() {
    let out_dir_base = env::current_dir().unwrap();
    let out_dir = Path::new(&out_dir_base).join("build");
    if !out_dir.exists() {
        fs::create_dir(&out_dir).expect("Failed to create build dir.");
    }
    let file = Path::new(&out_dir).join(ABI_FILE);

    // Use forc to generate contract abi and write to file
    let output = Command::new("forc").arg("json-abi").output().expect("Failed to get contract abi.");
    if !output.status.success() {
        panic!("Abi generation failed.")
    }
    fs::write(file, output.stdout).expect("Failed to write generated abi.");

    // Tell Cargo to rerun build script anytime src/** is updated
    println!("cargo:rerun-if-changed=src/");
}
