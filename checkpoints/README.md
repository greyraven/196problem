# Checkpoints

This folder is a community ledger for the **196** reverse-and-add search.

Use it to:

1. Look up known fingerprints (digit count + iteration + first 25 digits)
2. Publish new milestones so others can verify their engines
3. Optionally share a full-number seed so someone else can resume from your frontier

We are not asking anyone to upload multi-gigabyte numbers into git by default. A **fingerprint** is enough to prove you hit a milestone. A **full number** is only needed if others should resume from that exact state.

---

## Directory layout

```text
checkpoints/
  README.md                 ← you are here
  INDEX.csv                 ← master fingerprint list (all seeds below)
  known/
    p196-org-verification.csv   ← classic public table from p196.org
  submissions/
    TEMPLATE.md             ← copy this for a new submission
    196-….md                ← contributor submissions
```

---

## Two kinds of checkpoint

### 1. Fingerprint (always submit this)

Small, git-friendly, enough for verification:

| Field | Required | Example |
|---|---|---|
| `seed` | yes | `196` |
| `digit_count` | yes | `21938766` |
| `iteration` | yes | `53000000` |
| `first_25_digits` | yes | `1736344972994541681993376` |
| `last_20_digits` | recommended | `89187144400379334637` |
| `digit_sum_mod9` | recommended | `1` |
| `contributor` | yes | GitHub username |
| `date_utc` | yes | `2026-08-13` |
| `machine` | optional | `Mac Studio M3 Ultra, 28-core` |
| `kernel` | optional | `block` / `parallel` / other program |
| `notes` | optional | anything useful |

**Rules**

- `first_25_digits` must be exactly 25 decimal digits (or the whole number if shorter).
- Do not put spaces or commas inside the digit fields.
- Prefer submitting at a **full-number save** boundary (this engine: every 1,000,000 iterations by default), so `iteration` matches a reproducible file.
- If you only have a live status line, say so in `notes` — those can drift slightly from a full save.

### 2. Full number (optional, for resume seeds)

Only include if you want others to continue from your point.

- Prefer **not** committing huge files to git.
- Host elsewhere (Release asset, Hugging Face, personal download) and put the URL in your submission.
- If the file is small enough and you really want it in-repo, ask in a PR first.
- File contents: one continuous decimal integer, no spaces/newlines preferred (a single trailing newline is OK).
- Filename suggestion: `196-<digit_count>-iter<iteration>.txt`

---

## How to submit

1. Copy `submissions/TEMPLATE.md` to a new file named:

   ```text
   submissions/196-<digit_count>-iter<iteration>-<githubuser>.md
   ```

   Example: `submissions/196-21938766-iter53000000-greyraven.md`

2. Fill in every required field.

3. Add one row to `INDEX.csv` (keep the file sorted by `digit_count`, then `iteration`):

   ```csv
   digit_count,iteration,first_25_digits,contributor,source_file
   21938766,53000000,1736344972994541681993376,greyraven,submissions/196-21938766-iter53000000-greyraven.md
   ```

4. Open a pull request.

### How to get the fields from this engine

While running (or after stopping):

```bash
cat lychrel_progress/progress.txt
```

For a resumable fingerprint, use the **full save** (see `full_save_iteration`) and `current_number.txt`:

```bash
# digit count / first 25 / last 20 from the saved full number
python3 - <<'PY'
from pathlib import Path
n = Path('lychrel_progress/current_number.txt').read_text().strip()
print('digit_count', len(n))
print('first_25', n[:25])
print('last_20', n[-20:])
print('mod9', sum(map(int, n)) % 9)
PY
```

`iteration` for that file is `full_save_iteration` from `progress.txt`.

---

## Verification expectations

Reviewers (and you) should check:

1. **Length** — `digit_count` equals the length of the claimed number / matches first+…+last consistency.
2. **First 25** — matches an independent run, or continues a chain from a prior trusted fingerprint.
3. **mod 9** — for seed 196, `c0_mod9 = 7`, so after `iteration` steps:

   ```text
   expected = (7 * pow(2, iteration, 9)) % 9
   ```

   (with the usual `0` meaning divisible by 9 when the digit sum is non-zero multiples of 9 — compare against digit-sum mod 9 carefully; digit sum `0` mod 9 usually means multiple of 9).

4. **Monotonicity** — digit count should not shrink; iteration must increase with progress.

Fingerprint-only submissions are welcome. Full-number seeds are gold when someone is trying to leap past a known frontier.

---

## Known baseline

`known/p196-org-verification.csv` is the classic public table from
[p196.org/html/verification.html](https://www.p196.org/html/verification.html)
(format: `digit_count,iteration,first_25_digits`).

One historically inconsistent row (187,000,000 digits) is omitted — see comments in that file.

---

## Current local highlight

As of the first submission in this repo, a Mac Studio run reached a full save at:

| Digits | Iteration | Contributor |
|---:|---:|---|
| 21,938,766 | 53,000,000 | greyraven |

See `submissions/196-21938766-iter53000000-greyraven.md`.
