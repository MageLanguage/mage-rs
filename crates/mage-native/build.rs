use cc::Build;
use std::{env, path::Path, process::Command};

fn main() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("failed to resolve workspace root");

    let vm_source = workspace_root.join("native/amd64/vm.s");
    let output_directory = env::var("OUT_DIR").expect("missing OUT_DIR");
    let vm_object = Path::new(&output_directory).join("vm.o");

    println!("cargo:rerun-if-changed={}", vm_source.display());

    let status = Command::new("fasm")
        .arg(&vm_source)
        .arg(&vm_object)
        .status()
        .expect("failed to run fasm");

    if !status.success() {
        panic!("fasm failed to compile vm.s");
    }

    Build::new().object(&vm_object).compile("magevm");
}
