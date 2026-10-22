// Shared by the app's and the installer's build scripts: writes the Etapix mark as a
// multi-size `.ico` and links it into the `.exe` as its icon (through `windres`, which the
// WinLibs binutils provide). Without `windres` the build goes on with the default icon.

#[allow(dead_code)]
mod logo {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../app/src/logo.rs"));
}

/// A `.ico` with 32-bit BMP images of the given sizes.
fn ico(sizes: &[u32]) -> Vec<u8> {
    let images: Vec<Vec<u8>> = sizes
        .iter()
        .map(|&s| {
            let px = logo::rgba(s);
            let mut b = Vec::new();
            // BITMAPINFOHEADER; the height counts the colour rows and the AND mask.
            for v in [40u32, s, s * 2] {
                b.extend(v.to_le_bytes());
            }
            b.extend(1u16.to_le_bytes());
            b.extend(32u16.to_le_bytes());
            b.extend([0u8; 24]);
            for y in (0..s).rev() {
                for x in 0..s {
                    let i = ((y * s + x) * 4) as usize;
                    b.extend([px[i + 2], px[i + 1], px[i], px[i + 3]]);
                }
            }
            // AND mask: all zero (transparency comes from alpha), rows padded to 4 bytes.
            b.extend(vec![0u8; (s.div_ceil(32) * 4 * s) as usize]);
            b
        })
        .collect();
    let mut out = Vec::new();
    out.extend([0u8, 0, 1, 0]);
    out.extend((sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (&s, img) in sizes.iter().zip(&images) {
        let d = if s >= 256 { 0 } else { s as u8 };
        out.extend([d, d, 0, 0]);
        out.extend(1u16.to_le_bytes());
        out.extend(32u16.to_le_bytes());
        out.extend((img.len() as u32).to_le_bytes());
        out.extend(offset.to_le_bytes());
        offset += img.len() as u32;
    }
    for img in images {
        out.extend(img);
    }
    out
}

pub fn embed_icon() {
    println!("cargo:rerun-if-changed=../app/src/logo.rs");
    println!("cargo:rerun-if-changed=../app/icon_build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let icon = out.join("etapix.ico");
    std::fs::write(&icon, ico(&[16, 24, 32, 48, 64, 256])).unwrap();
    let rc = out.join("icon.rc");
    std::fs::write(&rc, format!("1 ICON \"{}\"\n", icon.display().to_string().replace('\\', "/"))).unwrap();
    let obj = out.join("icon.o");
    match std::process::Command::new("windres").arg(&rc).arg("-O").arg("coff").arg("-o").arg(&obj).status() {
        Ok(s) if s.success() => println!("cargo:rustc-link-arg-bins={}", obj.display()),
        _ => println!("cargo:warning=windres not found: the .exe keeps the default icon"),
    }
}
