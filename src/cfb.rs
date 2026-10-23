//! Reader and writer for OLE2 compound files ("structured storage"), the container `.mpp` files
//! use: a small file system of storages and streams inside one file, as described in Microsoft's
//! published [MS-CFB] specification.
//!
//! Writing keeps every directory entry as it was (names, class ids, timestamps, the tree) and
//! only lays the streams out again, so a file read and written back differs only where a
//! stream's contents changed.

use std::collections::BTreeMap;

const ENDOFCHAIN: u32 = 0xFFFF_FFFE;
const FREESECT: u32 = 0xFFFF_FFFF;
const FATSECT: u32 = 0xFFFF_FFFD;
const NOSTREAM: u32 = 0xFFFF_FFFF;
const SIGNATURE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
const ENTRY: usize = 128;
const STORAGE: u8 = 1;
const STREAM: u8 = 2;
/// What the writer produces: version 3, 512-byte sectors, 64-byte mini sectors.
const SECTOR: usize = 512;
const MINI: usize = 64;
const CUTOFF: usize = 4096;

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

fn put_u32(b: &mut [u8], o: usize, v: u32) {
    b[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

/// A compound file in memory: its directory entries, raw, and each stream's contents.
pub struct Compound {
    entries: Vec<[u8; ENTRY]>,
    /// Contents by entry index (streams only; the root's ministream is rebuilt on write).
    contents: Vec<Option<Vec<u8>>>,
    /// Entry index by path ("/   114/TBkndTask/FixedData").
    paths: BTreeMap<String, usize>,
}

impl Compound {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let file = Layout::read(data)?;
        let entries: Vec<[u8; ENTRY]> = file.dir.as_chunks::<ENTRY>().0.to_vec();
        if entries.is_empty() {
            return bad("diretório vazio");
        }
        let ministream = file.chain(entry_start(&entries[0]))?;
        let mut contents = vec![None; entries.len()];
        let mut paths = BTreeMap::new();
        // Walk the red-black trees of storages (left, node, right per level).
        let mut todo = vec![(entry_child(&entries[0]), String::new())];
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
                stack.push(u32_at(e, 68)?);
                stack.push(u32_at(e, 72)?);
                let p = format!("{path}/{}", entry_name(e));
                match e[66] {
                    STORAGE => todo.push((entry_child(e), p)),
                    STREAM => {
                        contents[i as usize] = Some(file.stream(e, &ministream)?);
                        paths.insert(p, i as usize);
                    }
                    _ => {}
                }
            }
        }
        Ok(Compound { entries, contents, paths })
    }

    pub fn get(&self, path: &str) -> Option<&[u8]> {
        self.paths.get(path).and_then(|&i| self.contents[i].as_deref())
    }

    /// Replaces an existing stream's contents.
    pub fn set(&mut self, path: &str, data: Vec<u8>) -> Result<()> {
        let &i = self.paths.get(path).ok_or_else(|| CfbError(format!("fluxo inexistente: {path}")))?;
        self.contents[i] = Some(data);
        Ok(())
    }

    pub fn streams(&self) -> BTreeMap<String, Vec<u8>> {
        self.paths.iter().map(|(p, &i)| (p.clone(), self.contents[i].clone().unwrap_or_default())).collect()
    }

    /// The whole file, as version 3 (512-byte sectors).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut entries = self.entries.clone();
        // Small streams go to the ministream, the rest to sectors of their own.
        let mut ministream = Vec::new();
        let mut minifat = Vec::new();
        let mut big = Vec::new();
        for (i, data) in self.contents.iter().enumerate() {
            let Some(data) = data else { continue };
            set_size(&mut entries[i], data.len());
            if data.is_empty() {
                put_u32(&mut entries[i], 116, ENDOFCHAIN);
            } else if data.len() < CUTOFF {
                let first = (ministream.len() / MINI) as u32;
                put_u32(&mut entries[i], 116, first);
                chain_into(&mut minifat, first, data.len().div_ceil(MINI));
                ministream.extend_from_slice(data);
                ministream.resize(ministream.len().next_multiple_of(MINI), 0);
            } else {
                big.push(i);
            }
        }
        let mut dir_len = entries.len().next_multiple_of(SECTOR / ENTRY);
        let sectors = |n: usize| n.div_ceil(SECTOR);
        let minifat_bytes = minifat.len() * 4;
        let content = sectors(dir_len * ENTRY)
            + sectors(minifat_bytes)
            + sectors(ministream.len())
            + big.iter().map(|&i| sectors(self.contents[i].as_ref().unwrap().len())).sum::<usize>();
        // FAT sectors list themselves too; past 109 of them the DIFAT needs sectors of its own.
        let per = SECTOR / 4;
        let (mut n_fat, mut n_difat) = (0usize, 0usize);
        loop {
            let need = (content + n_fat + n_difat).div_ceil(per);
            let difat = need.saturating_sub(109).div_ceil(per - 1);
            if (need, difat) == (n_fat, n_difat) {
                break;
            }
            (n_fat, n_difat) = (need, difat);
        }
        let mut fat = vec![FREESECT; n_fat * per];
        let mut next = 0u32;
        let mut take = |fat: &mut Vec<u32>, n: usize, mark: Option<u32>| -> u32 {
            let first = next;
            for k in 0..n as u32 {
                fat[(first + k) as usize] = mark.unwrap_or(if k + 1 == n as u32 { ENDOFCHAIN } else { first + k + 1 });
            }
            next += n as u32;
            if n == 0 {
                ENDOFCHAIN
            } else {
                first
            }
        };
        let fat_start = take(&mut fat, n_fat, Some(FATSECT));
        let difat_start = take(&mut fat, n_difat, Some(0xFFFF_FFFC));
        let dir_start = take(&mut fat, sectors(dir_len * ENTRY), None);
        let minifat_start = take(&mut fat, sectors(minifat_bytes), None);
        let mini_start = take(&mut fat, sectors(ministream.len()), None);
        put_u32(&mut entries[0], 116, if ministream.is_empty() { ENDOFCHAIN } else { mini_start });
        set_size(&mut entries[0], ministream.len());
        let mut big_starts = Vec::new();
        for &i in &big {
            let start = take(&mut fat, sectors(self.contents[i].as_ref().unwrap().len()), None);
            put_u32(&mut entries[i], 116, start);
            big_starts.push(start);
        }

        let mut out = vec![0u8; SECTOR * (1 + next as usize)];
        let header = &mut out[..SECTOR];
        header[..8].copy_from_slice(&SIGNATURE);
        header[0x18..0x1A].copy_from_slice(&0x3Eu16.to_le_bytes());
        header[0x1A..0x1C].copy_from_slice(&3u16.to_le_bytes());
        header[0x1C..0x1E].copy_from_slice(&0xFFFEu16.to_le_bytes());
        header[0x1E..0x20].copy_from_slice(&9u16.to_le_bytes());
        header[0x20..0x22].copy_from_slice(&6u16.to_le_bytes());
        put_u32(header, 0x2C, n_fat as u32);
        put_u32(header, 0x30, dir_start);
        put_u32(header, 0x38, CUTOFF as u32);
        put_u32(header, 0x3C, if minifat.is_empty() { ENDOFCHAIN } else { minifat_start });
        put_u32(header, 0x40, sectors(minifat_bytes) as u32);
        put_u32(header, 0x44, if n_difat == 0 { ENDOFCHAIN } else { difat_start });
        put_u32(header, 0x48, n_difat as u32);
        let fat_sectors: Vec<u32> = (0..n_fat as u32).map(|k| fat_start + k).collect();
        for k in 0..109 {
            put_u32(header, 0x4C + 4 * k, fat_sectors.get(k).copied().unwrap_or(FREESECT));
        }
        let at = |s: u32| SECTOR * (1 + s as usize);
        // DIFAT sectors: the FAT sectors past the header's 109, chained by their last slot.
        for d in 0..n_difat {
            let o = at(difat_start + d as u32);
            for k in 0..per - 1 {
                put_u32(&mut out, o + 4 * k, fat_sectors.get(109 + d * (per - 1) + k).copied().unwrap_or(FREESECT));
            }
            put_u32(&mut out, o + 4 * (per - 1), if d + 1 == n_difat { ENDOFCHAIN } else { difat_start + d as u32 + 1 });
        }
        for (k, v) in fat.iter().enumerate() {
            put_u32(&mut out, at(fat_start) + 4 * k, *v);
        }
        while entries.len() < dir_len {
            let mut empty = [0u8; ENTRY];
            for o in [68, 72, 76] {
                put_u32(&mut empty, o, NOSTREAM);
            }
            entries.push(empty);
        }
        dir_len = entries.len();
        for (k, e) in entries.iter().take(dir_len).enumerate() {
            out[at(dir_start) + ENTRY * k..][..ENTRY].copy_from_slice(e);
        }
        for (k, v) in minifat.iter().enumerate() {
            put_u32(&mut out, at(minifat_start) + 4 * k, *v);
        }
        let minifat_end = at(minifat_start) + sectors(minifat_bytes) * SECTOR;
        for b in &mut out[at(minifat_start) + minifat_bytes..minifat_end] {
            *b = 0xFF;
        }
        out[at(mini_start)..][..ministream.len()].copy_from_slice(&ministream);
        for (&i, &start) in big.iter().zip(&big_starts) {
            let data = self.contents[i].as_ref().unwrap();
            out[at(start)..][..data.len()].copy_from_slice(data);
        }
        out
    }
}

