const MANIFEST: &str = r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>"#;

fn main() {
    println!("cargo:rerun-if-changed=../apps/launcher/assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../apps/launcher/assets/icon.ico");
        // Without a requested execution level, Windows takes an exe whose
        // name says "Installer" for a setup program and runs it elevated.
        res.set_manifest(MANIFEST);
        res.compile().expect("failed to embed the installer resources");
    }
}
