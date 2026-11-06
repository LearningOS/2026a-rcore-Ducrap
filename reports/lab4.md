# Lab 4: Hard Links and File Metadata

## What I learned

I separate a file's name, inode, and open handle when reasoning about its
lifetime. Two names can point to the same inode. Removing the last name drops
the link count to zero, but the inode and data must remain available through
an open descriptor. The final close is then the point at which the blocks can
be reclaimed.

I would test this with more than a successful unlink. Repeating allocations
until their total exceeds the filesystem's capacity shows whether it actually
reuses the blocks and inode slots.

## Implementation

Implemented hard links, unlink, and fstat without changing the easy-fs disk format. Root-directory scanning supplies link counts. Shared inode handles keep an unlinked file alive until its final open handle closes; reclamation then releases inode and data bitmap entries. Metadata and timestamps use checked page-wise user copies. The required mmap, munmap, and direct ELF spawn interfaces are also implemented for earlier-chapter tests.

`linkat` rejects missing sources, duplicate destinations, identical names, and invalid flat-directory names. `unlinkat` clears the directory entry and reuses the vacant slot on later creation. `fstat` returns device 0, a stable disk inode number, the file type, and the current link count. An open file whose final name was removed reports zero links and remains readable and writable.

The filesystem stores weak references to live inode objects. Every lookup of the same inode returns the same live object. This distinguishes open handles from directory links without changing disk inode size. Final reclamation also frees indirect pointer blocks. Anonymous unmapping splits region ownership so a subsequent fork does not recreate removed pages.

## Validation

The following local host test was actually run:

```sh
RUSTUP_TOOLCHAIN=1.98.1 CARGO_TARGET_DIR=/tmp/opencamp-ch6-fs-target \
  cargo test --manifest-path easy-fs/Cargo.toml --test hard_links
```

Result: **1 passed, 0 failed**. The integration test covers shared inode identity, link counts, duplicate and missing names, data access after final unlink, deferred inode-number reuse, 4,200 create/write/unlink cycles, eight rounds of double-indirect data allocation and reclamation, and link persistence when reopening the filesystem. The repeated allocations exceed the available inode and data pools cumulatively, which checks reclamation rather than only directory disappearance.

The kernel also passed `cargo check --release --offline` for `riscv64gc-unknown-none-elf`, using nightly-2024-05-02 and the fixed checker's vendored dependencies in a temporary build snapshot.

The complete official local run was then executed on 2026-10-23:

```sh
python3 /home/developer/OpenCamp/.cache/run_rcore.py 6
```

Result: **passed, 31/31 checker conditions, return code 0**. The run used QEMU 8.2.2, nightly-2024-05-02, Checker commit `7d61ec55b58eed6ca7052917846c1b87af34563a`, and Test commit `a0593662ad55d670ba8c27ce1763347cd0dd552f`. The official tests exercised earlier-chapter process and mapping behavior together with file I/O, fstat, hard links, and repeated unlink/create operations. All four required report files were found.

The launcher copied the working tree into a disposable checkout and ran the original grading script there. Recorded evidence is in `tmp/rcore-result.json`, `tmp/rcore-grade.log`, `tmp/runner.log`, and `tmp/last-run.json`; the disposable run directory is `/home/developer/OpenCamp/.cache/rcore-runs/ch6-20261023-073616-c2b81d`. These are local results, not a remote submission.

## Chapter 6 Question

### What does the root inode do, and what happens if it is damaged?

The root inode is inode 0 and anchors the filesystem namespace. Its data contains directory entries mapping names to disk inode numbers. In this flat easy-fs implementation, ordinary file lookup, executable loading, link creation, and unlink all begin there.

Damage to its directory data can remove names or associate them with the wrong inode. Damage to its size or block pointers can make lookup read invalid blocks or panic. File data and inode bitmap entries may still exist, but losing directory names makes those files unreachable through normal lookup. Recovery needs a valid backup or a filesystem repair tool; scanning allocated inodes can identify surviving objects but cannot reliably reconstruct lost names. This implementation does not provide crash recovery or journaling.

## Chapter 7 Questions

### Give a practical use of a pipe.

`cat input.txt | wc -l` connects `cat`'s standard output to `wc`'s standard input through a pipe. `cat` writes the file bytes and `wc -l` counts newline characters. The shell creates the pipe, redirects the two descriptors, and closes unused ends in both children and itself. Closing every write end after `cat` exits lets `wc` observe end-of-file and finish. Leaving an unused write end open can prevent that EOF.

### Design a more convenient mechanism for communication among many processes.

Give each process a kernel-managed mailbox and expose `send(destination, message)` and `receive()` operations. A mailbox identifier or capability names the destination, so communicating processes do not create a separate pipe for every pair. Each message carries a sender identifier and payload; messages from one sender retain FIFO order, while messages from different senders need no global order.

Use bounded queues, blocking or nonblocking send/receive, and scheduler wakeups to handle backpressure without busy-waiting. Access checks prevent unauthorized sends or reads. Process exit closes its mailbox and wakes blocked peers with an error. Large payloads can use explicitly shared memory while mailbox messages carry notifications or buffer handles. This needs one receive endpoint per process instead of a set of pair-specific channels.

## References and disclosure

References include the supplied easy-fs and kernel code, the [chapter 6 exercise](https://learningos.cn/rCore-Tutorial-Guide/chapter6/4exercise.html), the [chapter 7 questions](https://learningos.cn/rCore-Tutorial-Guide/chapter7/3exercise.html), and the official tests and checker.

Codex implemented the lab, ran the recorded checks, and drafted this report. No other student's solution was used. [Course honor code](https://learningos.cn/rCore-Tutorial-Guide/honorcode.html).
