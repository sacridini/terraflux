//! A zip file, read and written: what a model package (`.tfpack`, see [`crate::pack`]) is made of. Written
//! entries are deflated (their names in UTF-8, their sizes and CRC-32 written back once known, so that the file
//! has no data descriptors) and sizes or offsets past 4 GB use ZIP64; read entries may be stored or deflated,
//! with ZIP64 sizes (what the usual tools write). No encryption, no multiple disks.

use std::io::{self, Read, Seek, SeekFrom, Write};

use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::{Compression, Crc};

const LOCAL: u32 = 0x0403_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const END: u32 = 0x0605_4b50;
const END64: u32 = 0x0606_4b50;
const LOCATOR64: u32 = 0x0706_4b50;
const UTF8: u16 = 0x0800;
const BIG: u64 = 0xFFFF_FFFF;

fn bad(text: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, text.into())
}

/// The date and time of a moment (seconds since 1970, UTC) as MS-DOS writes them: (date, time).
pub fn dos(secs: u64) -> (u16, u16) {
    let (y, m, d, h, mi, s) = civil(secs);
    let year = y.clamp(1980, 2107) - 1980;
    (((year as u16) << 9) | ((m as u16) << 5) | d as u16, ((h as u16) << 11) | ((mi as u16) << 5) | (s as u16 / 2))
}

/// A moment (seconds since 1970) as year, month, day, hour, minute, second (UTC).
pub fn civil(secs: u64) -> (i64, u32, u32, u32, u32, u32) {
    let days = (secs / 86_400) as i64;
    let rest = (secs % 86_400) as u32;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let y = yoe + era * 400 + (m <= 2) as i64;
    (y, m, d, rest / 3600, rest % 3600 / 60, rest % 60)
}

struct Counted<W: Write> {
    w: W,
    n: u64,
}

impl<W: Write> Write for Counted<W> {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        let n = self.w.write(b)?;
        self.n += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.w.flush()
    }
}

struct Done {
    name: String,
    crc: u32,
    csize: u64,
    size: u64,
    offset: u64,
    big: bool,
}

/// What an entry came to once written.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Added {
    pub crc: u32,
    pub csize: u64,
    pub size: u64,
}

/// A zip file being written: [`Writer::add`] each entry, then [`Writer::finish`].
pub struct Writer<W: Write + Seek> {
    w: W,
    done: Vec<Done>,
    when: (u16, u16),
    /// Sizes and offsets from this one on are written as ZIP64 (4 GB; lower only to test it).
    pub zip64_at: u64,
}

impl<W: Write + Seek> Writer<W> {
    /// A zip file whose entries are dated `secs` (seconds since 1970).
    pub fn new(w: W, secs: u64) -> Writer<W> {
        Writer { w, done: Vec::new(), when: dos(secs), zip64_at: BIG }
    }

    /// Add an entry named `name` (with `/`), read from `data` (`size`: its length, known beforehand: whether the
    /// entry needs ZIP64 is decided by it).
    pub fn add(&mut self, name: &str, size: u64, data: &mut dyn Read) -> io::Result<Added> {
        let big = size >= self.zip64_at;
        let offset = self.w.stream_position()?;
        let name_b = name.as_bytes();
        let mut head = Vec::with_capacity(30 + name_b.len() + 20);
        head.extend(LOCAL.to_le_bytes());
        head.extend((if big { 45u16 } else { 20 }).to_le_bytes());
        head.extend(UTF8.to_le_bytes());
        head.extend(8u16.to_le_bytes());
        head.extend(self.when.1.to_le_bytes());
        head.extend(self.when.0.to_le_bytes());
        head.extend(0u32.to_le_bytes()); // the CRC, written back
        let sizes = if big { BIG as u32 } else { 0 };
        head.extend(sizes.to_le_bytes());
        head.extend(sizes.to_le_bytes());
        head.extend((name_b.len() as u16).to_le_bytes());
        head.extend((if big { 20u16 } else { 0 }).to_le_bytes());
        head.extend(name_b);
        if big {
            head.extend(1u16.to_le_bytes());
            head.extend(16u16.to_le_bytes());
            head.extend([0u8; 16]);
        }
        self.w.write_all(&head)?;
        let mut crc = Crc::new();
        let mut size = 0u64;
        let mut enc = DeflateEncoder::new(Counted { w: &mut self.w, n: 0 }, Compression::fast());
        let mut buf = vec![0u8; 1 << 20];
        loop {
            let n = data.read(&mut buf)?;
            if n == 0 {
                break;
            }
            crc.update(&buf[..n]);
            size += n as u64;
            enc.write_all(&buf[..n])?;
        }
        let counted = enc.finish()?;
        let csize = counted.n;
        if !big && (size >= BIG || csize >= BIG) {
            return Err(bad(format!("{name}: grew past 4 GB while it was read")));
        }
        let end = self.w.stream_position()?;
        self.w.seek(SeekFrom::Start(offset + 14))?;
        self.w.write_all(&crc.sum().to_le_bytes())?;
        if big {
            self.w.seek(SeekFrom::Start(offset + 30 + name_b.len() as u64 + 4))?;
            self.w.write_all(&size.to_le_bytes())?;
            self.w.write_all(&csize.to_le_bytes())?;
        } else {
            self.w.write_all(&(csize as u32).to_le_bytes())?;
            self.w.write_all(&(size as u32).to_le_bytes())?;
        }
        self.w.seek(SeekFrom::Start(end))?;
        let added = Added { crc: crc.sum(), csize, size };
        self.done.push(Done { name: name.to_string(), crc: added.crc, csize, size, offset, big });
        Ok(added)
    }

