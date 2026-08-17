// 196 Lychrel reverse-and-add engine.
//
// Digit array convention: index 0 = least significant digit (LSD).
// This matches the natural "append a new digit when a carry overflows"
// operation (push to the end of the Vec) and is the representation used
// in Romain Dolbeau's p196_mpi paper (2014), which this implementation
// follows for the core algorithm.
//
// Correctness check (self-derived, not just trusted from a third party):
// for ANY starting number, digit_sum(N) mod 9 always equals N mod 9, and
// reverse(N) has the exact same digits as N (just reordered), so its digit
// sum -- and therefore its value mod 9 -- is IDENTICAL to N's. That means
// at every single iteration:
//     new_value = N + reverse(N)  =>  new_value mod 9 = (2 * N) mod 9
// So digit-sum-mod-9 must exactly double (mod 9) every iteration, with no
// exceptions, for any starting number at all. This is checked at every
// checkpoint below -- if it ever fails, there is definitely a bug in the
// addition step.

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::Local;
use rayon::prelude::*;

const VERIFICATION_CSV: &str = include_str!("../verification.csv");

/// Local wall-clock stamp for event lines (e.g. 2026-08-17 15:42:01).
fn local_timestamp() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

struct Checkpoint {
    digit_count: u64,
    iteration: u64,
    first_25: String,
}

fn load_verification_table() -> Vec<Checkpoint> {
    let mut out = Vec::new();
    for line in VERIFICATION_CSV.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() != 3 {
            continue;
        }
        out.push(Checkpoint {
            digit_count: parts[0].parse().expect("bad digit_count in verification.csv"),
            iteration: parts[1].parse().expect("bad iteration in verification.csv"),
            first_25: parts[2].to_string(),
        });
    }
    out
}

/// Parse a decimal number string into an LSD-first digit array.
fn parse_digits(s: &str) -> Vec<u8> {
    s.bytes().rev().map(|b| b - b'0').collect()
}

/// Render the digit array as a normal (MSD-first) decimal string.
fn to_string(digits: &[u8]) -> String {
    digits.iter().rev().map(|d| (d + b'0') as char).collect()
}

/// First n digits in normal reading order (the most-significant end).
fn first_n(digits: &[u8], n: usize) -> String {
    let len = digits.len();
    let n = n.min(len);
    digits[len - n..].iter().rev().map(|d| (d + b'0') as char).collect()
}

/// Last n digits in normal reading order (the least-significant end).
fn last_n(digits: &[u8], n: usize) -> String {
    let n = n.min(digits.len());
    digits[..n].iter().rev().map(|d| (d + b'0') as char).collect()
}

fn is_palindrome(digits: &[u8]) -> bool {
    let len = digits.len();
    for i in 0..len / 2 {
        if digits[i] != digits[len - 1 - i] {
            return false;
        }
    }
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kernel {
    /// Per-digit carry loop (Dolbeau section 2). Correctness reference.
    Naive,
    /// 8-digit u64 blocks with offset-246 carry trick (Dolbeau section 3.2).
    Block,
    /// Multi-threaded chunk split + carry resolve (Dolbeau section 3.1 / 4).
    Parallel,
}

impl Kernel {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "naive" => Some(Self::Naive),
            "block" => Some(Self::Block),
            "parallel" => Some(Self::Parallel),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Naive => "naive",
            Self::Block => "block",
            Self::Parallel => "parallel",
        }
    }
}

/// Keep chunks large enough that thread/sync overhead does not dominate.
const PARALLEL_MIN_CHUNK: usize = 100_000;

/// Per-digit reverse-and-add. Matches Dolbeau's basic algorithm exactly.
fn reverse_and_add_naive_into(current: &[u8], next: &mut Vec<u8>) {
    let n = current.len();
    next.clear();
    next.resize(n, 0);
    let mut carry: u8 = 0;
    for i in 0..n {
        let mut intermediate = current[i] + current[n - 1 - i] + carry;
        if intermediate >= 10 {
            carry = 1;
            intermediate -= 10;
        } else {
            carry = 0;
        }
        next[i] = intermediate;
    }
    if carry == 1 {
        next.push(1);
    }
}

