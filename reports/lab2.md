# Lab 2: Address spaces

## What I learned

My rule for a syscall that writes to a user buffer is to check every page the
buffer spans before writing the first byte. A timestamp can start on a
writable page and end on an unmapped one. Checking only the starting address
would let an error leave a half-written timestamp behind.

I also separate the page-table entries from the kernel's record of who owns
the frames. A partial unmap has to update both. Otherwise a later fork could
copy region metadata that still describes a page the process already removed.

## Implementation

The kernel copies `TimeVal` through checked user mappings, including structures
that cross page boundaries. `trace` checks user visibility and read/write rights
and keeps per-task syscall counts. Anonymous mapping validates page alignment,
permissions, canonical addresses, overflow, and overlap before allocating frames.
Unmapping validates the whole range, releases its frames, and splits region
metadata around holes. Page-table lookup rejects invalid leaf entries. Failed
anonymous allocations roll back installed leaf mappings.

## Design

- Syscall counters are updated before dispatch, so a trace query includes itself.
- User copies first validate all destination pages, then copy page-sized pieces;
  a bad trailing page cannot cause a partial timestamp write.
- `prot` bits are shifted into PTE permission positions and combined with `U`.
  The RISC-V `R=0, W=1` encoding remains inaccessible, as exercised by `mmap2`.
- `MapArea` owns each allocated frame. Partial `munmap` preserves left/right
  regions instead of leaving holes inside the metadata used by later `fork`.
- The existing trampoline executes `sfence.vma` after installing the user page
  table, so new mappings are observed when execution returns to user mode.

## Questions

### 1. Sv39 page-table entries

An entry is 64 bits. Bits 53:10 hold the physical page number, bits 9:8 are
software-reserved, and bits 7:0 are `D A G U X W R V`. In the base configuration,
upper bits must be zero; extensions can assign `N` and `PBMT` meanings.

`V` validates an entry; `R/W/X` grant data-read, data-write, and instruction-fetch
access. `U` permits user access. `G` marks a global mapping. `A` records access,
and `D` records a store. Hardware updates or faults for `A/D` according to the
implemented scheme. `R=W=X=0` identifies a next-level table; a leaf has `R` or `X`
set. `W=1, R=0` is reserved and faults. Software may use `RSW` for its own metadata.
[RISC-V privileged specification, April 2024](https://docs.riscv.org/reference/isa/v20240411/_attachments/riscv-privileged.pdf)

### 2. Page faults

Instruction, load, and store/AMO page faults have exception codes 12, 13, and 15.
`scause` identifies the access, `sepc` identifies the faulting instruction, and
`stval` supplies the faulting virtual address when supported. For a user trap,
`sstatus.SPP=0`; `SPIE` saves the previous interrupt-enable state and `SIE` is
cleared. `stvec` selects the trap entry and `satp` identifies the active root.
[Supervisor ISA](https://docs.riscv.org/reference/isa/priv/supervisor.html)

Lazy loading avoids allocating or reading pages that execution never touches.
It reduces startup work and resident memory, at the cost of a first-access fault.
For anonymous memory it similarly avoids allocating unused reservations.

For an aligned 10 GiB interval with 4 KiB pages, there are 2,621,440 pages.
Their leaf entries occupy 20 MiB: 5,120 leaf-table pages. Approximately ten
middle-level pages and one root add 44 KiB. This is about 20 MiB of page tables,
separate from the 10 GiB of data frames. Alignment may add boundary tables.

A lazy implementation would record valid virtual regions and their permissions
without eagerly creating every leaf entry. On a fault, it would check the
region and access type, allocate a zeroed anonymous frame or load the backing
data, install a PTE, invalidate the relevant cached translation, and retry the
same instruction. Invalid accesses would terminate the process. Allocation
failure would follow an explicit failure policy rather than map arbitrary data.

For a swapped-out page, software clears `V` and records backing-store information
in separate metadata or an invalid-PTE encoding. A fault handler reads the page
back, restores the valid mapping, and retries. Hardware must never interpret a
swap slot as a resident physical page number.

### 3. Shared and separate kernel page tables

With one combined table per process, the OS installs the destination process's
`satp` during a process switch while executing code mapped in both tables. The
kernel portion uses `U=0`, so user mode cannot access it even though the same
table describes both portions. Permissions, rather than the virtual-address
number alone, enforce this boundary.

A combined table can avoid a table change on every syscall or interrupt and can
share kernel mapping structures. With separate tables, this kernel installs the
kernel table in `__alltraps` and the user table in `__restore`; the trampoline is
mapped at the same address in each. A combined-table design would normally
change roots only when switching to another process's address space, plus any
required explicit mapping changes. ASIDs and selective invalidation can reduce
TLB disruption, but require correct translation-cache management.

## Validation

On 2026-10-23, isolated `cargo check --offline --release` and
`cargo build --offline --release` both passed with
`nightly-2024-05-02` for `riscv64gc-unknown-none-elf`. Only a temporary source
copy used the checker's vendored dependencies and lockfile; the worktree's
build configuration and the original checker were preserved. A placeholder
ELF was used solely to let the build script run during type checking; it was
not a runnable test application.

Checker revision: `7d61ec55b58eed6ca7052917846c1b87af34563a`.
Official test revision: `a0593662ad55d670ba8c27ce1763347cd0dd552f`.

The official chapter-4 checker then passed **16/16 checks** under QEMU 8.2.2
with the same toolchain. Run command:

```sh
python3 /home/developer/OpenCamp/.cache/run_rcore.py 4
```

The runner copied the worktree into a disposable checkout, used the pinned
official checker and tests, and preserved the source worktree. It recorded
`returncode: 0` and `passed: true`. The log contains successful mmap/unmap,
trace permissions and trace-after-unmap checks, earlier trace/time/sleep tests,
and the expected sbrk and read-only-page faults. Those deliberately faulting
programs were terminated by the kernel as intended.

Evidence is in `tmp/rcore-result.json`, `tmp/rcore-grade.log`, and
`tmp/last-run.json`. This run does not independently cover every possible
out-of-memory or partial-unmap/fork interaction beyond the official suite.

## References and disclosure

The [chapter 4 exercise](https://learningos.cn/rCore-Tutorial-Guide/chapter4/7exercise.html), course framework, and official tests define the lab interfaces. Architecture references appear with the relevant answers.

Codex implemented the lab, ran the recorded checks, and drafted this report. No other student's solution was used. [Course honor code](https://learningos.cn/rCore-Tutorial-Guide/honorcode.html).
