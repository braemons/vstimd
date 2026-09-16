// A stable least-significant-digit radix sort over 32-bit key/value pairs,
// in three kernels: histogram, scan, scatter. One pass handles RADIX_BITS of
// the key; the caller runs as many passes as the key is wide.
//
// Stability is the whole point. The tile rasteriser feeds this the splats in
// the global back-to-front order the CPU sorter already maintains, so sorting
// *stably* by tile id leaves each tile's run in depth order for free — the
// order key never has to enter the sort at all. An unstable sort would leave
// the order inside a tile arbitrary, and since splats blend, that is a stimulus
// whose pixels change from frame to frame for no reason.
//
// Within a block, stability comes from sorting by one bit at a time: a split on
// a 0/1 predicate keeps equal elements in their original relative order, so
// four splits leave the block stably ordered by the 4-bit digit. Across blocks
// it comes from the histogram being scanned digit-major, so every (digit,
// block) pair gets a global slot that respects block order.

const RADIX_BITS: u32 = 4u;
const RADIX: u32 = 16u;          // 1 << RADIX_BITS
const BLOCK: u32 = 256u;         // items per block, one per invocation

struct Push {
    n:          u32,   // live key/value pairs
    num_blocks: u32,   // ceil(n / BLOCK)
    shift:      u32,   // bit offset of the digit this pass sorts on
    _pad:       u32,
}
var<push_constant> pc: Push;

// Ping-pong: a pass reads `src` and writes `dst`. The host alternates two bind
// groups rather than the shader choosing, since WGSL cannot index a binding.
@group(0) @binding(0) var<storage, read>       src_keys: array<u32>;
@group(0) @binding(1) var<storage, read>       src_vals: array<u32>;
@group(0) @binding(2) var<storage, read_write> dst_keys: array<u32>;
@group(0) @binding(3) var<storage, read_write> dst_vals: array<u32>;
// RADIX × num_blocks, digit-major: counts after `histogram`, global offsets
// after `scan`.
@group(0) @binding(4) var<storage, read_write> hist: array<u32>;

var<workgroup> tmp:    array<u32, BLOCK>;
var<workgroup> keys:   array<u32, BLOCK>;
var<workgroup> vals:   array<u32, BLOCK>;
var<workgroup> counts: array<atomic<u32>, RADIX>;
var<workgroup> starts: array<u32, RADIX>;
var<workgroup> carry_out: u32;

fn digit_of(key: u32) -> u32 {
    return (key >> pc.shift) & (RADIX - 1u);
}

/// Inclusive scan of `v` across the workgroup, leaving the result in `tmp[lid]`
/// and returning it. Every barrier sits at uniform control flow.
fn scan_inclusive(lid: u32, v: u32) -> u32 {
    tmp[lid] = v;
    workgroupBarrier();
    for (var off = 1u; off < BLOCK; off = off << 1u) {
        var add = 0u;
        if (lid >= off) {
            add = tmp[lid - off];
        }
        workgroupBarrier();
        if (lid >= off) {
            tmp[lid] = tmp[lid] + add;
        }
        workgroupBarrier();
    }
    return tmp[lid];
}

// ── 1. Histogram ─────────────────────────────────────────────────────────────
// One block's digit counts, written digit-major so the scan that follows is a
// single linear pass.
@compute @workgroup_size(256)
fn histogram(
    @builtin(local_invocation_id) l: vec3<u32>,
    @builtin(workgroup_id) wg: vec3<u32>,
) {
    let lid = l.x;
    let block = wg.x;
    if (lid < RADIX) {
        atomicStore(&counts[lid], 0u);
    }
    workgroupBarrier();

    let i = block * BLOCK + lid;
    if (i < pc.n) {
        atomicAdd(&counts[digit_of(src_keys[i])], 1u);
    }
    workgroupBarrier();

    if (lid < RADIX) {
        hist[lid * pc.num_blocks + block] = atomicLoad(&counts[lid]);
    }
}

// ── 2. Scan ──────────────────────────────────────────────────────────────────
// Exclusive scan over the whole digit-major histogram, in one workgroup that
// walks it in BLOCK-sized chunks carrying a running total. Single-workgroup so
// the carry needs no second pass; the array is RADIX × num_blocks, which stays
// small (16 × 16k = 256k for a 4 M-pair sort).
@compute @workgroup_size(256)
fn scan(@builtin(local_invocation_id) l: vec3<u32>) {
    let lid = l.x;
    let total = RADIX * pc.num_blocks;
    if (lid == 0u) {
        carry_out = 0u;
    }
    workgroupBarrier();

    var base = 0u;
    loop {
        if (base >= total) {
            break;
        }
        let i = base + lid;
        var v = 0u;
        if (i < total) {
            v = hist[i];
        }
        let incl = scan_inclusive(lid, v);
        let chunk_total = tmp[BLOCK - 1u];
        if (i < total) {
            hist[i] = carry_out + incl - v;
        }
        workgroupBarrier();
        if (lid == 0u) {
            carry_out = carry_out + chunk_total;
        }
        workgroupBarrier();
        base = base + BLOCK;
    }
}

// ── 3. Scatter ───────────────────────────────────────────────────────────────
// Sort the block by its digit with RADIX_BITS single-bit splits — each split is
// stable, so the block ends stably ordered — then place each run at the global
// offset the scan assigned to its (digit, block).
@compute @workgroup_size(256)
fn scatter(
    @builtin(local_invocation_id) l: vec3<u32>,
    @builtin(workgroup_id) wg: vec3<u32>,
) {
    let lid = l.x;
    let block = wg.x;
    let i = block * BLOCK + lid;

    // Padding sorts to the end of the block on every digit, and since the
    // splits are stable it stays behind the live items sharing its digit — so
    // the live ranks below are unaffected.
    let live = i < pc.n;
    var k = 0xFFFFFFFFu;
    var v = 0u;
    if (live) {
        k = src_keys[i];
        v = src_vals[i];
    }
    keys[lid] = k;
    vals[lid] = v;
    workgroupBarrier();

    for (var b = 0u; b < RADIX_BITS; b = b + 1u) {
        let key = keys[lid];
        let val = vals[lid];
        let bit = (key >> (pc.shift + b)) & 1u;
        let zero = 1u - bit;
        let incl = scan_inclusive(lid, zero);
        let zeros_total = tmp[BLOCK - 1u];
        // Zeros keep their scanned position; ones follow, preserving order.
        var dest = zeros_total + lid - (incl - zero);
        if (bit == 0u) {
            dest = incl - zero;
        }
        workgroupBarrier();
        keys[dest] = key;
        vals[dest] = val;
        workgroupBarrier();
    }

    // Where each digit's run starts inside this now-sorted block. Counts cover
    // live items only, so a run's padding sits past its live length and is
    // never written out.
    if (lid < RADIX) {
        atomicStore(&counts[lid], 0u);
    }
    workgroupBarrier();
    if (live) {
        atomicAdd(&counts[digit_of(keys[lid])], 1u);
    }
    workgroupBarrier();
    if (lid == 0u) {
        var running = 0u;
        for (var d = 0u; d < RADIX; d = d + 1u) {
            starts[d] = running;
            running = running + atomicLoad(&counts[d]);
        }
    }
    workgroupBarrier();

    let d = digit_of(keys[lid]);
    let rank = lid - starts[d];
    if (rank < atomicLoad(&counts[d])) {
        let out = hist[d * pc.num_blocks + block] + rank;
        dst_keys[out] = keys[lid];
        dst_vals[out] = vals[lid];
    }
}