/// Dolbeau block kernel for absolute range `start..end`.
/// `next_out` is the destination subslice for that range (length `end-start`).
fn reverse_and_add_block_range(
    current: &[u8],
    next_out: &mut [u8],
    start: usize,
    mut carry: u64,
) -> u64 {
    let n = current.len();
    let end = start + next_out.len();
    debug_assert!(end <= n);
    let mut i = start;
    let mut local = 0usize;

    while i + 8 <= end {
        let forward = unsafe {
            std::ptr::read_unaligned(current.as_ptr().add(i) as *const u64)
        };
        let rev_load = unsafe {
            std::ptr::read_unaligned(current.as_ptr().add(n - 8 - i) as *const u64)
        };
        let backward = rev_load.swap_bytes();

        const OFFSET: u64 = 0x00F6_F6F6_F6F6_F6F6;
        let mut result = forward
            .wrapping_add(OFFSET)
            .wrapping_add(backward)
            .wrapping_add(carry);

        let signs = result & 0x0080_8080_8080_8080;
        result -= (signs >> 7) * 0xF6;

        let msb = result >> 56;
        carry = (msb >= 10) as u64;
        result -= carry * (10u64 << 56);

        unsafe {
            std::ptr::write_unaligned(next_out.as_mut_ptr().add(local) as *mut u64, result);
        }
        i += 8;
        local += 8;
    }

    while i < end {
        let mut intermediate = current[i] as u64 + current[n - 1 - i] as u64 + carry;
        if intermediate >= 10 {
            carry = 1;
            intermediate -= 10;
        } else {
            carry = 0;
        }
        next_out[local] = intermediate as u8;
        i += 1;
        local += 1;
    }
    carry
}

/// Serial Dolbeau block-level reverse-and-add.
fn reverse_and_add_block_into(current: &[u8], next: &mut Vec<u8>) {
    let n = current.len();
    next.clear();
    next.resize(n, 0);
    let carry = reverse_and_add_block_range(current, next, 0, 0);
    if carry == 1 {
        next.push(1);
    }
}

/// Parallel reverse-and-add: each thread runs the Dolbeau block kernel on a
/// large chunk for both cin=0 and cin=1, then a tiny serial chain picks the
/// correct cin per chunk (classic carry-select). `scratch` holds the cin=1
/// digits (reused across iterations).
fn reverse_and_add_parallel_into(
    current: &[u8],
    next: &mut Vec<u8>,
    scratch: &mut Vec<u8>,
    threads: usize,
) {
    let n = current.len();
    let num_chunks = std::cmp::min(threads, n / PARALLEL_MIN_CHUNK).max(1);
    if num_chunks <= 1 {
        reverse_and_add_block_into(current, next);
        return;
    }

    next.clear();
    next.resize(n, 0);
    scratch.clear();
    scratch.resize(n, 0);

    let chunk_size = (n + num_chunks - 1) / num_chunks;

    // Compute cin=0 into `next` and cin=1 into `scratch`, chunk by chunk.
    // `par_chunks_mut` gives each thread a disjoint mutable subslice.
    let couts_0: Vec<u64> = next
        .par_chunks_mut(chunk_size)
        .enumerate()
        .map(|(c, chunk)| {
            let start = c * chunk_size;
            reverse_and_add_block_range(current, chunk, start, 0)
        })
        .collect();
    let couts_1: Vec<u64> = scratch
        .par_chunks_mut(chunk_size)
        .enumerate()
        .map(|(c, chunk)| {
            let start = c * chunk_size;
            reverse_and_add_block_range(current, chunk, start, 1)
        })
        .collect();

    let mut cins = vec![0u64; num_chunks];
    let mut cin = 0u64;
    for c in 0..num_chunks {
        let start = c * chunk_size;
        if start >= n {
            break;
        }
        cins[c] = cin;
        cin = if cin == 0 { couts_0[c] } else { couts_1[c] };
    }
    let final_carry = cin;

    // Chunks that need cin=1: copy from scratch over the cin=0 result in next.
    let scratch_ref: &Vec<u8> = scratch;
    next.par_chunks_mut(chunk_size)
        .enumerate()
        .for_each(|(c, chunk)| {
            if c >= num_chunks || cins[c] == 0 {
                return;
            }
            let start = c * chunk_size;
            if start >= n {
                return;
            }
            chunk.copy_from_slice(&scratch_ref[start..start + chunk.len()]);
        });

    if final_carry == 1 {
        next.push(1);
    }
}