/// Every stream of a compound file, by path ("/   114/TBkndTask/FixedData").
pub fn read_streams(data: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    Compound::parse(data).map(|c| c.streams())
}

fn entry_name(e: &[u8]) -> String {
    let len = (u16::from_le_bytes([e[64], e[65]]) as usize).min(64);
    let units: Vec<u16> = (0..len.saturating_sub(2) / 2).map(|i| u16::from_le_bytes([e[2 * i], e[2 * i + 1]])).collect();
    String::from_utf16_lossy(&units)
}

fn entry_child(e: &[u8]) -> u32 {
    u32::from_le_bytes(e[76..80].try_into().unwrap())
}

fn entry_start(e: &[u8]) -> u32 {
    u32::from_le_bytes(e[116..120].try_into().unwrap())
}

fn set_size(e: &mut [u8; ENTRY], len: usize) {
    put_u32(e, 120, len as u32);
    put_u32(e, 124, 0);
}

/// Appends a chain of `n` mini sectors starting at `first` to the mini FAT.
fn chain_into(fat: &mut Vec<u32>, first: u32, n: usize) {
    for k in 0..n as u32 {
        fat.push(if k + 1 == n as u32 { ENDOFCHAIN } else { first + k + 1 });
    }
}

/// The sector structure of a file being read.
struct Layout<'a> {
    data: &'a [u8],
    sector_size: usize,
    mini_size: usize,
    cutoff: u64,
    fat: Vec<u32>,
    minifat: Vec<u32>,
    dir: Vec<u8>,
}

