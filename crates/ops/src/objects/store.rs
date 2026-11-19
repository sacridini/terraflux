//! Objects kept on disk, losslessly: the labels, the table (every bit of its numbers), the classes, the memberships
//! and the levels above, gzipped. What the labels give (cells, boxes, adjacency) is made again when they are read.

use std::io::{Read, Write};
use std::sync::Arc;

use terraflux_raster::Grid;

use super::{ObjectClass, Objects};
use crate::{Error, Result, Table};

const MAGIC: &[u8; 5] = b"TFOB1";

fn bad(what: &str) -> Error {
    Error::msg(format!("objects: the file is damaged ({what})"))
}

#[derive(Default)]
struct Out(Vec<u8>);

impl Out {
    fn u64(&mut self, x: u64) {
        self.0.extend_from_slice(&x.to_le_bytes());
    }

    fn bytes(&mut self, b: &[u8]) {
        self.u64(b.len() as u64);
        self.0.extend_from_slice(b);
    }

    fn u32s(&mut self, v: &[u32]) {
        self.u64(v.len() as u64);
        for x in v {
            self.0.extend_from_slice(&x.to_le_bytes());
        }
    }

    fn f64s(&mut self, v: &[f64]) {
        self.u64(v.len() as u64);
        for x in v {
            self.0.extend_from_slice(&x.to_bits().to_le_bytes());
        }
    }
}

struct In<'a>(&'a [u8]);

impl<'a> In<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.0.len() < n {
            return Err(bad("it ends early"));
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }

    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    /// A length, not above what is left to read at `each` bytes an item (so a damaged one cannot ask for a lot).
    fn len(&mut self, each: usize) -> Result<usize> {
        let n = self.u64()? as usize;
        if n.checked_mul(each).is_none_or(|b| b > self.0.len()) {
            return Err(bad("a length"));
        }
        Ok(n)
    }

    fn bytes(&mut self) -> Result<&'a [u8]> {
        let n = self.len(1)?;
        self.take(n)
    }

    fn text(&mut self) -> Result<String> {
        String::from_utf8(self.bytes()?.to_vec()).map_err(|_| bad("a text"))
    }

    fn u32s(&mut self) -> Result<Vec<u32>> {
        let n = self.len(4)?;
        Ok(self.take(n * 4)?.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c)).collect())
    }

    fn f64s(&mut self) -> Result<Vec<f64>> {
        let n = self.len(8)?;
        Ok(self.take(n * 8)?.as_chunks::<8>().0.iter().map(|c| f64::from_bits(u64::from_le_bytes(*c))).collect())
    }
}

fn write_level(o: &Objects, w: &mut Out, with_grid: bool) -> Result<()> {
    if with_grid {
        let grid = serde_json::to_vec(&*o.grid).map_err(|e| Error::msg(format!("objects: the grid: {e}")))?;
        w.bytes(&grid);
    }
    w.u64(o.count as u64);
    w.u32s(&o.labels);
    // the table: its columns, the dictionaries, a row after another
    let columns = serde_json::to_vec(&(&o.table.columns, &o.table.labels)).map_err(|e| Error::msg(e.to_string()))?;
    w.bytes(&columns);
    w.u64(o.table.rows.len() as u64);
    for r in &o.table.rows {
        w.f64s(r);
    }
    w.u64(o.classes.len() as u64);
    for c in &o.classes {
        w.bytes(c.name.as_bytes());
        match c.colour {
            Some([r, g, b]) => w.0.extend_from_slice(&[1, r, g, b]),
            None => w.0.extend_from_slice(&[0, 0, 0, 0]),
        }
    }
    w.u32s(&o.class_of);
    w.u64(o.membership.len() as u64);
    for m in &o.membership {
        w.f64s(m);
    }
    match &o.parent {
        Some(p) => {
            w.u64(1);
            w.u32s(&o.parent_of);
            write_level(p, w, false)?;
        }
        None => w.u64(0),
    }
    Ok(())
}

fn read_level(r: &mut In, grid: Option<Arc<Grid>>) -> Result<Objects> {
    let grid = match grid {
        Some(g) => g,
        None => Arc::new(serde_json::from_slice::<Grid>(r.bytes()?).map_err(|_| bad("the grid"))?),
    };
    let count = r.u64()? as usize;
    let labels = r.u32s()?;
    let mut o = Objects::from_labels(grid.clone(), labels, count)?;
    let (columns, dictionaries): (Vec<String>, _) =
        serde_json::from_slice(r.bytes()?).map_err(|_| bad("the table's columns"))?;
    let mut table = Table::new(columns);
    table.labels = dictionaries;
    for _ in 0..r.len(8)? {
        table.rows.push(r.f64s()?);
    }
    o = o.with_table(table)?;
    let mut classes = Vec::new();
    for _ in 0..r.len(12)? {
        let name = r.text()?;
        let c = r.take(4)?;
        classes.push(ObjectClass { name, colour: (c[0] == 1).then_some([c[1], c[2], c[3]]) });
    }
    let class_of = r.u32s()?;
    let mut membership = Vec::new();
    for _ in 0..r.len(8)? {
        membership.push(r.f64s()?);
    }
    o = o.with_classes(classes, class_of, membership)?;
    if r.u64()? == 1 {
        let parent_of = r.u32s()?;
        let parent = read_level(r, Some(grid))?;
        o = o.with_parent(Arc::new(parent), parent_of)?;
    }
    Ok(o)
}

impl Objects {
    /// The objects as bytes (gzipped), what [`Objects::from_bytes`] reads back.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut w = Out::default();
        w.0.extend_from_slice(MAGIC);
        write_level(self, &mut w, true)?;
        let mut z = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        z.write_all(&w.0).map_err(|e| Error::msg(e.to_string()))?;
        z.finish().map_err(|e| Error::msg(e.to_string()))
    }

    /// Objects from the bytes [`Objects::to_bytes`] made.
    pub fn from_bytes(bytes: &[u8]) -> Result<Objects> {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(bytes).read_to_end(&mut raw).map_err(|_| bad("not gzip"))?;
        let mut r = In(&raw);
        if r.take(MAGIC.len())? != MAGIC {
            return Err(bad("not objects"));
        }
        read_level(&mut r, None)
    }
}
