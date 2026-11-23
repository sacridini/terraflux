//! Small pictures of what a node gives, drawn inside its box on the canvas: a map, the first layer of a stack, the
//! last step of a series, the fill of objects, rasters on disk (read decimated, never whole). They are coloured as
//! the Viewer colours the same output by default ([`Look::of`]: the classes in the class palette, a continuous map
//! along the default ramp over the 2nd to 98th percentiles), so a box shows what opening the node will show.
//!
//! Nothing here runs in a frame: [`Thumbs`] queues the work to one background thread and the UI only asks for
//! the finished texture. The text of what has no picture (tables, numbers, models) is `value_shape::describe`'s;
//! the caller draws that when [`has_picture`] says no or [`Thumbs::get`] gives nothing. Vectors have no picture
//! yet (the Viewer draws them by tiles, by a style).

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use egui::{Color32, ColorImage, Context, TextureHandle, TextureOptions};
use terraflux_ops::{Map, Value};
use terraflux_raster::tiff::Window;

use crate::app::Look;
use crate::palette::Coloring;

/// The most cells whose values choose the colours (a sample by stride beyond it).
const SAMPLE: usize = 2_000_000;

/// The cells of a block looked at to give its pixel (at most 4 x 4, by stride).
const PER_SIDE: usize = 4;

/// How much larger than the picture rasters on disk are read (the long side, in pixels), so that the blocks
/// still have cells to average and the colours are chosen over more than the pixels.
const OVERSAMPLE: usize = 4;

/// Whether a value has a picture (what [`thumbnail`] can draw): the caller asks before it clones a value.
pub fn has_picture(v: &Value) -> bool {
    match v {
        Value::Map(_) | Value::Stack(_) | Value::Raster(_) | Value::Objects(_) => true,
        Value::Series(items) => items.last().is_some_and(has_picture),
        _ => false,
    }
}

/// The picture of a value, its longest side `max_side` pixels (the map's aspect kept, never larger than the
/// map), transparent where the cells have no value; none when it has no picture or no cell has a value.
pub fn thumbnail(v: &Value, max_side: usize) -> Option<ColorImage> {
    let max_side = max_side.max(1);
    match v {
        Value::Map(m) => of_map(m, max_side),
        // (the layer the Viewer shows first)
        Value::Stack(st) => {
            let data = st.layers.first()?;
            of_map(&Map { grid: st.grid.clone(), data: data.clone() }, max_side)
        }
        Value::Series(items) => thumbnail(items.last()?, max_side),
        Value::Raster(r) => {
            let layer = r.chosen.min(r.layers.len().checked_sub(1)?);
            let g = r.grid_of(layer);
            let (w, h) = (g.width, g.height);
            if w == 0 || h == 0 {
                return None;
            }
            // decimated: every step-th row and column, a few cells for each pixel
            let step = w.max(h).div_ceil(max_side * OVERSAMPLE).max(1);
            let cells = r.sample(layer, Window { x0: 0, y0: 0, width: w, height: h }, step).ok()?;
            let (sw, sh) = (w.div_ceil(step), h.div_ceil(step));
            let values: Vec<f64> = cells.iter().map(|&x| f64::from(x)).collect();
            let look = Look::of(&values, None);
            render(&values, sw, sh, max_side, &look.coloring)
        }
        Value::Objects(o) => {
            // the fill the Viewer draws by default: by class when they have classes, else nothing
            let f = crate::builder::objects_fill(o, None);
            let (h, w) = f.map.shape();
            render(&f.map.data, w, h, max_side, &f.coloring)
        }
        _ => None,
    }
}

/// The picture of a map in memory.
fn of_map(m: &Map, max_side: usize) -> Option<ColorImage> {
    let (h, w) = m.shape();
    if m.data.len() < w * h {
        return None;
    }
    // the colours from the cells themselves, or from a sample by stride when they are many
    let stride = (w * h).div_ceil(SAMPLE).max(1);
    let look = if stride == 1 {
        Look::of(&m.data[..w * h], None)
    } else {
        let sample: Vec<f64> = m.data[..w * h].iter().step_by(stride).copied().collect();
        Look::of(&sample, None)
    };
    render(&m.data, w, h, max_side, &look.coloring)
}