impl<'a> Layout<'a> {
    fn read(data: &'a [u8]) -> Result<Self> {
        if data.len() < 512 || data[..8] != SIGNATURE {
            return bad("assinatura ausente");
        }
        let sector_size = 1usize << u16_at(data, 0x1E)?;
        if !(sector_size == 512 || sector_size == 4096) {
            return bad("tamanho de setor");
        }
        let mut file = Layout {
            data,
            sector_size,
            mini_size: 1usize << u16_at(data, 0x20)?,
            cutoff: u32_at(data, 0x38)? as u64,
            fat: Vec::new(),
            minifat: Vec::new(),
            dir: Vec::new(),
        };
        let n_fat = u32_at(data, 0x2C)? as usize;
        // The DIFAT lists the FAT's sectors: 109 in the header, the rest chained.
        let mut difat: Vec<u32> = (0..109).map(|k| u32_at(data, 0x4C + 4 * k)).collect::<Result<_>>()?;
        let mut difat_next = u32_at(data, 0x44)?;
        for _ in 0..u32_at(data, 0x48)? {
            if difat_next == ENDOFCHAIN || difat_next == FREESECT {
                break;
            }
            let s = file.sector(difat_next)?;
            let per = sector_size / 4;
            for k in 0..per - 1 {
                difat.push(u32_at(s, 4 * k)?);
            }
            difat_next = u32_at(s, 4 * (per - 1))?;
        }
        let mut fat = Vec::with_capacity(n_fat * sector_size / 4);
        for &f in difat.iter().take(n_fat) {
            let s = file.sector(f)?;
            for k in 0..sector_size / 4 {
                fat.push(u32_at(s, 4 * k)?);
            }
        }
        file.fat = fat;
        file.dir = file.chain(u32_at(data, 0x30)?)?;
        if u32_at(data, 0x40)? > 0 {
            let mf = file.chain(u32_at(data, 0x3C)?)?;
            file.minifat = (0..mf.len() / 4).map(|k| u32_at(&mf, 4 * k)).collect::<Result<_>>()?;
        }
        // Version 3 files (512-byte sectors) only use the low 32 bits of a stream's size.
        if sector_size == 512 {
            for e in file.dir.as_chunks_mut::<ENTRY>().0 {
                e[124..128].fill(0);
            }
        }
        Ok(file)
    }

