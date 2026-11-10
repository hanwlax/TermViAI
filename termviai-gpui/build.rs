fn main() {
    #[cfg(windows)]
    {
        use std::{fs, path::PathBuf};
        let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
        let icon = root
            .join("../assets/windows/terminal.ico")
            .canonicalize()
            .unwrap();
        let version = std::env::var("CARGO_PKG_VERSION").unwrap();
        assert_eq!(
            fs::read_to_string(root.join("../VERSION")).unwrap().trim(),
            version
        );
        let components: Vec<_> = version.split(['.', '-']).collect();
        let major: u16 = components[0].parse().unwrap();
        let minor: u16 = components[1].parse().unwrap();
        let patch: u16 = components[2].parse().unwrap();
        let beta: u16 = if components.get(3) == Some(&"beta") {
            components[4].parse().unwrap()
        } else {
            0
        };
        let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        let rc = out.join("app.rc");
        let icon = icon.display().to_string().replace('\\', "/");
        fs::write(
            &rc,
            format!(
                r#"
#include <winres.h>
1 ICON "{icon}"
VS_VERSION_INFO VERSIONINFO
FILEVERSION {major},{minor},{patch},{beta}
PRODUCTVERSION {major},{minor},{patch},{beta}
FILEFLAGSMASK VS_FFI_FILEFLAGSMASK
FILEFLAGS VS_FF_PRERELEASE
FILEOS VOS__WINDOWS32
FILETYPE VFT_APP
FILESUBTYPE VFT2_UNKNOWN
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904E4"
    BEGIN
      VALUE "CompanyName", "TermViAI Project\0"
      VALUE "FileDescription", "TermViAI - Terminal Via AI\0"
      VALUE "FileVersion", "{version}\0"
      VALUE "ProductVersion", "{version}\0"
      VALUE "InternalName", "TermViAI\0"
      VALUE "OriginalFilename", "TermViAI.exe\0"
      VALUE "ProductName", "TermViAI\0"
      VALUE "LegalCopyright", "TermViAI contributors; based on WezTerm; MIT licensed\0"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1252
  END
END
"#
            ),
        )
        .unwrap();
        embed_resource::compile(rc);
        println!("cargo:rerun-if-changed=../assets/windows/terminal.ico");
        println!("cargo:rerun-if-changed=../VERSION");
    }
}