fn reverse_and_add_into(
    kernel: Kernel,
    threads: usize,
    current: &[u8],
    next: &mut Vec<u8>,
    scratch: &mut Vec<u8>,
) {
    match kernel {
        Kernel::Naive => reverse_and_add_naive_into(current, next),
        Kernel::Block => reverse_and_add_block_into(current, next),
        Kernel::Parallel => reverse_and_add_parallel_into(current, next, scratch, threads),
    }
}

fn init_thread_pool(threads: usize) {
    // Ignore error if the global pool was already built (e.g. repeated tests).
    let _ = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global();
}

/// Compare kernels for `iters` steps from `start`, then micro-benchmark.
fn run_self_test(start: &str, iters: u64, bench_iters: u64, threads: usize) {
    init_thread_pool(threads);

    let mut naive = parse_digits(start);
    let mut block = naive.clone();
    let mut parallel = naive.clone();
    let mut tmp_n = Vec::new();
    let mut tmp_b = Vec::new();
    let mut tmp_p = Vec::new();
    let mut scratch = Vec::new();

    for i in 1..=iters {
        reverse_and_add_naive_into(&naive, &mut tmp_n);
        reverse_and_add_block_into(&block, &mut tmp_b);
        reverse_and_add_parallel_into(&parallel, &mut tmp_p, &mut scratch, threads);
        if tmp_n != tmp_b || tmp_n != tmp_p {
            eprintln!("SELF-TEST FAILED at iteration {}", i);
            eprintln!("naive  first25={} ({} digits)", first_n(&tmp_n, 25), tmp_n.len());
            eprintln!("block  first25={} ({} digits)", first_n(&tmp_b, 25), tmp_b.len());
            eprintln!("parall first25={} ({} digits)", first_n(&tmp_p, 25), tmp_p.len());
            std::process::exit(1);
        }
        std::mem::swap(&mut naive, &mut tmp_n);
        std::mem::swap(&mut block, &mut tmp_b);
        std::mem::swap(&mut parallel, &mut tmp_p);
    }
    println!(
        "SELF-TEST OK: naive == block == parallel for {} iterations from {} (now {} digits, {} threads).",
        iters,
        start,
        naive.len(),
        threads
    );

    if bench_iters == 0 {
        return;
    }

    // Grow just enough for a few parallel chunks (keep this short).
    let mut base = naive;
    let target = PARALLEL_MIN_CHUNK.saturating_mul(2); // ~200k digits, 2 chunks
    if base.len() < target {
        println!(
            "Growing to ~{} digits for a quick parallel bench (block kernel)...",
            target
        );
        let mut nxt = Vec::with_capacity(target + 1);
        while base.len() < target {
            reverse_and_add_block_into(&base, &mut nxt);
            std::mem::swap(&mut base, &mut nxt);
        }
        println!("Grow done: {} digits.", base.len());
    }

    // Spot-check parallel multi-chunk vs block at this size.
    {
        let mut b = base.clone();
        let mut p = base.clone();
        let mut nb = Vec::new();
        let mut np = Vec::new();
        let mut sc = Vec::new();
        for i in 1..=16 {
            reverse_and_add_block_into(&b, &mut nb);
            reverse_and_add_parallel_into(&p, &mut np, &mut sc, threads);
            if nb != np {
                eprintln!("SELF-TEST FAILED (multi-chunk) at +{}", i);
                std::process::exit(1);
            }
            std::mem::swap(&mut b, &mut nb);
            std::mem::swap(&mut p, &mut np);
        }
        println!(
            "SELF-TEST OK: multi-chunk parallel == block ({} chunks).",
            std::cmp::min(threads, base.len() / PARALLEL_MIN_CHUNK).max(1)
        );
    }

    // Skip naive here -- too slow at this size; block is the 1-thread baseline.
    for (name, kernel) in [("block", Kernel::Block), ("parallel", Kernel::Parallel)] {
        let mut cur = base.clone();
        let mut nxt = Vec::with_capacity(cur.len() + 1);
        let mut scratch = Vec::new();
        let mut work = 0u64;
        let t0 = Instant::now();
        for _ in 0..bench_iters {
            work += cur.len() as u64;
            reverse_and_add_into(kernel, threads, &cur, &mut nxt, &mut scratch);
            std::mem::swap(&mut cur, &mut nxt);
        }
        let elapsed = t0.elapsed().as_secs_f64().max(1e-9);
        println!(
            "BENCH {} : {} iters in {:.3}s ({:.0} d/s, end {} digits, ~{} chunks)",
            name,
            bench_iters,
            elapsed,
            work as f64 / elapsed,
            cur.len(),
            std::cmp::min(threads, cur.len() / PARALLEL_MIN_CHUNK).max(1)
        );
    }
}

