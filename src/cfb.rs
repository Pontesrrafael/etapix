//! Reader for OLE2 compound files ("structured storage"), the container `.mpp` files use: a
//! small file system of storages and streams inside one file, as described in Microsoft's
//! published [MS-CFB] specification.

use std::collections::BTreeMap;

const ENDOFCHAIN: u32 = 0xFFFF_FFFE;
const FREESECT: u32 = 0xFFFF_FFFF;
const NOSTREAM: u32 = 0xFFFF_FFFF;
const SIGNATURE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfbError(pub String);

impl std::fmt::Display for CfbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

type Result<T> = std::result::Result<T, CfbError>;

fn bad<T>(what: &str) -> Result<T> {
    Err(CfbError(format!("arquivo corrompido ou não é um .mpp: {what}")))
}

fn u16_at(b: &[u8], o: usize) -> Result<u16> {
    b.get(o..o + 2).map(|s| u16::from_le_bytes([s[0], s[1]])).map_or_else(|| bad("fim inesperado"), Ok)
}

fn u32_at(b: &[u8], o: usize) -> Result<u32> {
    b.get(o..o + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])).map_or_else(|| bad("fim inesperado"), Ok)
}

struct Entry {
    name: String,
    kind: u8,
    left: u32,
    right: u32,
    child: u32,
    start: u32,
    size: u64,
}

/// Every stream of a compound file, by path ("/   114/TBkndTask/FixedData").
pub fn read_streams(data: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    if data.len() < 512 || data[..8] != SIGNATURE {
        return bad("assinatura ausente");
    }
    let sector_size = 1usize << u16_at(data, 0x1E)?;
    let mini_size = 1usize << u16_at(data, 0x20)?;
    if !(sector_size == 512 || sector_size == 4096) {
        return bad("tamanho de setor");
    }
    let n_fat = u32_at(data, 0x2C)? as usize;
    let dir_start = u32_at(data, 0x30)?;
    let cutoff = u32_at(data, 0x38)? as u64;
    let minifat_start = u32_at(data, 0x3C)?;
    let n_minifat = u32_at(data, 0x40)? as usize;
    let mut difat_next = u32_at(data, 0x44)?;
    let n_difat = u32_at(data, 0x48)? as usize;

    let sector = |n: u32| -> Result<&[u8]> {
        let o = (n as usize + 1) * sector_size;
        data.get(o..o + sector_size).map_or_else(|| bad("setor fora do arquivo"), Ok)
    };
    // The DIFAT lists the FAT's sectors: 109 in the header, the rest chained.
    let mut difat: Vec<u32> = (0..109).map(|k| u32_at(data, 0x4C + 4 * k)).collect::<Result<_>>()?;
    for _ in 0..n_difat {
        if difat_next == ENDOFCHAIN || difat_next == FREESECT {
            break;
        }
        let s = sector(difat_next)?;
        let per = sector_size / 4;
        for k in 0..per - 1 {
            difat.push(u32_at(s, 4 * k)?);
        }
        difat_next = u32_at(s, 4 * (per - 1))?;
    }
    let mut fat = Vec::with_capacity(n_fat * sector_size / 4);
    for &f in difat.iter().take(n_fat) {
        let s = sector(f)?;
        for k in 0..sector_size / 4 {
            fat.push(u32_at(s, 4 * k)?);
        }
    }
    let chain = |start: u32| -> Result<Vec<u8>> {
        let mut out = Vec::new();
        let mut s = start;
        let mut guard = 0;
        while s != ENDOFCHAIN && s != FREESECT {
            out.extend_from_slice(sector(s)?);
            s = *fat.get(s as usize).ok_or_else(|| CfbError("cadeia de setores inválida".into()))?;
            guard += 1;
            if guard > fat.len() + 1 {
                return bad("cadeia de setores em ciclo");
            }
        }
        Ok(out)
    };

    let dir = chain(dir_start)?;
    let mut entries = Vec::new();
    for k in 0..dir.len() / 128 {
        let e = &dir[k * 128..(k + 1) * 128];
        let name_len = (u16_at(e, 64)? as usize).min(64);
        let units: Vec<u16> = (0..name_len.saturating_sub(2) / 2).map(|i| u16::from_le_bytes([e[2 * i], e[2 * i + 1]])).collect();
        entries.push(Entry {
            name: String::from_utf16_lossy(&units),
            kind: e[66],
            left: u32_at(e, 68)?,
            right: u32_at(e, 72)?,
            child: u32_at(e, 76)?,
            start: u32_at(e, 116)?,
            // Version 3 files (512-byte sectors) only use the low 32 bits of the size.
            size: u32_at(e, 120)? as u64 | if sector_size == 4096 { (u32_at(e, 124)? as u64) << 32 } else { 0 },
        });
    }
    let root = entries.first().ok_or_else(|| CfbError("diretório vazio".into()))?;
    let ministream = chain(root.start)?;
    let mut minifat = Vec::new();
    if n_minifat > 0 {
        let mf = chain(minifat_start)?;
        for k in 0..mf.len() / 4 {
            minifat.push(u32_at(&mf, 4 * k)?);
        }
    }
    let stream = |e: &Entry| -> Result<Vec<u8>> {
        let size = e.size as usize;
        if e.size < cutoff {
            let mut out = Vec::with_capacity(size);
            let mut s = e.start;
            let mut guard = 0;
            while s != ENDOFCHAIN && s != FREESECT && out.len() < size {
                let o = s as usize * mini_size;
                out.extend_from_slice(ministream.get(o..o + mini_size).ok_or_else(|| CfbError("minissetor fora".into()))?);
                s = *minifat.get(s as usize).ok_or_else(|| CfbError("minicadeia inválida".into()))?;
                guard += 1;
                if guard > minifat.len() + 1 {
                    return bad("minicadeia em ciclo");
                }
            }
            out.truncate(size);
            Ok(out)
        } else {
            let mut out = chain(e.start)?;
            out.truncate(size);
            Ok(out)
        }
    };

    // Walk the red-black trees of storages (an in-order walk of left, node, right per level).
    let mut out = BTreeMap::new();
    let mut todo = vec![(root.child, String::new())];
    let mut seen = 0;
    while let Some((first, path)) = todo.pop() {
        let mut stack = vec![first];
        while let Some(i) = stack.pop() {
            if i == NOSTREAM {
                continue;
            }
            seen += 1;
            if seen > entries.len() * 2 + 2 {
                return bad("diretório em ciclo");
            }
            let e = entries.get(i as usize).ok_or_else(|| CfbError("entrada de diretório inválida".into()))?;
            stack.push(e.left);
            stack.push(e.right);
            let p = format!("{path}/{}", e.name);
            match e.kind {
                1 => todo.push((e.child, p)),
                2 => {
                    out.insert(p, stream(e)?);
                }
                _ => {}
            }
        }
    }
    Ok(out)
}