/// The picture of `w` x `h` cells: each pixel is the block of cells under it, looked at through at most
/// 4 x 4 of them by stride (the cost is the pixels', not the map's): the most common class of a block for
/// classes, the mean of its values for a continuous colouring (NaN skipped), nothing where it has none.
fn render(values: &[f64], w: usize, h: usize, max_side: usize, coloring: &Coloring) -> Option<ColorImage> {
    if w == 0 || h == 0 || values.len() < w * h {
        return None;
    }
    let k = (max_side as f64 / w.max(h) as f64).min(1.0);
    let ow = ((w as f64 * k).round() as usize).max(1);
    let oh = ((h as f64 * k).round() as usize).max(1);
    let mean = matches!(coloring, Coloring::Continuous { .. });
    let mut pixels = Vec::with_capacity(ow * oh);
    let mut seen_any = false;
    let mut block: Vec<f64> = Vec::with_capacity(PER_SIDE * PER_SIDE);
    for oy in 0..oh {
        let (y0, y1) = (oy * h / oh, ((oy + 1) * h / oh).max(oy * h / oh + 1).min(h));
        let sy = (y1 - y0).div_ceil(PER_SIDE).max(1);
        for ox in 0..ow {
            let (x0, x1) = (ox * w / ow, ((ox + 1) * w / ow).max(ox * w / ow + 1).min(w));
            let sx = (x1 - x0).div_ceil(PER_SIDE).max(1);
            block.clear();
            for y in (y0 + sy / 2..y1).step_by(sy) {
                for x in (x0 + sx / 2..x1).step_by(sx) {
                    let v = values[y * w + x];
                    if !v.is_nan() {
                        block.push(v);
                    }
                }
            }
            let c = if block.is_empty() {
                Color32::TRANSPARENT
            } else {
                let v = if mean { block.iter().sum::<f64>() / block.len() as f64 } else { mode(&mut block) };
                seen_any = true;
                coloring.colour(v)
            };
            pixels.push(c);
        }
    }
    seen_any.then(|| ColorImage::new([ow, oh], pixels))
}

/// The most common value of a few (the least of the tied).
fn mode(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    let (mut best, mut best_n) = (v[0], 0);
    let mut i = 0;
    while i < v.len() {
        let j = v[i..].iter().take_while(|x| **x == v[i]).count();
        if j > best_n {
            (best, best_n) = (v[i], j);
        }
        i += j;
    }
    best
}

/// The key of a thumbnail: a node and a port.
pub type Key = (String, String);

/// The most thumbnails kept.
pub const MOST: usize = 512;

/// The longest side of the pictures made, in pixels.
pub const SIDE: usize = 96;

struct Job {
    key: Key,
    generation: u64,
    value: Value,
}

struct Entry {
    generation: u64,
    /// The picture made, not yet uploaded.
    image: Option<ColorImage>,
    /// Whether this generation has a picture at all (a value that gives none is not tried again).
    drawn: bool,
    texture: Option<TextureHandle>,
    /// When it was last made or asked for (the oldest goes first beyond the bound).
    used: u64,
}

#[derive(Default)]
struct State {
    done: HashMap<Key, Entry>,
    /// The generation asked for last, by key, while its picture is not made.
    wanted: HashMap<Key, u64>,
    clock: u64,
}

/// The pictures of what nodes give, made by one background thread and uploaded as textures on the UI thread. A
/// picture is asked for by node and port with the generation of the value (a run's counter, say): the same
/// generation twice is made once, an older one than asked for is dropped, and the picture of the last
/// generation stays shown until the new one is ready.
pub struct Thumbs {
    state: Arc<Mutex<State>>,
    jobs: Sender<Job>,
    most: usize,
}

impl Thumbs {
    pub fn new(ctx: &Context) -> Thumbs {
        Thumbs::with_limit(ctx, MOST)
    }

    /// As [`Thumbs::new`], keeping `most` pictures at most.
    pub fn with_limit(ctx: &Context, most: usize) -> Thumbs {
        let (jobs, rx) = channel();
        let state = Arc::new(Mutex::new(State::default()));
        let (st, ctx, most) = (state.clone(), ctx.clone(), most.max(1));
        // (it ends when the Thumbs is dropped: the channel closes)
        std::thread::Builder::new()
            .name("thumbs".into())
            .spawn(move || work(rx, st, ctx, most))
            .expect("the thumbnail thread");
        Thumbs { state, jobs, most }
    }