fn digit_sum_mod9(digits: &[u8]) -> u64 {
    let sum: u64 = digits.iter().map(|&d| d as u64).sum();
    sum % 9
}

/// 2^exp mod 9, exploiting the period-6 cycle (1,2,4,8,7,5,1,2,4,...).
fn pow2_mod9(exp: u64) -> u64 {
    const CYCLE: [u64; 6] = [1, 2, 4, 8, 7, 5];
    CYCLE[(exp % 6) as usize]
}

struct Progress {
    iteration: u64,
    digit_count: u64,
    elapsed_seconds: f64,
    start_number: String,
    c0_mod9: u64,
    /// Iteration at which current_number.txt was last written. Resume uses
    /// this (not `iteration`), since timed checkpoints only store snippets.
    full_save_iteration: u64,
    first20: String,
    last20: String,
    finished: bool,
    result: Option<String>,
}

fn write_progress_meta(dir: &Path, progress: &Progress) -> std::io::Result<()> {
    let meta_path = dir.join("progress.txt");
    let mut f = fs::File::create(&meta_path)?;
    writeln!(f, "start_number={}", progress.start_number)?;
    writeln!(f, "iteration={}", progress.iteration)?;
    writeln!(f, "digit_count={}", progress.digit_count)?;
    writeln!(f, "elapsed_seconds={:.3}", progress.elapsed_seconds)?;
    writeln!(f, "c0_mod9={}", progress.c0_mod9)?;
    writeln!(f, "full_save_iteration={}", progress.full_save_iteration)?;
    writeln!(f, "first20={}", progress.first20)?;
    writeln!(f, "last20={}", progress.last20)?;
    writeln!(f, "finished={}", progress.finished)?;
    if let Some(r) = &progress.result {
        writeln!(f, "result={}", r)?;
    }
    Ok(())
}

/// Timed / status checkpoint: snippets + metadata only (no full number).
fn save_snippet_checkpoint(dir: &Path, progress: &Progress) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    write_progress_meta(dir, progress)
}