    /// Write the directory of the entries; the file's end.
    pub fn finish(mut self) -> io::Result<W> {
        let start = self.w.stream_position()?;
        let mut cd = Vec::new();
        for e in &self.done {
            let mut extra = Vec::new();
            let (mut s32, mut c32, mut o32) = (e.size as u32, e.csize as u32, e.offset as u32);
            if e.big {
                (s32, c32) = (BIG as u32, BIG as u32);
                extra.extend(e.size.to_le_bytes());
                extra.extend(e.csize.to_le_bytes());
            }
            if e.offset >= self.zip64_at {
                o32 = BIG as u32;
                extra.extend(e.offset.to_le_bytes());
            }
            let zip64 = !extra.is_empty();
            cd.extend(CENTRAL.to_le_bytes());
            cd.extend((if zip64 { 45u16 } else { 20 }).to_le_bytes());
            cd.extend((if zip64 { 45u16 } else { 20 }).to_le_bytes());
            cd.extend(UTF8.to_le_bytes());
            cd.extend(8u16.to_le_bytes());
            cd.extend(self.when.1.to_le_bytes());
            cd.extend(self.when.0.to_le_bytes());
            cd.extend(e.crc.to_le_bytes());
            cd.extend(c32.to_le_bytes());
            cd.extend(s32.to_le_bytes());
            cd.extend((e.name.len() as u16).to_le_bytes());
            cd.extend((if zip64 { extra.len() as u16 + 4 } else { 0 }).to_le_bytes());
            cd.extend([0u8; 2 + 2 + 2 + 4]); // comment, disk, internal and external attributes
            cd.extend(o32.to_le_bytes());
            cd.extend(e.name.as_bytes());
            if zip64 {
                cd.extend(1u16.to_le_bytes());
                cd.extend((extra.len() as u16).to_le_bytes());
                cd.extend(&extra);
            }
        }
        self.w.write_all(&cd)?;
        let n = self.done.len() as u64;
        let (size, at) = (cd.len() as u64, start);
        let zip64 = n >= 0xFFFF || size >= self.zip64_at || at >= self.zip64_at;
        if zip64 {
            let here = self.w.stream_position()?;
            let mut z = Vec::new();
            z.extend(END64.to_le_bytes());
            z.extend(44u64.to_le_bytes());
            z.extend(45u16.to_le_bytes());
            z.extend(45u16.to_le_bytes());
            z.extend([0u8; 8]); // this disk and the directory's
            z.extend(n.to_le_bytes());
            z.extend(n.to_le_bytes());
            z.extend(size.to_le_bytes());
            z.extend(at.to_le_bytes());
            z.extend(LOCATOR64.to_le_bytes());
            z.extend(0u32.to_le_bytes());
            z.extend(here.to_le_bytes());
            z.extend(1u32.to_le_bytes());
            self.w.write_all(&z)?;
        }
        let mut end = Vec::new();
        end.extend(END.to_le_bytes());
        end.extend([0u8; 4]);
        let count = if zip64 { 0xFFFF } else { n as u16 };
        end.extend(count.to_le_bytes());
        end.extend(count.to_le_bytes());
        end.extend((if zip64 { BIG as u32 } else { size as u32 }).to_le_bytes());
        end.extend((if zip64 { BIG as u32 } else { at as u32 }).to_le_bytes());
        end.extend(0u16.to_le_bytes());
        self.w.write_all(&end)?;
        self.w.flush()?;
        Ok(self.w)
    }
}

