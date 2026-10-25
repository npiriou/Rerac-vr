//! **Level content spread over frames** while a flight hides the level change (crate::vr::loading builds the
//! destination behind the ship's voyage). Every other level start adds its content at once, as before.
//!
//! * [`LevelStream::mesh`] / [`LevelStream::image`]: while [`LevelStream::deferring`], the handle is reserved at once
//!   (entities and materials use it right away) and the asset is inserted later, a budget per frame ([`drip`]), so the
//!   render world uploads the level over many frames instead of stalling one. An entity whose mesh or texture is not
//!   in yet is simply not drawn, as with any asset still loading.
//! * [`LevelStream::job`]: work resumed every frame within [`JOB_TIME`] (the ties' and shrubs' part entities, the old
//!   ties' despawn).
//! * [`run_later`] / [`drop_later`]: work moved off the main thread (freeing the old level's resources).
//!
//! [`LevelStream::pending`] holds the flight until everything is in (crate::vr::loading); a new level change first
//! finishes what is left ([`finish`]).

use bevy::prelude::*;
use std::collections::VecDeque;
use std::sync::mpsc::Sender;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Asset bytes inserted per frame (each inserted asset is extracted and uploaded in that frame's render).
const BYTES_PER_FRAME: usize = 4 << 20;
/// Images inserted per frame: each one is a GPU texture creation.
const IMAGES_PER_FRAME: usize = 12;
/// Meshes inserted per frame.
const MESHES_PER_FRAME: usize = 48;
/// Main-thread time given to the jobs per frame.
const JOB_TIME: Duration = Duration::from_micros(2500);

/// A resumable piece of a level start: runs on the world until the deadline and returns true once it is done.
pub type Job = Box<dyn FnMut(&mut World, Instant) -> bool + Send + Sync>;

enum Pending {
    Mesh(AssetId<Mesh>, Mesh),
    Image(AssetId<Image>, Image),
}

/// What a stream moved over its frames (the `VR_HITCH stream` line).
#[derive(Clone, Copy, Debug, Default)]
pub struct StreamStats {
    pub meshes: usize,
    pub images: usize,
    pub bytes: usize,
    pub jobs: usize,
    pub frames: u32,
    pub job_time: Duration,
}

/// The deferred part of a level start (module docs).
#[derive(Resource, Default)]
pub struct LevelStream {
    deferring: bool,
    assets: VecDeque<(Pending, usize)>,
    jobs: VecDeque<Job>,
    stats: StreamStats,
}

impl LevelStream {
    /// Whether the level start being run defers its content (set by crate::level_switch around the level start).
    pub fn deferring(&self) -> bool { self.deferring }

    pub(crate) fn set_deferring(&mut self, on: bool) {
        if on && !self.deferring && !self.pending() { self.stats = StreamStats::default(); }
        self.deferring = on;
    }

    /// `meshes.add(mesh)`, or its handle now and the mesh a few frames later while deferring.
    pub fn mesh(&mut self, meshes: &mut Assets<Mesh>, mesh: Mesh) -> Handle<Mesh> {
        if !self.deferring { return meshes.add(mesh); }
        let h = meshes.reserve_handle();
        let bytes = mesh_bytes(&mesh);
        self.assets.push_back((Pending::Mesh(h.id(), mesh), bytes));
        h
    }

    /// `images.add(image)`, or its handle now and the image a few frames later while deferring.
    pub fn image(&mut self, images: &mut Assets<Image>, image: Image) -> Handle<Image> {
        if !self.deferring { return images.add(image); }
        let h = images.reserve_handle();
        let bytes = image.data.as_ref().map_or(0, Vec::len);
        self.assets.push_back((Pending::Image(h.id(), image), bytes));
        h
    }

    /// Queues `job` (run from the next frame on, after the jobs queued before it).
    pub fn job(&mut self, job: Job) {
        self.jobs.push_back(job);
        self.stats.jobs += 1;
    }

    /// Assets or jobs are left.
    pub fn pending(&self) -> bool { !self.assets.is_empty() || !self.jobs.is_empty() }

    /// Read by crate::vr::loading (only a VR flight streams).
    #[cfg_attr(not(feature = "vr"), allow(dead_code))]
    pub fn stats(&self) -> StreamStats { self.stats }
}

/// Vertex and index bytes of `mesh` (what the render world uploads).
fn mesh_bytes(mesh: &Mesh) -> usize {
    let vertex: usize = mesh.attributes().map(|(a, _)| a.format.size() as usize).sum();
    vertex * mesh.count_vertices() + mesh.get_index_buffer_bytes().map_or(0, <[u8]>::len)
}

/// `Last`, before the level change: this frame's share of the stream (module docs).
pub fn drip(world: &mut World) {
    let Some(mut s) = world.get_resource_mut::<LevelStream>() else { return };
    if !s.pending() { return; }
    s.stats.frames += 1;
    let (mut bytes, mut n_images, mut n_meshes) = (0usize, 0usize, 0usize);
    let (mut meshes, mut images) = (Vec::new(), Vec::new());
    while let Some((p, n)) = s.assets.front() {
        let (n, full) = match p {
            Pending::Mesh(..) => (*n, n_meshes >= MESHES_PER_FRAME),
            Pending::Image(..) => (*n, n_images >= IMAGES_PER_FRAME),
        };
        // At least one asset per frame, however large.
        if full || (bytes > 0 && bytes + n > BYTES_PER_FRAME) { break; }
        bytes += n;
        match s.assets.pop_front().map(|(p, _)| p) {
            Some(Pending::Mesh(id, m)) => { n_meshes += 1; meshes.push((id, m)); }
            Some(Pending::Image(id, i)) => { n_images += 1; images.push((id, i)); }
            None => {}
        }
    }
    s.stats.meshes += n_meshes;
    s.stats.images += n_images;
    s.stats.bytes += bytes;
    insert(world, meshes, images);
    run_jobs(world, Instant::now() + JOB_TIME);
}

/// Everything left, now: a level change while the last one still streams (its content must be complete before
/// the old level's state is dropped).
pub fn finish(world: &mut World) {
    let Some(mut s) = world.get_resource_mut::<LevelStream>() else { return };
    if !s.pending() { return; }
    let (mut meshes, mut images) = (Vec::new(), Vec::new());
    for (p, _) in std::mem::take(&mut s.assets) {
        match p {
            Pending::Mesh(id, m) => meshes.push((id, m)),
            Pending::Image(id, i) => images.push((id, i)),
        }
    }
    println!("level stream: finishing {} meshes, {} images and {} jobs at once", meshes.len(), images.len(), s.jobs.len());
    insert(world, meshes, images);
    // A deadline that never passes: each job runs to its end.
    run_jobs(world, Instant::now() + Duration::from_secs(3600));
}

/// Inserts reserved assets (an id whose handles are all gone is skipped: nothing uses it any more).
fn insert(world: &mut World, meshes: Vec<(AssetId<Mesh>, Mesh)>, images: Vec<(AssetId<Image>, Image)>) {
    if !meshes.is_empty() {
        let mut a = world.resource_mut::<Assets<Mesh>>();
        for (id, m) in meshes { let _ = a.insert(id, m); }
    }
    if !images.is_empty() {
        let mut a = world.resource_mut::<Assets<Image>>();
        for (id, i) in images { let _ = a.insert(id, i); }
    }
}

/// Runs the jobs in order until `deadline` (a job is taken out of the queue while it runs: it may use the stream).
fn run_jobs(world: &mut World, deadline: Instant) {
    let t0 = Instant::now();
    while Instant::now() < deadline {
        let Some(mut job) = world.resource_mut::<LevelStream>().jobs.pop_front() else { break };
        if !job(world, deadline) {
            world.resource_mut::<LevelStream>().jobs.push_front(job);
            break;
        }
    }
    world.resource_mut::<LevelStream>().stats.job_time += t0.elapsed();
}

/// Work for the level cleanup thread ([`run_later`]).
type Cleanup = Box<dyn FnOnce() + Send>;

/// Runs `f` on the level cleanup thread (on this thread when that thread cannot run).
pub fn run_later(f: impl FnOnce() + Send + 'static) {
    static CLEANUP: OnceLock<Option<Sender<Cleanup>>> = OnceLock::new();
    let tx = CLEANUP.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<Cleanup>();
        std::thread::Builder::new().name("level cleanup".into()).spawn(move || for f in rx { f() }).ok().map(|_| tx)
    });
    let f: Cleanup = Box::new(f);
    match tx {
        Some(tx) => {
            if let Err(e) = tx.send(f) { (e.0)() }
        }
        None => f(),
    }
}

/// Frees `value` on the level cleanup thread: a level's data takes milliseconds to free.
pub fn drop_later<T: Send + 'static>(value: T) { run_later(move || drop(value)); }

/// Times one of the set-ups a level runs on its first frames (the lazy ones after the level start): a
/// `level setup: <name>/<step> <ms> ms` line per [`SetupTimer::lap`], and `level setup: <name> <ms> ms` for the whole at
/// the end of its scope.
pub struct SetupTimer {
    name: &'static str,
    start: Instant,
    lap: Instant,
}

impl SetupTimer {
    pub fn start(name: &'static str) -> Self {
        let now = Instant::now();
        SetupTimer { name, start: now, lap: now }
    }

    /// The time since the last lap (or the start), as step `step`.
    pub fn lap(&mut self, step: &str) {
        let now = Instant::now();
        println!("level setup: {}/{step} {:.1} ms", self.name, (now - self.lap).as_secs_f64() * 1e3);
        self.lap = now;
    }
}

impl Drop for SetupTimer {
    fn drop(&mut self) { println!("level setup: {} {:.1} ms", self.name, self.start.elapsed().as_secs_f64() * 1e3); }
}
