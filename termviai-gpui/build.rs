fn main() {
    #[cfg(windows)] {
        use std::{path::PathBuf, fs};
        let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
        let icon = root.join("../assets/windows/terminal.ico").canonicalize().unwrap();
        let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        let rc = out.join("app.rc");
        fs::write(&rc, format!("1 ICON \"{}\"\n", icon.display().to_string().replace('\\', "/"))).unwrap();
        embed_resource::compile(rc);
        println!("cargo:rerun-if-changed=../assets/windows/terminal.ico");
    }
}
