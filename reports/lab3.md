# Lab 3: Process creation and stride scheduling

## Implementation

`spawn` validates a bounded user path, loads its ELF into a fresh address space,
allocates a PID and kernel stack, connects the parent/child records, and queues
the child. It does not copy the parent's image. The scheduler selects the ready
process with the smallest accumulated stride and charges a priority-dependent
pass. New processes start at stride zero with priority 16; priorities below two
are rejected. Virtual-memory syscalls are adapted to the process-owned memory
set, and `exec` resets heap boundaries for the new image.

## Design

- Parent records own child `Arc`s; children retain a weak parent reference.
  Existing exit, adoption, and wait paths therefore also work for spawned tasks.
- ELF construction initializes the child's own trap frame and user entry rather
  than returning through the parent's fork syscall.
- The ready queue retains insertion order for equal strides. A linear minimum
  search is sufficient for the course workload and avoids stale heap keys when
  a process changes priority.
- `BIG_STRIDE` is `2^64` and accumulation uses `u128`. Every legal positive
  `isize` priority has a nonzero pass. The wider accumulator postpones overflow
  beyond practical runs; this implementation does not claim arbitrary-duration
  wraparound handling.
- Memory unmapping splits region ownership, so a subsequent fork copies only
  surviving mappings. Timestamp copies handle noncontiguous physical pages.

## Questions: stride overflow

### 1. The 8-bit example

With ordinary unsigned ordering, process 2 wins again: its stride becomes
`(250 + 10) mod 256 = 4`, which compares below process 1's 255. The unbounded
logical values would be 260 and 255, so process 1 should run next. Raw numeric
ordering loses the epoch information at wraparound.

### 2. Why the spread is bounded

For a fixed runnable set starting at the same stride, a dispatch increments a
minimum by at most `BIG_STRIDE/2`, because every priority is at least two. The
other values were already within that distance of the old minimum. Neither
retaining the old maximum nor raising the selected minimum can enlarge the
spread beyond that bound. This is an induction over dispatches. New admissions
or blocked-task reactivation require a separate placement rule if that global
bound is to remain guaranteed.

### 3. A wrap-aware comparator for a max heap

If the true separation is below half the counter modulus, a wrapping subtraction
distinguishes the logical order. `BinaryHeap` returns its greatest element, so
reverse that order to pop the logically smallest stride. Under the question's
assumption that strides differ:

```rust
impl PartialOrd for Stride {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let self_is_ahead = self.0.wrapping_sub(other.0) < (1u64 << 63);
        Some(if self_is_ahead {
            Ordering::Less
        } else {
            Ordering::Greater
        })
    }
}
```

A complete heap key also needs a matching `Ord` and consistent equality/tie
handling. In natural ring order, the 8-bit differences `125 - 255 = 126` and
`129 - 255 = 130` place 125 after 255 and 129 before 255, respectively. The heap
ordering reverses those relations. Values separated by exactly half the modulus
are ambiguous, so the bound must be strictly below that half-modulus threshold.

## Validation

On 2026-10-23, isolated `cargo check --offline --release` and
`cargo build --offline --release` both passed with
`nightly-2024-05-02` for `riscv64gc-unknown-none-elf`. The temporary copy used
the checker's vendored dependencies and lockfile, leaving the original worktree
configuration and checker unchanged. A placeholder ELF enabled only type
checking; it was not booted or treated as a valid test program.

Checker revision: `7d61ec55b58eed6ca7052917846c1b87af34563a`.
Official test revision: `a0593662ad55d670ba8c27ce1763347cd0dd552f`.

The official chapter-5 checker passed **15/15 checks** under QEMU 8.2.2 with
the same toolchain. Run command:

```sh
python3 /home/developer/OpenCamp/.cache/run_rcore.py 5
```

The runner copied the worktree into a disposable checkout and used the pinned
official checker and tests. It recorded `returncode: 0` and `passed: true`.
`ch5_usertest` completed, including migrated memory/time tests, priority
validation with `isize::MAX`, 40 spawned children, and wait/waitpid exit codes.
The official suite also ran all six stride workers; the unchanged chapter-5
checker itself does not evaluate their numeric ratios, so they were checked
separately from the recorded output:

| Priority | Loop count | Count / priority |
| --- | ---: | ---: |
| 5 | 15,483,200 | 3,096,640 |
| 6 | 19,509,600 | 3,251,600 |
| 7 | 22,772,800 | 3,253,257.14 |
| 8 | 26,110,800 | 3,263,850 |
| 9 | 29,410,800 | 3,267,866.67 |
| 10 | 33,063,600 | 3,306,360 |

The maximum/minimum normalized count is approximately **1.06773**, below the
guide's 1.5 threshold. These are measurements from one local QEMU run, not a
claim about every workload or host. Evidence is in `tmp/rcore-result.json`,
`tmp/rcore-grade.log`, and `tmp/last-run.json`.

## Assistance, references, and honor-code disclosure

OpenAI Codex implemented and reviewed this local study work and drafted the
report at the user's request. The participant's independent authorship or
personal execution of tests is not claimed. No peer solution was used.

The [chapter-5 exercise specification](https://learningos.cn/rCore-Tutorial-Guide/chapter5/4exercise.html)
defines the process interfaces and scheduler; the [chapter-4 specification](https://learningos.cn/rCore-Tutorial-Guide/chapter4/7exercise.html)
defines the migrated memory syscalls. The supplied framework and official test
sources were consulted. Source-file comments identify these guide references;
checker rules and test assertions were preserved.

The [course honor code](https://learningos.cn/rCore-Tutorial-Guide/honorcode.html)
requires assistance and references to be disclosed, prohibits copying others'
solutions, and requires experiment code to remain private. The changes implement the lab interfaces. The checks above were run locally.
The guide does not specify how AI-generated work is assessed.