/// Full resume checkpoint: write the complete number, then matching metadata.
/// Order matters -- number first, so a crash mid-write never leaves
/// progress.txt claiming a full_save_iteration whose file is incomplete.
fn save_full_checkpoint(dir: &Path, digits: &[u8], progress: &Progress) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    fs::write(dir.join("current_number.txt"), to_string(digits))?;
    write_progress_meta(dir, progress)
}

fn make_progress(
    digits: &[u8],
    iteration: u64,
    elapsed: f64,
    start_number: &str,
    c0_mod9: u64,
    full_save_iteration: u64,
    finished: bool,
    result: Option<String>,
) -> Progress {
    Progress {
        iteration,
        digit_count: digits.len() as u64,
        elapsed_seconds: elapsed,
        start_number: start_number.to_string(),
        c0_mod9,
        full_save_iteration,
        first20: first_n(digits, 20),
        last20: last_n(digits, 20),
        finished,
        result,
    }
}

fn load_checkpoint(dir: &Path, start_number: &str) -> Option<(Vec<u8>, u64, f64, u64)> {
    let meta_path = dir.join("progress.txt");
    let number_path = dir.join("current_number.txt");
    if !meta_path.exists() || !number_path.exists() {
        return None;
    }
    let meta = fs::read_to_string(&meta_path).ok()?;
    let mut full_save_iteration = None;
    let mut elapsed = None;
    let mut c0_mod9 = None;
    let mut saved_start = None;
    let mut finished = false;
    for line in meta.lines() {
        if let Some((k, v)) = line.split_once('=') {
            match k {
                // Prefer the dedicated full-save marker; fall back to
                // `iteration` for checkpoints written by older builds.
                "full_save_iteration" => full_save_iteration = v.parse::<u64>().ok(),
                "iteration" => {
                    if full_save_iteration.is_none() {
                        full_save_iteration = v.parse::<u64>().ok();
                    }
                }
                "elapsed_seconds" => elapsed = v.parse::<f64>().ok(),
                "c0_mod9" => c0_mod9 = v.parse::<u64>().ok(),
                "start_number" => saved_start = Some(v.to_string()),
                "finished" => finished = v == "true",
                _ => {}
            }
        }
    }
    if finished || saved_start.as_deref() != Some(start_number) {
        return None;
    }
    let number_str = fs::read_to_string(&number_path).ok()?;
    let digits = parse_digits(number_str.trim());
    Some((digits, full_save_iteration?, elapsed?, c0_mod9?))
}

