//! Back-to-front splat order, computed on a worker thread.
//!
//! Splats blend, so they must be drawn far to near. The order depends only on
//! the direction of the view axis in the cloud's own frame — never on where the
//! camera is: view-space depth is `dot(row₂(V·M), p) + const`, and the constant
//! does not reorder anything. A camera walking a straight corridor therefore
//! sorts once; only turning (or rotating the stimulus) asks for a new order.
//!
//! The render thread must not block or allocate, so it never sorts. It posts the
//! latest direction with [`SplatSorter::request`] and copies out the newest
//! finished order with [`SplatSorter::copy_latest`] — both `try_lock`, both
//! skipped rather than waited on when contended. A frame may therefore draw an
//! order a few frames old while the view turns; that is the accepted trade
//! (`dev/design/GAUSSIAN_SPLAT_PLAN.md` §4).

use std::sync::{Arc, Condvar, Mutex};

use glam::Vec3;

use super::GpuSplat;

/// Directions closer than this (in `1 − cos θ`, about 0.03°) reuse the order.
const RESORT_THRESHOLD: f32 = 1.5e-7;

struct Request {
    /// Normalised view axis in cloud space; `None` until the first request.
    axis: Option<Vec3>,
    shutdown: bool,
}

struct Latest {
    generation: u64,
    order: Vec<u32>,
}

struct Shared {
    request: Mutex<Request>,
    wake: Condvar,
    latest: Mutex<Latest>,
}

pub struct SplatSorter {
    shared: Arc<Shared>,
}

impl SplatSorter {
    /// Start a worker sorting `splats`. It idles until the first [`request`](Self::request).
    pub fn spawn(splats: Arc<[GpuSplat]>) -> Self {
        let shared = Arc::new(Shared {
            request: Mutex::new(Request {
                axis: None,
                shutdown: false,
            }),
            wake: Condvar::new(),
            latest: Mutex::new(Latest {
                generation: 0,
                order: Vec::new(),
            }),
        });
        let worker = Arc::clone(&shared);
        std::thread::Builder::new()
            .name("vstimd-splat-sort".into())
            .spawn(move || run(&worker, &splats))
            .expect("failed to spawn the splat sort thread");
        Self { shared }
    }

    /// Ask for the order seen along `view_axis` (cloud space; any length).
    /// Never blocks: a contended lock drops the request, and the next frame posts
    /// it again.
    pub fn request(&self, view_axis: Vec3) {
        let Some(axis) = view_axis.try_normalize() else {
            return;
        };
        if let Ok(mut r) = self.shared.request.try_lock()
            && r.axis != Some(axis)
        {
            r.axis = Some(axis);
            self.shared.wake.notify_one();
        }
    }

    /// If an order newer than `seen` is ready, copy it into `dst` and return its
    /// generation. Never blocks. `dst` must hold one index per splat.
    pub fn copy_latest(&self, seen: u64, dst: &mut [u32]) -> Option<u64> {
        let latest = self.shared.latest.try_lock().ok()?;
        if latest.generation == seen || latest.order.len() != dst.len() {
            return None;
        }
        dst.copy_from_slice(&latest.order);
        Some(latest.generation)
    }
}

impl Drop for SplatSorter {
    /// Stops the worker without joining it: it may be mid-sort, and the render
    /// thread must not wait for it. It exits at its next check.
    fn drop(&mut self) {
        if let Ok(mut r) = self.shared.request.lock() {
            r.shutdown = true;
        }
        self.shared.wake.notify_one();
    }
}

fn run(shared: &Shared, splats: &[GpuSplat]) {
    let n = splats.len();
    let mut sorted_axis: Option<Vec3> = None;
    let mut keys = vec![0u32; n];
    let mut order = (0..n as u32).collect::<Vec<_>>();
    let mut scratch = vec![0u32; n];
    let mut counts = vec![0u32; 1 << 16];
    loop {
        let axis = {
            let mut r = shared.request.lock().expect("splat sort lock poisoned");
            loop {
                if r.shutdown {
                    return;
                }
                match r.axis {
                    Some(a) if sorted_axis.is_none_or(|s| 1.0 - s.dot(a) > RESORT_THRESHOLD) => {
                        break a;
                    }
                    _ => r = shared.wake.wait(r).expect("splat sort lock poisoned"),
                }
            }
        };
        let t0 = std::time::Instant::now();
        sort_back_to_front(
            splats,
            axis,
            &mut keys,
            &mut order,
            &mut scratch,
            &mut counts,
        );
        sorted_axis = Some(axis);
        {
            let mut latest = shared.latest.lock().expect("splat sort lock poisoned");
            // Swap rather than copy: the worker keeps the previous buffer as its
            // next output, so steady-state sorting allocates nothing.
            std::mem::swap(&mut latest.order, &mut order);
            latest.generation += 1;
            // Only the first swap hands back the empty initial buffer.
            order.resize(n, 0);
        }
        log::trace!(
            "vstimd: sorted {n} splats in {} ms",
            t0.elapsed().as_millis()
        );
    }
}

