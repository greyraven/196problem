# The 196 Problem

A Mac-friendly Rust engine for the **196 Lychrel / palindrome quest**: repeatedly reverse the digits of a number and add, looking for a palindrome.

Most starting numbers eventually become palindromes. **196** is the smallest candidate that never has, despite decades of searching. This project will not *prove* 196 is a Lychrel number — brute force can only disprove that, by finding a palindrome. It *can* push further, verify known milestones, and let more people run the search on ordinary Apple silicon.

Home of the broader quest and records: **[p196.org](https://www.p196.org/)**  
Verification table used here: **[p196.org verification](https://www.p196.org/html/verification.html)**

## Background

I have always been into numbers, as a child with some OCD of counting the cracks in the sidewalk or counting telephone poles when I was in the backseat of my parents car. When I heard about Palindromic numbers later in my life I became obsessed with understanding how a number could result in a palindrome 100s of steps out and how some could do it with 1 or 2 steps. When I learned about unresolved numbers (called **Lychrel**) I felt compelled to figure it out. I didn't have a supercomputer, this was 2013 and people had already gotten it up over a billion. Computers back then couldn't do as much as they can now. So in 2026, I went to the p196 site which is a great history of the effort and I noticed it hadn't really been maintained. 

Everyone had apparently given up. It is probably the best honestly. I don't really have hope that we'll resolve it. We have to accept that some numbers never resolve palindromicly (is that a word?) and that's the way it works. But I have a nice machine now with 28 cores and M4 architecture. I think I got it to over 20 million digits in the first few days. The record I think is 2.5 billion (Maybe someone has done more and not published). I did 20 million in a few days. Some of you guys have monster rigs and can do even more than I can. I wanted to share my code so someone else can maybe solve this or not solve it.

The bigger question is not why 196 can't be resolved. It's the determination if there is a pattern of numbers that don't resolve and what are the significance of those numbers as a whole? Is there a predictable pattern of numbers that never resolve. Can we predict future Lychrel numbers we haven't tried yet? Technically, every number in the 196 "problem" is a Lychrel number. Those numbers chain through the ether as unicorns. Never to know their mirror. Ok, I digress.

Thanks to all those folks to tried and failed before me. We are bound by our failure but in trying we are on the battlefield. Imagine if we actually solve it?!?! Just one more iteration...

---

## Reporting

If you have have checkpoints beyond 2 billion please consider adding it to the checkpoint directory here. See the format of the existing ones.

---

## Features

- **Three kernels:** `naive` (per-digit reference), `block` (Dolbeau-style 64-bit blocks), `parallel` (multi-threaded chunk split)
- **Resumable checkpoints** — snippets often; full number every 1M iterations by default
- **Validators** — digit-sum mod-9 integrity check + cross-check against p196.org’s published first-25-digit milestones
- **Self-test / micro-benchmark** mode to confirm kernels agree before a long run

---

## Install (macOS)

### 1. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

Confirm:

```bash
rustc --version
cargo --version
```

### 2. Get this repo and build

```bash
git clone <your-repo-url> lychrel196
cd lychrel196
cargo build --release
```

Binary: `target/release/lychrel196`

---

## Quick start

Fresh run from 196 (block kernel is a good default for most Mac runs):

```bash
./target/release/lychrel196 \
  --start 196 \
  --checkpoint-dir lychrel_progress \
  --kernel block
```

Leave it running overnight (survives closing the terminal):

```bash
nohup ./target/release/lychrel196 \
  --start 196 \
  --checkpoint-dir lychrel_progress \
  --kernel block \
  > run_stdout.txt 2>&1 &
```

Watch progress (safe while running):

```bash
cat lychrel_progress/progress.txt
tail lychrel_progress/run_log.txt
```

Stop with `Ctrl+C` (foreground) or `kill <pid>`.  
**Resume** with the same command — it reloads the last **full-number** save automatically.

> Tip: stop shortly after a `[FULL SAVE]` line (every 1,000,000 iterations by default) so you lose almost no work. Timed checkpoints every 5s only store first/last 20-digit snippets for monitoring.

---

## Options

| Flag | Default | Meaning |
|---|---|---|
| `--start` | `196` | Starting seed |
| `--checkpoint-dir` | `lychrel_progress` | Progress directory |
| `--checkpoint-interval` | `5` | Seconds between snippet/status updates |
| `--full-save-every` | `1000000` | Iterations between full-number saves (resume points) |
| `--max-iterations` | (none) | Optional cap for short tests |
| `--kernel` | `parallel` | `naive`, `block`, or `parallel` |
| `--threads` | all CPUs | Thread count for `--kernel parallel` |
| `--self-test` | | Compare kernels, then micro-benchmark |

### Which kernel should I use?

| Kernel | When |
|---|---|
| `naive` | Correctness reference / matching an old run bit-for-bit |
| `block` | **Best everyday choice on a Mac** once you’ve verified the early milestones |
| `parallel` | Large numbers (many millions of digits+), when you want all cores |

At a few hundred thousand digits, `parallel` can be *slower* than `block` (carry-select does roughly 2× work per chunk). It becomes useful once the number is large enough to split into big chunks across many cores.

Example multi-core launch:

```bash
./target/release/lychrel196 \
  --start 196 \
  --checkpoint-dir lychrel_progress \
  --kernel parallel \
  --threads 28
```

---

## How to test

### 1. Kernel self-test (recommended before a long run)

Compares `naive`, `block`, and `parallel` for N iterations, then micro-benchmarks:

```bash
./target/release/lychrel196 --self-test 20000 1000 --threads 28
```

You want:

```text
SELF-TEST OK: naive == block == parallel ...
```

### 2. Short capped run

```bash
./target/release/lychrel196 \
  --start 196 \
  --checkpoint-dir lychrel_progress_test \
  --kernel block \
  --max-iterations 100000 \
  --checkpoint-interval 1
```

### 3. Live milestone verification

On a real run from 196, watch the terminal for lines like:

```text
[VERIFIED] 1000000 digits @ iteration 2415836 matches p196.org exactly.
[VERIFIED] 10000000 digits @ iteration 24159531 matches p196.org exactly.
```

Those compare **iteration count + first 25 digits** against the embedded table from p196.org.

---

## Validators

### A. Digit-sum mod 9 (every timed checkpoint)

For any starting number \(N\):

- digit-sum\((N)\) ≡ \(N \pmod 9\)
- reverse\((N)\) has the same digits, so the same value mod 9
- therefore each reverse-and-add doubles the value mod 9:

\[
N_{k+1} \equiv 2 \cdot N_k \pmod 9
\]

If this ever fails, the addition step is wrong and the program **stops immediately**.

### B. p196.org verification table (`verification.csv`)

Embedded checkpoints of the form:

```text
digit_count,iteration,first_25_digits
```

Source: [Wade VanLandingham’s verification page](https://www.p196.org/html/verification.html) (cross-checked there across independent machines/programs). As the run passes each digit-count milestone, we compare our iteration and first 25 digits.

Known early rows include:

| Digits | Iteration | First 25 digits |
|---:|---:|---|
| 1,000,000 | 2,415,836 | `1321620866345900792403087` |
| 10,000,000 | 24,159,531 | `1570638871513424055539043` |

One inconsistent row in the original public table (187,000,000 digits) is omitted on purpose — see comments in `verification.csv`.

---

## What gets saved

**Every few seconds (snippets only)**

- `progress.txt` — iteration, digits, elapsed time, mod9, first/last 20 digits
- `run_log.txt` — append-only history of the same

**Every `--full-save-every` iterations (default 1M)**

- `current_number.txt` — **full** number (needed to resume)
- Also written on clean exit (max-iterations / palindrome / integrity failure)

Abrupt kill loses at most one full-save window of work.

---

## Benchmarks (Mac Studio, Apple M3 Ultra, 28 cores)

These are ballpark numbers from development on a **Mac Studio (M3 Ultra, 28-core, ~96 GB RAM)**. They are not lab-grade STREAM numbers — useful for “what should I expect?”

### Micro-benchmark (digits processed / second)

At roughly 100k–200k digits:

| Kernel | Approx. throughput | Notes |
|---|---:|---|
| `naive` | ~0.74 billion d/s | Per-digit carry loop |
| `block` | ~2.7 billion d/s | ~3.5–3.6× faster than naive |
| `parallel` (2 chunks @ ~200k digits) | ~1.2 billion d/s | Slower than block at this size |

`d/s` = “digits summed per second” in the sense used by the palindrome-quest community (one reverse-and-add on a D-digit number counts as D digits of work).

### Wall-clock ballpark (from 196)

| Milestone | Rough time on this machine |
|---|---|
| 1,000,000 digits (`naive`) | ~25–30 minutes |
| 1,000,000 digits (`block`) | several minutes |
| ~11,000,000 digits (`block`, long run) | on the order of half a day once the block kernel is engaged |

**Scaling note:** total work grows about like **digits²**. Going 10× further in digits is roughly **100×** more work. Multi-core helps most at very large sizes; for everyday Mac runs, prefer `block` until you’re deep into the tens of millions of digits.

---

## Credits & thanks

Standing on a long line of patient computers and patient people:

- **[John Walker](https://www.fourmilab.ch/documents/threeyears/threeyears.html)** — early systematic reverse-and-add search (“Three Years of Computing”)
- **[Wade VanLandingham / p196.org](https://www.p196.org/)** — the living archive of Lychrel records, seeds, software history, and the public verification tables this project embeds
- **[Romain Dolbeau](http://www.dolbeau.name/dolbeau/p196/p196.html)** — `p196_mpi` and the [block-level carry / distributed algorithms paper](http://www.dolbeau.name/dolbeau/p196/p196_mpi.pdf) that inspired the fast kernels here
- **Jason Doucette, Ian Peters, Benjamin Despres, Vaughn Suite, Matt Stenson, Eric Goldstein, Pierre Laurent**, and many others credited on p196.org for software and record pushes over the years
- Everyone who published checkpoints so later implementations can prove they are not quietly wrong

If you push a new milestone, consider reporting it upstream at [p196.org](https://www.p196.org/) so the community table stays useful.

---

## License / contributing

This is a research/hobby computational project. Improvements welcome — especially:

- Faster Apple-silicon kernels (NEON / wider blocks)
- Better multi-core scaling at mid sizes
- Importers for published full-number frontier seeds
- Clearer progress reporting / CSV exports for verification rows

Please keep validation strict: a faster wrong answer is worse than a slower right one.
