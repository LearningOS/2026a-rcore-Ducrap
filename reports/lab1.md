# Lab 1: task tracing

## What I learned

I now read a trap as two related changes: the CPU switches privilege levels,
while the trap handler switches to a kernel stack and saves the application's
registers. That distinction explains why swapping the stack pointer alone
cannot return to user mode. The restored status and program counter tell
`sret` where and at what privilege to resume.

For syscall tracing, I look at when the counter is updated. Counting at
dispatch means that a trace query includes its own call. Keeping the counters
with the task means switching to another application leaves those counts intact.

## Implementation

`sys_trace` reads or writes one byte in the chapter's flat address space, or returns the current task's syscall count. The dispatcher records calls before invoking handlers, so querying trace includes the current trace call. Five counters per task cover the supported syscall IDs without putting a large sparse table on the boot stack. A query for an unsupported ID returns zero; an unsupported trace operation returns `-1`. Counts remain separate when the scheduler switches tasks.

## Trap handling

### User-mode restrictions

The supplied `ch2b_bad_*` programs exercise illegal addresses, privileged instructions, and supervisor CSR access from U mode. The actual QEMU log records a page fault at address zero for the bad-address application and two illegal-instruction traps for the other applications. The kernel terminates each offending application and continues scheduling. An illegal instruction or CSR access is not executed with supervisor authority merely because the kernel implements its trap handler.

The supplied firmware metadata identifies RustSBI Prototyper 0.4.1, revision `1e10645e9a3cc6230988b037df506f97310555b7`, in jump mode. The kernel is loaded at `0x80200000`; the firmware SHA-256 is recorded in `bootloader/rustsbi-qemu.toml`.

### Entry to `__restore`

The stack pointer addresses a kernel-resident `TrapContext`. Restoration is used both to start an application from its initialized context and to resume an application after its trap has been handled. The saved context supplies the user register values and the address at which execution resumes.

### Special registers

The restore code loads saved `sstatus`, `sepc`, and user `x2` into temporary registers. It writes `sstatus` to restore the intended return privilege and interrupt state, `sepc` to select the user instruction address, and `sscratch` to hold the user stack pointer until the final stack exchange.

`x2` is handled separately because it is the stack pointer used to access the trap frame itself. Loading it alongside ordinary registers would lose access to the remaining saved state. `x4` is the thread pointer; this chapter does not assign it a per-user-thread meaning, so the assembly deliberately omits it from the ordinary restore loop.

### Stack exchange and privilege changes

Immediately before returning, the restore code first removes the trap-frame allocation. Exchanging `sp` and `sscratch` then gives `sp` the user stack pointer and leaves the kernel stack top in `sscratch`. `sret` changes privilege according to the saved `sstatus.SPP`, sets the interrupt state from `SPIE`, and resumes at `sepc`. With the application's SPP cleared, execution continues in U mode.

On the next user trap, the initial stack exchange reverses those roles: `sp` becomes the kernel stack top and `sscratch` temporarily holds the interrupted user stack pointer. The following stack subtraction allocates the new `TrapContext`. An `ecall`, fault, illegal instruction, or enabled interrupt causes the hardware trap transition to S mode and transfers control through `stvec`; the exchange instruction itself does not change privilege.

## Validation

The original official checker completed successfully: **7/7**, exit status **0**, with `reports/lab1.md` found. The suite ran the trace and time tests together with earlier baseline applications, including the three bad applications described above.

Environment: QEMU 8.2.2, `nightly-2024-05-02`, cargo-binutils 0.4.0, and the supplied RustSBI 0.4.1 firmware. Checker revision: `7d61ec55b58eed6ca7052917846c1b87af34563a`; test revision: `a0593662ad55d670ba8c27ce1763347cd0dd552f`. The unmodified checker used its vendored offline dependencies in a disposable snapshot. Evidence is saved in `tmp/rcore-grade.log`, `tmp/rcore-result.json`, and `tmp/last-run.json`. No result was uploaded.

## References and disclosure

The supplied kernel, trap assembly, and RustSBI metadata were used alongside the [chapter 3 exercise](https://learningos.cn/rCore-Tutorial-Guide/chapter3/5exercise.html).

Codex implemented the lab, ran the recorded checks, and drafted this report. No other student's solution was used. [Course honor code](https://learningos.cn/rCore-Tutorial-Guide/honorcode.html).