/// Order `splats` far to near along `axis`, the view's +Z (towards the viewer)
/// in cloud space: the most negative `dot(axis, p)` is farthest. Two 16-bit
/// radix passes over a 32-bit quantised depth. All four buffers are the
/// caller's, so a sort allocates nothing.
fn sort_back_to_front(
    splats: &[GpuSplat],
    axis: Vec3,
    keys: &mut [u32],
    order: &mut [u32],
    scratch: &mut [u32],
    counts: &mut [u32],
) {
    let n = splats.len();
    if n == 0 {
        return;
    }
    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
    for s in splats {
        let z = axis.dot(s.position());
        lo = lo.min(z);
        hi = hi.max(z);
    }
    let range = (hi - lo).max(f32::MIN_POSITIVE);
    let scale = f64::from(u32::MAX) / f64::from(range);
    for (k, s) in keys.iter_mut().zip(splats) {
        *k = (f64::from(axis.dot(s.position()) - lo) * scale) as u32;
    }
    for (i, o) in order.iter_mut().enumerate() {
        *o = i as u32;
    }
    for shift in [0u32, 16] {
        counts.fill(0);
        for &i in order.iter() {
            counts[((keys[i as usize] >> shift) & 0xffff) as usize] += 1;
        }
        let mut sum = 0;
        for c in counts.iter_mut() {
            let here = *c;
            *c = sum;
            sum += here;
        }
        for &i in order.iter() {
            let bucket = &mut counts[((keys[i as usize] >> shift) & 0xffff) as usize];
            scratch[*bucket as usize] = i;
            *bucket += 1;
        }
        order.copy_from_slice(&scratch[..n]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Quat;

    fn at(z: f32) -> GpuSplat {
        GpuSplat::new(Vec3::new(0.0, 0.0, z), Vec3::ONE, Quat::IDENTITY, [1.0; 4])
    }

    fn sorted(splats: &[GpuSplat], axis: Vec3) -> Vec<u32> {
        let n = splats.len();
        let (mut keys, mut order, mut scratch) = (vec![0; n], vec![0; n], vec![0; n]);
        let mut counts = vec![0; 1 << 16];
        sort_back_to_front(
            splats,
            axis,
            &mut keys,
            &mut order,
            &mut scratch,
            &mut counts,
        );
        order
    }

    #[test]
    fn farthest_first_along_the_view_axis() {
        // The default camera looks down −Z, so its +Z axis is +Z: z = −50 is
        // farthest, z = 10 nearest.
        let splats = [at(0.0), at(-50.0), at(10.0), at(-5.0)];
        assert_eq!(sorted(&splats, Vec3::Z), vec![1, 3, 0, 2]);
        // Turned around, the order reverses.
        assert_eq!(sorted(&splats, -Vec3::Z), vec![2, 0, 3, 1]);
    }

    #[test]
    fn a_large_cloud_is_a_sorted_permutation() {
        let splats: Vec<_> = (0..100_000u32)
            .map(|i| at(((i.wrapping_mul(2_654_435_761)) % 10_007) as f32 * 0.37 - 1000.0))
            .collect();
        let order = sorted(&splats, Vec3::Z);
        let mut seen = vec![false; splats.len()];
        for &i in &order {
            assert!(
                !std::mem::replace(&mut seen[i as usize], true),
                "index {i} twice"
            );
        }
        for w in order.windows(2) {
            assert!(splats[w[0] as usize].position[2] <= splats[w[1] as usize].position[2]);
        }
    }

    #[test]
    fn the_worker_delivers_an_order() {
        let splats: Arc<[GpuSplat]> = vec![at(1.0), at(-1.0)].into();
        let sorter = SplatSorter::spawn(splats);
        sorter.request(Vec3::Z);
        let mut dst = [0u32; 2];
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let generation = loop {
            if let Some(g) = sorter.copy_latest(0, &mut dst) {
                break g;
            }
            assert!(std::time::Instant::now() < deadline, "no order within 5 s");
            std::thread::yield_now();
        };
        assert_eq!(generation, 1);
        assert_eq!(dst, [1, 0]);
    }
}