    /// Asks for the picture of a node's output at a generation; nothing when it is made or asked for already (or
    /// a later generation is), or when the value has no picture.
    pub fn request(&self, key: Key, generation: u64, value: Value) {
        if !has_picture(&value) {
            return;
        }
        {
            let mut s = self.state.lock().unwrap();
            let known = s.done.get(&key).map(|e| e.generation);
            let asked = s.wanted.get(&key).copied();
            if known.is_some_and(|g| g >= generation) || asked.is_some_and(|g| g >= generation) {
                return;
            }
            s.wanted.insert(key.clone(), generation);
        }
        let _ = self.jobs.send(Job { key, generation, value });
    }

    /// The texture of a key's picture (named `thumb:<node>.<port>`, linear filtering), when it is made.
    pub fn get(&self, ctx: &Context, key: &Key) -> Option<TextureHandle> {
        let mut s = self.state.lock().unwrap();
        s.clock += 1;
        let clock = s.clock;
        let e = s.done.get_mut(key)?;
        e.used = clock;
        if let Some(img) = e.image.take() {
            e.texture = Some(ctx.load_texture(format!("thumb:{}.{}", key.0, key.1), img, TextureOptions::LINEAR));
        }
        if e.drawn { e.texture.clone() } else { None }
    }

    /// The generation of a key's picture made last.
    pub fn generation(&self, key: &Key) -> Option<u64> {
        self.state.lock().unwrap().done.get(key).map(|e| e.generation)
    }

    /// Whether anything is still to be made.
    pub fn busy(&self) -> bool {
        !self.state.lock().unwrap().wanted.is_empty()
    }