/// An entry of a zip file read.
#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    /// Its size once read.
    pub size: u64,
    method: u16,
    crc: u32,
    csize: u64,
    offset: u64,
}

impl Entry {
    /// A folder (a name ending in `/`).
    pub fn is_dir(&self) -> bool {
        self.name.ends_with('/')
    }
}

fn u16_at(b: &[u8], i: usize) -> io::Result<u16> {
    b.get(i..i + 2).map(|s| u16::from_le_bytes([s[0], s[1]])).ok_or_else(|| bad("a zip file cut short"))
}

fn u32_at(b: &[u8], i: usize) -> io::Result<u32> {
    b.get(i..i + 4).map(|s| u32::from_le_bytes(s.try_into().unwrap())).ok_or_else(|| bad("a zip file cut short"))
}

fn u64_at(b: &[u8], i: usize) -> io::Result<u64> {
    b.get(i..i + 8).map(|s| u64::from_le_bytes(s.try_into().unwrap())).ok_or_else(|| bad("a zip file cut short"))
}

/// A zip file read: its entries, each [extracted](Archive::extract) on its own.
pub struct Archive<R: Read + Seek> {
    r: R,
    pub entries: Vec<Entry>,
}

impl<R: Read + Seek> Archive<R> {
    pub fn open(mut r: R) -> io::Result<Archive<R>> {
        let len = r.seek(SeekFrom::End(0))?;
        let tail = len.min(22 + 0xFFFF + 20);
        r.seek(SeekFrom::Start(len - tail))?;
        let mut b = vec![0u8; tail as usize];
        r.read_exact(&mut b)?;
        let at = (0..b.len().saturating_sub(21))
            .rev()
            .find(|&i| u32_at(&b, i).is_ok_and(|s| s == END))
            .ok_or_else(|| bad("not a zip file (no end of directory)"))?;
        let (mut n, mut size, mut start) =
            (u16_at(&b, at + 10)? as u64, u32_at(&b, at + 12)? as u64, u32_at(&b, at + 16)? as u64);
        if (n == 0xFFFF || size == BIG || start == BIG) && at >= 20 && u32_at(&b, at - 20)? == LOCATOR64 {
            let z = u64_at(&b, at - 20 + 8)?;
            r.seek(SeekFrom::Start(z))?;
            let mut h = [0u8; 56];
            r.read_exact(&mut h)?;
            if u32_at(&h, 0)? != END64 {
                return Err(bad("a zip file with a broken ZIP64 directory"));
            }
            (n, size, start) = (u64_at(&h, 32)?, u64_at(&h, 40)?, u64_at(&h, 48)?);
        }
        if size > len || start > len {
            return Err(bad("a zip file whose directory is past its end"));
        }
        r.seek(SeekFrom::Start(start))?;
        let mut cd = vec![0u8; size as usize];
        r.read_exact(&mut cd)?;
        let mut entries = Vec::new();
        let mut i = 0;
        for _ in 0..n {
            if u32_at(&cd, i)? != CENTRAL {
                return Err(bad("a zip file with a broken directory"));
            }
            let method = u16_at(&cd, i + 10)?;
            let crc = u32_at(&cd, i + 16)?;
            let (mut csize, mut usize_) = (u32_at(&cd, i + 20)? as u64, u32_at(&cd, i + 24)? as u64);
            let (nl, el, cl) =
                (u16_at(&cd, i + 28)? as usize, u16_at(&cd, i + 30)? as usize, u16_at(&cd, i + 32)? as usize);
            let mut offset = u32_at(&cd, i + 42)? as u64;
            let name =
                String::from_utf8_lossy(cd.get(i + 46..i + 46 + nl).ok_or_else(|| bad("cut short"))?).into_owned();
            let extra = cd.get(i + 46 + nl..i + 46 + nl + el).ok_or_else(|| bad("cut short"))?;
            let mut j = 0;
            while j + 4 <= extra.len() {
                let (id, l) = (u16_at(extra, j)?, u16_at(extra, j + 2)? as usize);
                if id == 1 {
                    let mut k = j + 4;
                    for field in [&mut usize_, &mut csize, &mut offset] {
                        if *field == BIG {
                            *field = u64_at(extra, k)?;
                            k += 8;
                        }
                    }
                }
                j += 4 + l;
            }
            entries.push(Entry { name, size: usize_, method, crc, csize, offset });
            i += 46 + nl + el + cl;
        }
        Ok(Archive { r, entries })
    }

