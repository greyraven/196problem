# Checkpoint submission

Copy this file to:

```text
196-<digit_count>-iter<iteration>-<githubuser>.md
```

Then fill in the fields below and add a matching row to `../INDEX.csv`.

## Fingerprint

```yaml
seed: 196
digit_count: REPLACE
iteration: REPLACE
first_25_digits: REPLACE                 # exactly 25 digits
last_20_digits: REPLACE                  # recommended
digit_sum_mod9: REPLACE                  # recommended
contributor: REPLACE                     # GitHub username
date_utc: YYYY-MM-DD
machine: REPLACE                         # optional, e.g. Mac Studio M3 Ultra 28-core
kernel_or_program: REPLACE               # optional, e.g. lychrel196 --kernel block
notes: |
  Optional free-form notes.
```

## Full number (optional)

```yaml
full_number_included: false
full_number_url:                         # if hosted outside git
full_number_sha256:                      # recommended if you publish a file
```

If you attach a full number in-repo (please ask first for large files):

```text
../seeds/196-<digit_count>-iter<iteration>.txt
```
