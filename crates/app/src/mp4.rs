//! H.264 frames in an MP4 file, as players, browsers and slides take it: the boxes of one video track (its
//! description first, so that it plays before it is all read), the frames' NAL units length-prefixed in the data.

/// A box: its size, its kind, its content.
fn boxed(kind: &[u8; 4], content: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(content.len() + 8);
    b.extend_from_slice(&(content.len() as u32 + 8).to_be_bytes());
    b.extend_from_slice(kind);
    b.extend_from_slice(content);
    b
}

/// A full box: a box with a version and flags first.
fn full(kind: &[u8; 4], version: u8, flags: u32, content: &[u8]) -> Vec<u8> {
    let mut c = Vec::with_capacity(content.len() + 4);
    c.extend_from_slice(&((u32::from(version) << 24) | (flags & 0x00FF_FFFF)).to_be_bytes());
    c.extend_from_slice(content);
    boxed(kind, &c)
}

fn u16s(v: &[u16]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_be_bytes()).collect()
}

fn u32s(v: &[u32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_be_bytes()).collect()
}

/// The identity matrix of a movie's or a track's display.
const MATRIX: [u32; 9] = [0x0001_0000, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000];

/// The NAL units of an Annex B stream (each after its start code, 00 00 01 or 00 00 00 01).
pub fn nal_units(stream: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= stream.len() {
        if stream[i] == 0 && stream[i + 1] == 0 && stream[i + 2] == 1 {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    let mut out = Vec::with_capacity(starts.len());
    for (k, &s) in starts.iter().enumerate() {
        let mut e = starts.get(k + 1).map_or(stream.len(), |&n| n - 3);
        // (a four-byte start code: its first zero not of this unit)
        while e > s && stream[e - 1] == 0 && k + 1 < starts.len() {
            e -= 1;
        }
        if e > s {
            out.push(&stream[s..e]);
        }
    }
    out
}

/// A video being written: its parameter sets, its frames (length-prefixed NAL units), whether each is a key
/// frame, how long each lasts.
#[derive(Default)]
pub struct Video {
    sps: Option<Vec<u8>>,
    pps: Option<Vec<u8>>,
    samples: Vec<Vec<u8>>,
    sync: Vec<bool>,
    durations: Vec<u32>,
}

impl Video {
    /// A frame of the encoder (an Annex B stream: its parameter sets kept, its picture's units the frame),
    /// shown `duration` milliseconds. A stream of no picture (an encoder skipping a frame the same as the last)
    /// lengthens the last frame.
    pub fn add(&mut self, stream: &[u8], duration: u32) {
        let mut sample = Vec::new();
        let mut key = false;
        for u in nal_units(stream) {
            match u[0] & 0x1F {
                7 => self.sps = Some(u.to_vec()),
                8 => self.pps = Some(u.to_vec()),
                // (the access unit delimiter and supplemental information: not needed)
                9 | 6 => {}
                t => {
                    key |= t == 5;
                    sample.extend_from_slice(&(u.len() as u32).to_be_bytes());
                    sample.extend_from_slice(u);
                }
            }
        }
        if sample.is_empty() {
            if let Some(d) = self.durations.last_mut() {
                *d += duration;
            }
            return;
        }
        self.samples.push(sample);
        self.sync.push(key);
        self.durations.push(duration);
    }

    /// The MP4 file of the frames, `width` by `height` pixels.
    pub fn finish(&self, width: u16, height: u16) -> Result<Vec<u8>, String> {
        let (Some(sps), Some(pps)) = (&self.sps, &self.pps) else {
            return Err("the encoder gave no parameter sets".into());
        };
        if self.samples.is_empty() || sps.len() < 4 {
            return Err("the encoder gave no frame".into());
        }
        let ftyp = boxed(b"ftyp", &[b"isom".as_slice(), &512u32.to_be_bytes(), b"isomiso2avc1mp41"].concat());
        // the description's size does not depend on where the data is: written once to know it, then again
        let at = |moov_len: usize| (ftyp.len() + moov_len + 8) as u32;
        let first = self.moov(width, height, sps, pps, 0);
        let moov = self.moov(width, height, sps, pps, at(first.len()));
        debug_assert_eq!(first.len(), moov.len());
        let data: usize = self.samples.iter().map(Vec::len).sum();
        let mut out = Vec::with_capacity(ftyp.len() + moov.len() + data + 8);
        out.extend_from_slice(&ftyp);
        out.extend_from_slice(&moov);
        out.extend_from_slice(&(data as u32 + 8).to_be_bytes());
        out.extend_from_slice(b"mdat");
        for s in &self.samples {
            out.extend_from_slice(s);
        }
        Ok(out)
    }

    /// The movie's description: one video track, its frames from `offset` on, one after another.
    fn moov(&self, width: u16, height: u16, sps: &[u8], pps: &[u8], offset: u32) -> Vec<u8> {
        const SCALE: u32 = 1000;
        let duration: u32 = self.durations.iter().sum();
        let mvhd = full(
            b"mvhd",
            0,
            0,
            &[
                u32s(&[0, 0, SCALE, duration, 0x0001_0000]),
                u16s(&[0x0100, 0]),
                u32s(&[0, 0]),
                u32s(&MATRIX),
                u32s(&[0; 6]),
                u32s(&[2]),
            ]
            .concat(),
        );
        let tkhd = full(
            b"tkhd",
            0,
            3,
            &[
                u32s(&[0, 0, 1, 0, duration, 0, 0]),
                u16s(&[0, 0, 0, 0]),
                u32s(&MATRIX),
                u32s(&[u32::from(width) << 16, u32::from(height) << 16]),
            ]
            .concat(),
        );
        // ("und": no language)
        let mdhd = full(b"mdhd", 0, 0, &[u32s(&[0, 0, SCALE, duration]), u16s(&[0x55C4, 0])].concat());
        let hdlr =
            full(b"hdlr", 0, 0, &[u32s(&[0]), b"vide".to_vec(), u32s(&[0, 0, 0]), b"VideoHandler\0".to_vec()].concat());
        let vmhd = full(b"vmhd", 0, 1, &u16s(&[0, 0, 0, 0]));
        let dinf = boxed(b"dinf", &full(b"dref", 0, 0, &[u32s(&[1]), full(b"url ", 0, 1, &[])].concat()));
        let avcc = boxed(
            b"avcC",
            &[
                vec![1, sps[1], sps[2], sps[3], 0xFF, 0xE1],
                (sps.len() as u16).to_be_bytes().to_vec(),
                sps.to_vec(),
                vec![1],
                (pps.len() as u16).to_be_bytes().to_vec(),
                pps.to_vec(),
            ]
            .concat(),
        );
        let avc1 = boxed(
            b"avc1",
            &[
                vec![0; 6],
                u16s(&[1, 0, 0]),
                u32s(&[0, 0, 0]),
                u16s(&[width, height]),
                u32s(&[0x0048_0000, 0x0048_0000, 0]),
                u16s(&[1]),
                vec![0; 32],
                u16s(&[0x0018, 0xFFFF]),
                avcc,
            ]
            .concat(),
        );
        let stsd = full(b"stsd", 0, 0, &[u32s(&[1]), avc1].concat());
        // the durations, the same ones run together
        let mut runs: Vec<(u32, u32)> = Vec::new();
        for &d in &self.durations {
            match runs.last_mut() {
                Some((n, last)) if *last == d => *n += 1,
                _ => runs.push((1, d)),
            }
        }
        let stts = full(
            b"stts",
            0,
            0,
            &[u32s(&[runs.len() as u32]), runs.iter().flat_map(|(n, d)| u32s(&[*n, *d])).collect()].concat(),
        );
        let keys: Vec<u32> = (1..).zip(&self.sync).filter(|(_, k)| **k).map(|(i, _)| i).collect();
        let stss = full(b"stss", 0, 0, &[u32s(&[keys.len() as u32]), u32s(&keys)].concat());
        // a chunk a frame
        let stsc = full(b"stsc", 0, 0, &u32s(&[1, 1, 1, 1]));
        let sizes: Vec<u32> = self.samples.iter().map(|s| s.len() as u32).collect();
        let stsz = full(b"stsz", 0, 0, &[u32s(&[0, sizes.len() as u32]), u32s(&sizes)].concat());
        let mut offsets = Vec::with_capacity(sizes.len());
        let mut at = offset;
        for s in &sizes {
            offsets.push(at);
            at += s;
        }
        let stco = full(b"stco", 0, 0, &[u32s(&[offsets.len() as u32]), u32s(&offsets)].concat());
        let stbl = boxed(b"stbl", &[stsd, stts, stss, stsc, stsz, stco].concat());
        let minf = boxed(b"minf", &[vmhd, dinf, stbl].concat());
        let mdia = boxed(b"mdia", &[mdhd, hdlr, minf].concat());
        let trak = boxed(b"trak", &[tkhd, mdia].concat());
        boxed(b"moov", &[mvhd, trak].concat())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The boxes of a run of bytes: (kind, content).
    fn boxes(b: &[u8]) -> Vec<(&[u8], &[u8])> {
        let mut out = Vec::new();
        let mut i = 0;
        while i + 8 <= b.len() {
            let size = u32::from_be_bytes(b[i..i + 4].try_into().unwrap()) as usize;
            assert!(size >= 8 && i + size <= b.len(), "a box of {size} at {i} of {}", b.len());
            out.push((&b[i + 4..i + 8], &b[i + 8..i + size]));
            i += size;
        }
        assert_eq!(i, b.len(), "the boxes fill it");
        out
    }

    fn find<'a>(b: &'a [u8], path: &[&[u8; 4]]) -> &'a [u8] {
        let (head, rest) = path.split_first().unwrap();
        let (_, c) = *boxes(b).iter().find(|(k, _)| k == head).unwrap_or_else(|| panic!("{:?}", head));
        if rest.is_empty() { c } else { find(c, rest) }
    }

    #[test]
    fn the_units_of_a_stream_and_a_file_of_them() {
        let stream = [
            &[0, 0, 0, 1, 0x67, 0x42, 0xC0, 0x1E, 9][..],
            &[0, 0, 0, 1, 0x68, 0xCE, 0x3C, 0x80],
            &[0, 0, 1, 0x65, 1, 2, 3, 0],
        ]
        .concat();
        let u = nal_units(&stream);
        assert_eq!(u, vec![&[0x67, 0x42, 0xC0, 0x1E, 9][..], &[0x68, 0xCE, 0x3C, 0x80], &[0x65, 1, 2, 3, 0]]);
        let mut v = Video::default();
        assert!(v.finish(2, 2).is_err(), "no frame");
        v.add(&stream, 250);
        v.add(&[0, 0, 0, 1, 0x41, 7, 7], 250);
        // (a frame skipped: the last one longer)
        v.add(&[], 250);
        v.add(&[0, 0, 0, 1, 0x41, 8], 1000);
        let f = v.finish(32, 18).unwrap();
        let top: Vec<&[u8]> = boxes(&f).iter().map(|(k, _)| *k).collect();
        assert_eq!(top, vec![b"ftyp".as_slice(), b"moov", b"mdat"]);
        let stbl = [b"moov", b"trak", b"mdia", b"minf", b"stbl"];
        let at = |name: &[u8; 4]| find(&f, &[&stbl[..], &[name]].concat());
        // the durations: 250, 500 (a frame skipped), 1000
        assert_eq!(at(b"stts"), &u32s(&[0, 3, 1, 250, 1, 500, 1, 1000])[..]);
        assert_eq!(at(b"stss"), &u32s(&[0, 1, 1])[..], "the first a key frame");
        assert_eq!(at(b"stsz"), &u32s(&[0, 0, 3, 9, 7, 6])[..]);
        let avc1 = &at(b"stsd")[8 + 8..];
        assert_eq!(&avc1[24..28], &[0, 32, 0, 18]);
        let avcc = find(&avc1[78..], &[b"avcC"]);
        assert_eq!(&avcc[..6], &[1, 0x42, 0xC0, 0x1E, 0xFF, 0xE1]);
        // each offset where its frame is: its first unit's length, then the unit
        let stco = at(b"stco");
        let offsets: Vec<usize> =
            stco[8..].chunks(4).map(|c| u32::from_be_bytes(c.try_into().unwrap()) as usize).collect();
        assert_eq!(&f[offsets[0]..offsets[0] + 9], &[0, 0, 0, 5, 0x65, 1, 2, 3, 0]);
        assert_eq!(&f[offsets[2]..], &[0, 0, 0, 2, 0x41, 8]);
        let mvhd = find(&f, &[b"moov", b"mvhd"]);
        assert_eq!(&mvhd[12..20], &u32s(&[1000, 1750])[..], "a thousandth of a second, 1.75 s");
    }
}