    /// How many pictures are kept.
    pub fn len(&self) -> usize {
        self.state.lock().unwrap().done.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Forgets every picture (and what was asked for: what is being made is dropped when it arrives).
    pub fn clear(&self) {
        let mut s = self.state.lock().unwrap();
        s.done.clear();
        s.wanted.clear();
    }

    /// The most pictures kept.
    pub fn limit(&self) -> usize {
        self.most
    }
}

/// The thread: makes each job asked for, keeps it unless a later one was asked for, drops the oldest beyond
/// the bound, and wakes the UI.
fn work(rx: Receiver<Job>, state: Arc<Mutex<State>>, ctx: Context, most: usize) {
    while let Ok(job) = rx.recv() {
        // (asked again at a later generation, or cleared, while it waited)
        if state.lock().unwrap().wanted.get(&job.key) != Some(&job.generation) {
            continue;
        }
        let image = thumbnail(&job.value, SIDE);
        drop(job.value);
        let mut s = state.lock().unwrap();
        if s.wanted.get(&job.key) != Some(&job.generation) {
            continue;
        }
        s.wanted.remove(&job.key);
        s.clock += 1;
        let used = s.clock;
        let texture = s.done.get(&job.key).and_then(|e| e.texture.clone());
        let drawn = image.is_some();
        // (the last picture stays while this one has none to show: no flicker, no stale one either)
        let texture = if drawn { texture } else { None };
        s.done.insert(job.key.clone(), Entry { generation: job.generation, image, drawn, texture, used });
        while s.done.len() > most {
            let oldest =
                s.done.iter().filter(|(k, _)| **k != job.key).min_by_key(|(_, e)| e.used).map(|(k, _)| k.clone());
            match oldest {
                Some(k) => {
                    s.done.remove(&k);
                }
                None => break,
            }
        }
        drop(s);
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::{Ramp, seen};
    use terraflux_raster::Grid;

    fn map(w: usize, h: usize, f: impl Fn(usize, usize) -> f64) -> Value {
        let data = (0..w * h).map(|i| f(i % w, i / w)).collect();
        Value::Map(Map::new(Arc::new(Grid::new(w, h)), data))
    }

    fn wait(t: &Thumbs, ctx: &Context, key: &Key) -> Option<TextureHandle> {
        for _ in 0..500 {
            if !t.busy() {
                return t.get(ctx, key);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("the thumbnail was not made in 5 s");
    }

    fn key(n: &str) -> Key {
        (n.to_string(), "map".to_string())
    }

    #[test]
    fn a_categorical_map_keeps_its_aspect_and_the_viewers_colours() {
        // 400 x 200, classes 1, 2, 3 in thirds along x, a column of no data on the left
        let v = map(400, 200, |x, _| if x < 10 { f64::NAN } else { [1.0, 2.0, 3.0][x * 3 / 400] });
        let img = thumbnail(&v, 96).unwrap();
        assert_eq!(img.size, [96, 48]);
        let Value::Map(m) = &v else { unreachable!() };
        let Coloring::Classes(_) = Look::of(&m.data, None).coloring else { panic!("classes") };
        let co = Look::of(&m.data, None).coloring;
        // the middle of each third
        for (px, class) in [(16, 1.0), (48, 2.0), (80, 3.0)] {
            assert_eq!(img.pixels[24 * 96 + px], co.colour(class), "class {class}");
            assert_ne!(co.colour(class).a(), 0);
        }
        // no data: transparent
        assert_eq!(img.pixels[24 * 96], Color32::TRANSPARENT);
    }

    #[test]
    fn a_continuous_map_has_the_viewers_ramp_at_its_ends() {
        let v = map(1000, 100, |x, _| x as f64 + 0.5);
        let img = thumbnail(&v, 50).unwrap();
        assert_eq!(img.size, [50, 5]);
        let row = &img.pixels[2 * 50..3 * 50];
        assert_eq!(row[0], seen(Ramp::Viridis.colour(0.0)));
        assert_eq!(row[49], seen(Ramp::Viridis.colour(1.0)));
        assert_ne!(row[0], row[25]);
        // a small map is not enlarged
        assert_eq!(thumbnail(&map(10, 4, |x, _| x as f64 + 0.5), 96).unwrap().size, [10, 4]);
        // a map without a value has no picture
        assert!(thumbnail(&map(5, 5, |_, _| f64::NAN), 96).is_none());
        assert!(thumbnail(&Value::Number(1.0), 96).is_none());
    }

    #[test]
    fn a_big_map_costs_the_pixels_not_the_cells() {
        let v = map(4000, 3000, |x, y| ((x / 500 + y / 500) % 4) as f64);
        let t = std::time::Instant::now();
        let img = thumbnail(&v, 96).unwrap();
        let took = t.elapsed();
        assert_eq!(img.size, [96, 72]);
        assert!(took.as_secs_f64() < 2.0, "{took:?}");
        // the last of a series and the first of a stack are the maps' pictures
        let s = Value::Series(vec![map(3, 3, |_, _| f64::NAN), v.clone()]);
        assert_eq!(thumbnail(&s, 96).unwrap().size, [96, 72]);
    }

    #[test]
    fn the_cache_gives_a_texture_drops_older_generations_and_is_bounded() {
        let ctx = Context::default();
        let t = Thumbs::new(&ctx);
        let k = key("a");
        assert!(t.get(&ctx, &k).is_none());
        t.request(k.clone(), 2, map(20, 10, |x, _| (x % 3) as f64));
        let tex = wait(&t, &ctx, &k).expect("a texture");
        assert_eq!(tex.size(), [20, 10]);
        assert_eq!(t.generation(&k), Some(2));
        // an older generation, or the same, is ignored
        t.request(k.clone(), 1, map(10, 10, |x, _| (x % 3) as f64));
        t.request(k.clone(), 2, map(10, 10, |x, _| (x % 3) as f64));
        assert!(!t.busy());
        assert_eq!(wait(&t, &ctx, &k).unwrap().size(), [20, 10]);
        // a later one replaces it
        t.request(k.clone(), 3, map(10, 10, |x, _| (x % 3) as f64));
        assert_eq!(wait(&t, &ctx, &k).unwrap().size(), [10, 10]);
        assert_eq!(t.generation(&k), Some(3));
        // no picture: none, and not tried again
        t.request(key("n"), 1, Value::Number(3.0));
        assert!(!t.busy() && t.get(&ctx, &key("n")).is_none());
        t.clear();
        assert!(t.is_empty());

        let small = Thumbs::with_limit(&ctx, 4);
        for i in 0..10 {
            small.request(key(&i.to_string()), 1, map(8, 8, |x, _| (x % 3) as f64));
        }
        wait(&small, &ctx, &key("9"));
        assert!(small.len() <= 4, "{}", small.len());
        assert!(small.get(&ctx, &key("9")).is_some());
    }
}