    /// The entry named `name`.
    pub fn find(&self, name: &str) -> Option<usize> {
        self.entries.iter().position(|e| e.name == name)
    }

    /// Write the entry `i` to `out`, its CRC-32 and size checked.
    pub fn extract(&mut self, i: usize, out: &mut dyn Write) -> io::Result<()> {
        let e = self.entries.get(i).ok_or_else(|| bad("no such entry"))?.clone();
        self.r.seek(SeekFrom::Start(e.offset))?;
        let mut h = [0u8; 30];
        self.r.read_exact(&mut h)?;
        if u32_at(&h, 0)? != LOCAL {
            return Err(bad(format!("{}: its header is not where the directory says", e.name)));
        }
        let skip = u16_at(&h, 26)? as i64 + u16_at(&h, 28)? as i64;
        self.r.seek(SeekFrom::Current(skip))?;
        let body = (&mut self.r).take(e.csize);
        let mut src: Box<dyn Read + '_> = match e.method {
            0 => Box::new(body),
            8 => Box::new(DeflateDecoder::new(body)),
            m => return Err(io::Error::new(io::ErrorKind::Unsupported, format!("{}: compression method {m}", e.name))),
        };
        let mut crc = Crc::new();
        let mut total = 0u64;
        let mut buf = vec![0u8; 1 << 20];
        loop {
            let n = src.read(&mut buf)?;
            if n == 0 {
                break;
            }
            crc.update(&buf[..n]);
            total += n as u64;
            out.write_all(&buf[..n])?;
        }
        if crc.sum() != e.crc || total != e.size {
            return Err(bad(format!("{}: damaged (its CRC-32 or its size is not the one written)", e.name)));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn zip(zip64_at: u64, files: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let mut w = Writer::new(Cursor::new(Vec::new()), 1_700_000_000);
        w.zip64_at = zip64_at;
        for (n, d) in files {
            w.add(n, d.len() as u64, &mut d.as_slice()).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    fn files() -> Vec<(&'static str, Vec<u8>)> {
        vec![
            ("a.txt", b"hello hello hello hello".to_vec()),
            ("data/ü.bin", (0..5000u32).flat_map(|i| (i % 251).to_le_bytes()).collect()),
            ("empty", Vec::new()),
        ]
    }

    #[test]
    fn what_is_written_is_read_back_whole_and_checked() {
        for at in [BIG, 100] {
            let bytes = zip(at, &files());
            let mut a = Archive::open(Cursor::new(bytes.clone())).unwrap();
            assert_eq!(a.entries.len(), 3);
            for (k, (name, data)) in files().iter().enumerate() {
                assert_eq!(a.entries[k].name, *name);
                let mut out = Vec::new();
                a.extract(k, &mut out).unwrap();
                assert_eq!(&out, data, "{name} (zip64 from {at})");
            }
            // a byte of the data changed: the CRC says so
            let mut broken = bytes;
            broken[36] ^= 0x55;
            let mut a = Archive::open(Cursor::new(broken)).unwrap();
            assert!((0..3).any(|k| a.extract(k, &mut Vec::new()).is_err()));
        }
    }

    #[test]
    fn the_date_is_the_one_given() {
        assert_eq!(civil(1_700_000_000), (2023, 11, 14, 22, 13, 20));
        assert_eq!(civil(0), (1970, 1, 1, 0, 0, 0));
        let (d, t) = dos(1_700_000_000);
        assert_eq!((d >> 9) + 1980, 2023);
        assert_eq!((d >> 5) & 15, 11);
        assert_eq!(t >> 11, 22);
    }

    #[test]
    fn a_file_that_is_not_a_zip_is_refused() {
        assert!(Archive::open(Cursor::new(b"just text, nothing like a zip".to_vec())).is_err());
        assert!(Archive::open(Cursor::new(Vec::new())).is_err());
    }
}
