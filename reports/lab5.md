# Lab 5: Threads and Deadlock Detection

## What I learned

When a thread asks for a lock or semaphore token, I can check the request
before committing it to the real synchronization object. The check asks which
thread could finish with the resources now available, then adds back the
resources that thread would release. If the new request leaves no sequence in
which all threads can finish, only that request needs to be undone.

I would keep resource accounting active even when detection is off. If
detection starts while threads already hold resources, it needs those holdings
to make a correct decision about the next request.

## Scope and time

This lab adds process-local deadlock detection for mutexes and semaphores. A resource request is recorded before checking whether all participating threads can finish. Unsafe requests return `-0xDEAD` without changing the real primitive. Accounting continues while detection is disabled. `get_time` also validates its complete user buffer and copies its result across page boundaries.

Work began on 2026-10-22 at approximately 20:49, Asia/Shanghai, and resumed after an interruption. Official local validation finished on 2026-10-23 at approximately 07:37. Active work and learner study time were not measured.

## Design

`ResourceSet` keeps Available, Allocation, and outstanding Need separately for each process. Mutexes and semaphores have independent sets, as permitted by the exercise. A request temporarily increases Need. The safety check repeatedly finishes a thread whose requests fit Work, then adds its held resources to Work. Failure rolls back only the new request. Successful acquisition moves that request into Allocation and reduces Available; release reverses the resource accounting.

Need describes current unresolved calls rather than a declaration of maximum future claims. This is the detection algorithm specified by the lab, using the same Work/Finish reduction as a Banker safety check. Its cost is O(T²R) for T tracked threads and R resource types.

New resource columns extend existing rows. Recycled TIDs receive fresh rows. Exited owners' resources stay unavailable because the real primitives do not automatically release them. Unowned semaphore posts produce a token without subtracting from an empty allocation. Condition-variable waits record mutex release, then register a normal mutex request before reacquisition. A rejected reacquisition returns `-0xDEAD` without owning the mutex. Condition dependencies themselves are outside this detector.

The process's exclusive borrow is released before any scheduling or blocking operation. A suspended syscall also drops its local PCB reference before blocking. Fork starts with fresh synchronization objects and detection disabled, matching the framework's existing resource inheritance policy.

`get_time` rejects overflow, unmapped pages, supervisor-only pages, and pages without write permission before copying any bytes. Each translated fragment receives its corresponding bytes, so the two words in `TimeVal` can lie on different physical pages.

## Questions

### 1. What must be reclaimed when the main thread exits?

The whole process loses its user stacks, trap-context pages, application code/data pages, page-table frames, file-descriptor references, synchronization objects and wait queues, task identifiers, and eventually kernel stacks and PCB/PID storage. Children are reparented instead of destroyed. The zombie's exit status and minimal PCB state remain until its parent waits for it.

TCB references occur in the process task vector, scheduler ready queue, processor's current slot, timers, and synchronization wait queues. Ready-queue and timer entries must be removed explicitly because they outlive the process and could schedule a terminated thread. The task vector and process-local wait references must also be released. The baseline relies on synchronization-object destruction for local queues; an object retained by an Arc on a suspended kernel stack can delay this cleanup. This lab has not verified complete reclamation of that baseline ownership cycle.

The current thread's kernel stack cannot be freed while exit code or the context switch is still using it. This framework retains the exiting main task in `TaskManager::stop_task`; a later stopping task replaces that reference after execution has moved to another stack. User resources are extracted and dropped before recycling the rest of the address space. Their destructors borrow the PCB, so they must run after the existing exclusive PCB borrow has been released.

### 2. How do the two mutex implementations differ?

Mutex1 clears the lock before waking one waiter. Its `lock` loop rechecks the state after every wakeup. A newly arriving thread can acquire the unlocked mutex before the awakened thread runs, forcing that waiter to queue again. Mutual exclusion is preserved by the loop, but FIFO handoff is lost and repeated arrivals can starve an older waiter.

Mutex2 keeps the lock marked held when a waiter exists. Unlock transfers ownership directly to that waiter; its suspended `lock` call can return without another acquisition attempt. New arrivals remain blocked. The flag is cleared only when the queue is empty. This prevents barging and avoids repeated acquisition attempts, provided each wakeup corresponds to a real ownership transfer. Adding unrelated or spurious wakeups would require changing that protocol.

## Validation

- Passed: eight host tests from the actual detection module with Rust 1.98.1: self-lock, safe blocking, cycle rollback, multiple instances, disabled detection/resource growth, unowned posts, exited owners/TID reuse, and condition-waiter mutex reacquisition.
- Passed: `cargo check --offline` and `cargo build --release --offline` for `riscv64gc-unknown-none-elf` using `nightly-2024-05-02`.
- Passed: the unchanged official chapter-8 checker under QEMU 8.2.2, **25/25**, exit status 0, `passed: true`. It includes all three deadlock cases, thread/condition/semaphore tests, and required earlier-chapter baselines. All five required reports were found.
- Passed: `git diff --check`.
- Local results: [checker JSON](../tmp/rcore-result.json), [full log](../tmp/rcore-grade.log), and [run metadata](../tmp/last-run.json).

Pinned checker revision: `7d61ec55b58eed6ca7052917846c1b87af34563a`. Pinned test revision: `a0593662ad55d670ba8c27ce1763347cd0dd552f`.

Official run command:

```sh
python3 /home/developer/OpenCamp/.cache/run_rcore.py 8
```

The run used disposable snapshot `.cache/rcore-runs/ch8-20261023-073618-e4477c`; it did not modify the implementation working tree or publish results online.

Kernel checks used `/tmp/rcore-ch8-static-check/os`, whose `src` links to the real working tree. The staging directory uses the official checker's unchanged vendored dependencies and lockfile. Direct offline compilation initially lacked the Git dependency cache; staging resolved that environment issue without changing the working tree's dependency configuration.

Host validation command:

```sh
RUSTUP_TOOLCHAIN=1.98.1 rustc --edition=2021 --test /tmp/rcore-ch8-deadlock-tests.rs -o /tmp/rcore-ch8-deadlock-tests
/tmp/rcore-ch8-deadlock-tests

cd /tmp/rcore-ch8-static-check/os
RUSTUP_TOOLCHAIN=nightly-2024-05-02 cargo check --offline
RUSTUP_TOOLCHAIN=nightly-2024-05-02 cargo build --release --offline
```

The temporary harness contains only `extern crate alloc` and a path import of the kernel's detection module. It does not replace the kernel or any official test.

## References and disclosure

The resource check follows the [chapter 8 exercise](https://learningos.cn/rCore-Tutorial-Guide/chapter8/5exercise.html). The kernel framework and official tests were used for verification.

Codex implemented the lab, ran the recorded checks, and drafted this report. No other student's solution was used. [Course honor code](https://learningos.cn/rCore-Tutorial-Guide/honorcode.html).