    fn sector(&self, n: u32) -> Result<&'a [u8]> {
        let o = (n as usize + 1) * self.sector_size;
        self.data.get(o..o + self.sector_size).map_or_else(|| bad("setor fora do arquivo"), Ok)
    }

    fn chain(&self, start: u32) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        let mut s = start;
        let mut guard = 0;
        while s != ENDOFCHAIN && s != FREESECT {
            out.extend_from_slice(self.sector(s)?);
            s = *self.fat.get(s as usize).ok_or_else(|| CfbError("cadeia de setores inválida".into()))?;
            guard += 1;
            if guard > self.fat.len() + 1 {
                return bad("cadeia de setores em ciclo");
            }
        }
        Ok(out)
    }

    fn stream(&self, e: &[u8], ministream: &[u8]) -> Result<Vec<u8>> {
        let size = u32_at(e, 120)? as u64 | (u32_at(e, 124)? as u64) << 32;
        let mut out = if size < self.cutoff {
            let mut out = Vec::with_capacity(size as usize);
            let mut s = entry_start(e);
            let mut guard = 0;
            while s != ENDOFCHAIN && s != FREESECT && (out.len() as u64) < size {
                let o = s as usize * self.mini_size;
                out.extend_from_slice(ministream.get(o..o + self.mini_size).ok_or_else(|| CfbError("minissetor fora".into()))?);
                s = *self.minifat.get(s as usize).ok_or_else(|| CfbError("minicadeia inválida".into()))?;
                guard += 1;
                if guard > self.minifat.len() + 1 {
                    return bad("minicadeia em ciclo");
                }
            }
            out
        } else {
            self.chain(entry_start(e))?
        };
        out.truncate(size as usize);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writing a file back keeps every stream, and reading the copy gives the same streams.
    #[test]
    fn round_trip_keeps_every_stream() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/mpp-corpus/b1/S1-caminho-critico.mpp");
        let original = Compound::parse(&std::fs::read(path).unwrap()).unwrap();
        let copy = Compound::parse(&original.to_bytes()).unwrap();
        assert_eq!(original.streams(), copy.streams());
    }

    #[test]
    fn changed_streams_survive_the_round_trip() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/mpp-corpus/b1/S1-caminho-critico.mpp");
        let mut file = Compound::parse(&std::fs::read(path).unwrap()).unwrap();
        let name = file.paths.keys().find(|p| p.ends_with("TBkndTask/FixedData")).unwrap().clone();
        let grown = vec![7u8; 9000];
        file.set(&name, grown.clone()).unwrap();
        let copy = Compound::parse(&file.to_bytes()).unwrap();
        assert_eq!(copy.get(&name), Some(&grown[..]));
    }
}