fn append_log(dir: &Path, line: &str) -> std::io::Result<()> {
    let log_path = dir.join("run_log.txt");
    let mut f = fs::OpenOptions::new().create(true).append(true).open(log_path)?;
    writeln!(f, "{}", line)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut start_number = "196".to_string();
    let mut checkpoint_dir = PathBuf::from("lychrel_progress");
    let mut checkpoint_interval: f64 = 5.0;
    let mut full_save_every: u64 = 1_000_000;
    let mut max_iterations: Option<u64> = None;
    // Default to parallel on multi-core machines; falls back to serial block
    // while the number is still small. Use --kernel naive for the live
    // verification run's algorithm, or --kernel block for 1-thread Dolbeau.
    let mut kernel = Kernel::Parallel;
    let mut threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let mut self_test: Option<(u64, u64)> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--start" => { start_number = args[i + 1].clone(); i += 1; }
            "--checkpoint-dir" => { checkpoint_dir = PathBuf::from(&args[i + 1]); i += 1; }
            "--checkpoint-interval" => { checkpoint_interval = args[i + 1].parse().unwrap(); i += 1; }
            "--full-save-every" => { full_save_every = args[i + 1].parse().unwrap(); i += 1; }
            "--max-iterations" => { max_iterations = Some(args[i + 1].parse().unwrap()); i += 1; }
            "--threads" => { threads = args[i + 1].parse().unwrap(); i += 1; }
            "--kernel" => {
                let name = args[i + 1].as_str();
                kernel = Kernel::parse(name).unwrap_or_else(|| {
                    eprintln!("Unknown kernel '{}' (use naive, block, or parallel)", name);
                    std::process::exit(1);
                });
                i += 1;
            }
            "--self-test" => {
                // --self-test [compare_iters=50000] [bench_iters=20000]
                let compare = if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    i += 1;
                    args[i].parse().unwrap()
                } else {
                    50_000
                };
                let bench = if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    i += 1;
                    args[i].parse().unwrap()
                } else {
                    20_000
                };
                self_test = Some((compare, bench));
            }
            other => { eprintln!("Unknown argument: {}", other); std::process::exit(1); }
        }
        i += 1;
    }

    if threads == 0 {
        eprintln!("--threads must be >= 1");
        std::process::exit(1);
    }

    if let Some((compare, bench)) = self_test {
        run_self_test(&start_number, compare, bench, threads);
        return;
    }

    if kernel == Kernel::Parallel {
        init_thread_pool(threads);
    }

    if full_save_every == 0 {
        eprintln!("--full-save-every must be >= 1");
        std::process::exit(1);
    }

    fs::create_dir_all(&checkpoint_dir).expect("could not create checkpoint directory");

    let verification = load_verification_table();
    println!("Loaded {} verification checkpoints from p196.org data.", verification.len());

    let (mut digits, mut iteration, elapsed_prior, c0_mod9, mut full_save_iteration) =
        match load_checkpoint(&checkpoint_dir, &start_number) {
            Some((d, it, el, c0)) => {
                println!(
                    "Resuming from full-number checkpoint: iteration {}, {} digits, {:.1}s already elapsed",
                    it, d.len(), el
                );
                (d, it, el, c0, it)
            }
            None => {
                let d = parse_digits(&start_number);
                let c0 = digit_sum_mod9(&d);
                println!("Starting fresh from {}", start_number);
                // Tiny seed number -- save it so a crash before the first
                // million-iteration mark can still resume from iteration 0.
                let progress = make_progress(&d, 0, 0.0, &start_number, c0, 0, false, None);
                save_full_checkpoint(&checkpoint_dir, &d, &progress)
                    .expect("initial checkpoint save failed");
                (d, 0u64, 0.0, c0, 0u64)
            }
        };

    println!(
        "Kernel: {} (threads={}); snippet every {}s; full number every {} iterations.",
        kernel.name(), threads, checkpoint_interval, full_save_every
    );

    // Track which verification checkpoint comes next, so we never re-check
    // one and never miss one (digit count only ever grows by 0 or 1 per
    // iteration, so we won't skip past a target).
    let mut next_check_idx = verification
        .iter()
        .position(|c| c.digit_count > digits.len() as u64)
        .unwrap_or(verification.len());

    let run_start = Instant::now();
    let mut last_checkpoint = Instant::now();
    let mut other = Vec::with_capacity(digits.len() + 1);
    let mut scratch = Vec::new();

    loop {
        if iteration > 0 && is_palindrome(&digits) {
            let elapsed = elapsed_prior + run_start.elapsed().as_secs_f64();
            let result = format!("PALINDROME after {} iterations", iteration);
            println!("\n{}", result);
            let progress = make_progress(
                &digits, iteration, elapsed, &start_number, c0_mod9,
                iteration, true, Some(result),
            );
            save_full_checkpoint(&checkpoint_dir, &digits, &progress)
                .expect("checkpoint save failed");
            return;
        }

        if let Some(max) = max_iterations {
            if iteration >= max {
                let elapsed = elapsed_prior + run_start.elapsed().as_secs_f64();
                println!("\nReached max-iterations cap ({}). Full number saved.", max);
                let progress = make_progress(
                    &digits, iteration, elapsed, &start_number, c0_mod9,
                    iteration, false, None,
                );
                save_full_checkpoint(&checkpoint_dir, &digits, &progress)
                    .expect("checkpoint save failed");
                return;
            }
        }

        reverse_and_add_into(kernel, threads, &digits, &mut other, &mut scratch);
        std::mem::swap(&mut digits, &mut other);
        iteration += 1;

        // Check against the next unchecked verification checkpoint.
        if next_check_idx < verification.len() {
            let target = &verification[next_check_idx];
            if digits.len() as u64 == target.digit_count {
                let ours = first_n(&digits, 25);
                if target.iteration == iteration && ours == target.first_25 {
                    println!(
                        "\n[{}] [VERIFIED] {} digits @ iteration {} matches p196.org exactly.",
                        local_timestamp(), target.digit_count, iteration
                    );
                } else {
                    println!(
                        "\n[{}] [MISMATCH] at {} digits: iteration ours={} theirs={}, first25 ours={} theirs={}",
                        local_timestamp(), target.digit_count, iteration, target.iteration, ours, target.first_25
                    );
                }
                next_check_idx += 1;
            }
        }

        // Periodic full-number save for resume (default: every 1e6 iterations).
        if iteration - full_save_iteration >= full_save_every {
            let elapsed = elapsed_prior + run_start.elapsed().as_secs_f64();
            let progress = make_progress(
                &digits, iteration, elapsed, &start_number, c0_mod9,
                iteration, false, None,
            );
            save_full_checkpoint(&checkpoint_dir, &digits, &progress)
                .expect("full checkpoint save failed");
            full_save_iteration = iteration;
            eprintln!(
                "\n[{}] [FULL SAVE] iteration {} | {} digits | {:.0}s",
                local_timestamp(), iteration, digits.len(), elapsed
            );
        }

        if last_checkpoint.elapsed().as_secs_f64() >= checkpoint_interval {
            let elapsed = elapsed_prior + run_start.elapsed().as_secs_f64();
            let digit_count = digits.len() as u64;

            let actual_mod9 = digit_sum_mod9(&digits);
            let predicted_mod9 = (c0_mod9 * pow2_mod9(iteration)) % 9;
            let integrity_ok = actual_mod9 == predicted_mod9;

            let first20 = first_n(&digits, 20);
            let last20 = last_n(&digits, 20);
            let log_line = format!(
                "[{}] iter={} digits={} elapsed={:.1}s mod9={} first20={} last20={}",
                local_timestamp(), iteration, digit_count, elapsed, actual_mod9, first20, last20
            );
            append_log(&checkpoint_dir, &log_line).expect("log write failed");

            eprint!(
                "\r[{}] iter {} | {} digits | {:.0}s | mod9 {}{}   ",
                local_timestamp(), iteration, digit_count, elapsed, actual_mod9,
                if integrity_ok { "" } else { " <-- INTEGRITY CHECK FAILED" }
            );
            std::io::stderr().flush().ok();

            if !integrity_ok {
                eprintln!(
                    "\n\n[{}] INTEGRITY CHECK FAILED at iteration {}: expected digit-sum mod 9 = {}, got {}. \
                     This means there is a bug in the addition step -- stopping.",
                    local_timestamp(), iteration, predicted_mod9, actual_mod9
                );
                let progress = make_progress(
                    &digits, iteration, elapsed, &start_number, c0_mod9,
                    iteration, false, Some("INTEGRITY CHECK FAILED".to_string()),
                );
                save_full_checkpoint(&checkpoint_dir, &digits, &progress)
                    .expect("checkpoint save failed");
                std::process::exit(1);
            }

            // Snippets only -- do not rewrite the multi-megabyte number file.
            let progress = make_progress(
                &digits, iteration, elapsed, &start_number, c0_mod9,
                full_save_iteration, false, None,
            );
            save_snippet_checkpoint(&checkpoint_dir, &progress)
                .expect("snippet checkpoint save failed");
            last_checkpoint = Instant::now();
        }
    }
}
