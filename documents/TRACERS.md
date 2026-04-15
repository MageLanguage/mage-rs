# Debuggers and Tracers

Research on debugger and tracer implementations across virtual machines, languages, and operating systems.

---

## 1. Bytecode Patching

### 1.1. Luau — LOP_BREAK

Luau (Roblox's Lua fork) rejects the standard Lua `debug.sethook()` mechanism entirely. Hooks are not free — even when the hook function is a no-op, the interpreter must check and call it on every instruction, line, or function entry. This is a per-instruction tax that every program pays whether or not anyone is debugging. Luau calls this unacceptable and instead uses two techniques:

**Breakpoints:** The bytecode opcode at the target location is overwritten with `LOP_BREAK`. A parallel `debuginsn[]` array stores the original opcodes. When `LOP_BREAK` executes, the debugger is notified. When the breakpoint is cleared, the original opcode is restored from `debuginsn[]`.

**Single-stepping:** A separate interpreter loop handles step mode. This loop is structurally identical to the main loop but includes per-instruction callbacks. The interpreter switches between the fast loop and the debug loop as needed.

The result is zero overhead when no breakpoints are set. The bytecode stream is identical to the non-debug case. Cost is paid only at the exact instructions that have breakpoints, not globally. This is the cleanest approach we found for bytecode interpreters.

Source: https://luau.org/performance/ — "Epsilon-overhead debugger" section.

### 1.2. Erlang BeamAsm — Jump Target Patching

Erlang's BeamAsm JIT emits a small prologue at the start of every function:

```
0x0: short jmp 6                    ; skip to actual code (2 bytes)
0x2: nop                             ; padding (1 byte)
0x3: call breakpoint_fragment        ; shared breakpoint handler (5 bytes)
0x8: actual code...
```

Normal execution: the `short jmp 6` skips directly to the real code at offset 0x8. Cost: one taken branch, which the CPU predicts perfectly after the first execution.

To set a breakpoint: patch the jump offset from 6 to 1. The jump now lands on the `nop` at 0x2, falls through to the `call breakpoint_fragment` at 0x3. The shared fragment checks flags and calls into the runtime.

Patching a single byte (the jmp offset) is atomic on x86. No locks, no stop-the-world required. The breakpoint fragment is shared across all functions, so code size overhead is minimal. Furthermore, because BeamAsm uses W^X-compatible dual-mapped memory (one executable page, one writable page backed by the same physical memory), the writes go to the writable mapping and appear in the executable one without cache coherency issues.

This is impressively elegant. The prologue is only 8 bytes, the normal-case cost is one perfectly-predicted branch, and enabling a breakpoint is a single byte write.

Source: https://www.erlang.org/doc/apps/erts/beamasm — "Tracing and NIF Loading" section.

### 1.3. Linux ftrace / eBPF — NOP-to-CALL Patching

The Linux kernel compiles every function with `-fpatchable-function-entry`, inserting a 5-byte NOP (`0F 1F 44 00 00`) at the entry point. When tracing is enabled for a function, the NOP is atomically replaced with a 5-byte `call trampoline`. When tracing is disabled, it is patched back to NOP.

eBPF tracepoints use the same mechanism. At compile time, each tracepoint location is a 5-byte NOP. At runtime, enabling a tracepoint patches the NOP into a 5-byte jump to the trampoline. BPF trampolines (fentry/fexit) claim "practically zero overhead" compared to kprobes, which use the heavier INT 3 mechanism.

Truly zero cost when disabled — NOPs are eliminated by the CPU's front end. The patching is done via `text_poke()` which handles instruction cache coherency and cross-CPU synchronization. This is the gold standard for "zero when off, cheap when on" instrumentation. It has been battle-tested across billions of machines.

Source: https://docs.ebpf.io/linux/concepts/trampolines/

### 1.4. Wasmtime — NOP Padding with INT 3 Replacement

Wasmtime's Winch baseline compiler inserts a `nop` between the native code for each WebAssembly instruction. To set a breakpoint, the `nop` is replaced with `int3`. The SIGTRAP signal handler catches it, identifies the Wasm instruction, and notifies the debugger.

Same principle as ftrace, applied to JIT-compiled code. The NOP reservation at compile time makes runtime patching trivial. Still early in development but the design is sound.

Source: https://hackmd.io/@hvqFkDgPTuGNcu-NiycXZQ/SyXX166Yp

### 1.5. DTrace USDT — Is-Enabled Probes

DTrace's User Statically Defined Tracing (USDT) probes allow application developers to place trace points in their code that have near-zero overhead when not enabled. The mechanism: at each probe site, the compiler emits a `test-and-branch` over the probe payload, where the test reads an "is-enabled" flag. When no tracer is attached, the flag is 0 and the branch skips the probe arguments entirely.

More recent implementations go further: the `test-and-branch` is replaced by a NOP when no tracer is active, and patched to a jump to the trampoline when a probe is enabled — the same technique as ftrace. This means the disabled cost isn't even a predicted branch; it is a NOP eliminated by the CPU front-end.

The key design pattern: separate the "is anyone listening?" check from the "prepare and emit probe data" work. The expensive part (formatting arguments, copying data) only runs when the probe is active. This two-phase pattern — cheap guard, expensive payload — appears in almost every high-performance tracing system.

Source: https://blogs.oracle.com/linux/from-kernel-to-user-space-tracing

---

## 2. Polling and Signaling

### 2.1. HotSpot JVM — Safepoint Polling Page

HotSpot JIT-compiles Java methods into native x86 code. At certain "safepoint" locations (method returns, loop back-edges), the compiled code includes a load from a fixed "polling page" address:

```
test eax, [polling_page]
```

During normal execution the page is readable, the load succeeds silently (~1 cycle, always predicted to not fault), and execution continues. When the JVM needs to pause all threads (for GC, deoptimization, or debugger attachment), it calls `mprotect(polling_page, PROT_NONE)`. Every thread's next safepoint poll triggers a SIGSEGV. The JVM's signal handler recognizes the faulting address as the polling page and suspends the thread.

A memory load is essentially free on modern CPUs due to caching and speculative execution. The "should I stop?" check is not a branch at all — it is a load that either succeeds silently or faults. The hardware handles the rare case (fault) at zero cost to the common case (no fault). This is the fastest possible polling mechanism on x86 — faster even than a branch that is perfectly predicted, because there is no branch at all.

The downside: signal handling is expensive when it fires (~microseconds per thread). But since it only fires for rare events (GC, debugger attach), the amortized cost is negligible.

Source: https://shipilev.net/jvm/anatomy-quarks/22-safepoint-polls/

### 2.2. CPython 3.14 — PEP 768 Safe External Debugger Interface

CPython's traditional `sys.settrace()` has measurable overhead even when idle, because the interpreter's evaluation loop must check the trace function pointer on every instruction.

PEP 768 (accepted March 2025) introduces a zero-overhead alternative. A `debugger_pending_call` field is added to `PyThreadState`, and a check is inserted into the existing `eval_breaker` path — a branch the interpreter already takes for signals and periodic tasks. Since `debugger_pending_call` is never set during normal execution, the CPU's branch predictor eliminates it entirely.

An external debugger attaches by:
1. Locating `PyRuntime` in the target process via ELF/Mach-O section offsets.
2. Writing a script path into `debugger_script_path` in the target's memory.
3. Setting `debugger_pending_call = 1` and arming the `eval_breaker`.
4. The interpreter picks it up at the next safe point and executes the script.

Rather than injecting code at arbitrary points, the debugger signals the interpreter to execute code at the next safe opportunity. This works *with* the interpreter's natural execution flow rather than against it. The check piggybacks on an existing branch, adding zero new branches to the hot path.

PyPy independently implemented the same mechanism after seeing the PEP, confirming its generality.

Source: https://peps.python.org/pep-0768/

---

## 3. Native Breakpoints

### 3.1. GDB — INT 3 and Displaced Stepping

GDB sets breakpoints by overwriting the first byte of the target instruction with `0xCC` (INT 3). When the CPU executes it, a trap fires, transferring control to the debugger. The original byte is saved and restored when resuming.

To resume past a breakpoint without removing it (which would be racy in multithreaded programs), GDB uses "displaced stepping": it copies the original instruction to an out-of-line scratch area, executes it there, then adjusts the PC. This avoids the classic remove-step-reinsert race.

The simplest breakpoint mechanism is also the most universal. Every CPU architecture has a trap instruction. The out-of-line execution trick avoids races that simpler schemes cannot handle.

Source: https://eli.thegreenplace.net/2011/01/27/how-debuggers-work-part-2-breakpoints and https://devblogs.microsoft.com/oldnewthing/20241111-00/?p=110503

### 3.2. Chris Wellons — INT3;NOP as Fast Conditional Breakpoint

Chris Wellons (nullprogram.com) observed that GDB's conditional breakpoints are unusably slow because GDB stops the process, evaluates the condition in the debugger, and resumes — on every hit. For a breakpoint inside a tight loop with a rare condition, this means millions of stop/evaluate/resume cycles per second.

His alternative: compile the condition directly into the code:

```c
#define breakpoint() asm ("int3; nop")

if (rare_condition) breakpoint();
```

The `nop` after `int3` is essential because `int3` leaves the instruction pointer on the *next* instruction, confusing GDB about the current scope. The `nop` gives GDB something to "land on" within the correct scope.

This transforms a conditional breakpoint from a debugger-evaluated expression (thousands of context switches per second) into a single compiled branch (zero cost until the condition is true). The fastest debugger interaction is one that doesn't involve the debugger at all.

He also describes "named positions" using C labels or assembly labels as stable breakpoint targets that survive code edits — a clever alternative to line-number breakpoints that drift when the source changes.

Source: https://nullprogram.com/blog/2024/01/28/

### 3.3. x86 Hardware Debug Registers (DR0–DR3)

x86 processors provide four hardware debug registers (DR0–DR3) that can trigger a debug exception when a specific memory address is read, written, or executed. The addresses are configured via DR7 (the debug control register). When a watched address is accessed, the CPU raises a #DB exception.

The remarkable property: hardware watchpoints add **zero overhead** to instructions that don't touch the watched address. There is no polling, no flag check, no NOP — the watch is implemented in the memory access pipeline itself. The only cost is the trap handling when the watchpoint fires (~3μs on Linux via perf).

The limitation: only 4 watchpoints, each watching at most 8 bytes. For debugging a specific variable or memory location this is ideal. For broader tracing it is insufficient. GDB uses hardware watchpoints when available and falls back to single-stepping (vastly slower) when you exceed 4.

Linux exposes this via `perf_event_open` and `ptrace(PTRACE_POKEUSER)`. Jane Street's `perftrace` tool wraps this in a Python library that records timestamps and register values when watchpoints fire, enabling trace-style analysis of specific memory locations.

Source: https://thume.ca/2023/12/02/tracing-methods/ — "Hardware breakpoints" section.

---

## 4. Record and Replay

### 4.1. rr — Deterministic Record and Replay

Mozilla's `rr` records the execution of a Linux process with ~20% overhead by:
1. Executing only one thread at a time (eliminating data race nondeterminism).
2. Using CPU hardware performance counters (retired conditional branches) to measure application progress deterministically.
3. Recording only the sources of nondeterminism: system call results, signal delivery points, context switch points.

Replay re-executes the program using the same counter values to deliver signals and switch contexts at exactly the right points. This gives bit-for-bit identical replay.

Because replay is deterministic, GDB's reverse execution commands work: reverse-continue, reverse-step, reverse-next. `rr` implements these by restoring the nearest checkpoint and replaying forward to the desired point.

The critical insight: you don't need to record every instruction — only the nondeterministic inputs. The CPU deterministically re-derives everything else during replay. This makes recording nearly free for CPU-bound code. The ~20% overhead comes primarily from the single-threaded scheduling constraint.

The hardware performance counter approach is brittle — it depends on CPU-specific counter behavior and has been broken by various CPU microcode updates and errata. The rr team maintains a list of known-good CPU models. An alternative project, `rr.soft`, replaces hardware counters with lightweight dynamic instrumentation for environments where counters are unavailable (VMs, cloud, Apple Silicon via emulation).

Source: https://rr-project.org/ and https://queue.acm.org/detail.cfm?id=3688088

### 4.2. Pernosco — Omniscient Debugging via Post-Hoc Analysis

Robert O'Callahan (rr's creator) built Pernosco on top of rr recordings. Pernosco takes an rr trace, analyzes it using massive parallelism in the cloud, and builds a **searchable database of all program states at all points in time**. The result is an omniscient debugger that instantly answers:

- "What is the value at this memory address at time T?"
- "When was this value last modified?"
- "What is the dataflow path from this NULL to its origin?"

The killer feature is **reverse dataflow tracking**: click on a NULL pointer, and Pernosco traces it back through registers and memory to the exact instruction that produced it — without re-executing anything. This is by far the most powerful debugging capability we encountered in the research.

Pernosco also demonstrates "omniscient JS debugging" by observing V8's internal operation and inferring JavaScript-level state without modifying V8's source code. This suggests that high-level language debugging can be implemented as a layer on top of low-level omniscient tracing.

The trade-off: Pernosco's analysis takes minutes to hours and writes tens of gigabytes. It is a post-hoc tool, not a live tool. But users report it is overwhelmingly faster than traditional debugging for complex bugs.

Source: https://pernos.co/ and https://robert.ocallahan.org/2024/10/debt-workshop.html

### 4.3. Visual Studio Snapshot Debugger — Fork + Copy-on-Write

Visual Studio's Snapshot Debugger for Azure App Service uses `fork()` to debug production applications. When a "snappoint" (a non-breaking breakpoint) is hit, the application process forks. The forked child is immediately suspended, creating a snapshot. The developer debugs against the frozen fork while the parent continues serving requests.

Because `fork()` uses copy-on-write pages, the snapshot is nearly free in memory — only pages that the parent subsequently modifies are actually copied. The overhead to the production process is one `fork()` system call per snappoint hit.

This is a fundamentally different model from traditional debugging: snappoints don't stop execution. They create a point-in-time clone that can be inspected at leisure, possibly hours later. Multiple snapshots can be collected over time and compared. The trade-off: you cannot step forward from a snapshot, only inspect state.

Source: https://devblogs.microsoft.com/visualstudio/snapshot-debugging-with-visual-studio-2017-now-ready-for-production/

### 4.4. QEMU Record/Replay — Full-System Deterministic Execution

QEMU supports full-system record and replay by running in `icount` mode (instruction counting). All nondeterministic events — hardware interrupts, timer reads, network packets, disk I/O — are logged with their instruction count. During replay, events are injected at exactly the same instruction count, producing identical execution.

Unlike rr (which only records userspace), QEMU records the entire virtual machine including the kernel, drivers, and all processes. The cost is running inside QEMU's TCG (Tiny Code Generator) JIT instead of native execution, which is typically 5–10x slower. But for kernel-level debugging or debugging across the user/kernel boundary, it is the only option.

Source: https://www.qemu.org/docs/master/devel/replay.html

---

## 5. Omniscient / Time-Travel Debugging

### 5.1. Bil Lewis — ODB (Omniscient Debugger for Java)

Bil Lewis's ODB, the original omniscient debugger, records *every variable assignment* as a `(timestamp, variable, old_value, new_value)` tuple. After execution, a GUI allows the programmer to navigate backwards and forwards through the entire history.

The core idea: **you don't need breakpoints if you record everything.** The debugger becomes a database query tool — "show me the last time variable X changed" is a lookup, not a re-execution. "Which threads ran when?" is a fact, not a mystery.

Lewis reported that his programming style changed after using ODB exclusively: *"I now write insanely fast, making numerous mistakes. This gives me something to search for with the ODB. It's fun."* This observation — that omniscient debugging changes not just how you debug but how you write code — is striking.

The recording overhead is real but bounded. Lewis reported being able to debug Ant, JUnit, and the debugger itself. The approach does not scale to long-running production workloads, but for development and testing it is viable.

Source: https://omniscientdebugger.github.io/

### 5.2. Toby Ho — Structural Sharing for Time-Travel State

Toby Ho built a time-traveling debugger for his "Fun" language that records a snapshot of the entire program state at every step. To keep the recording compact, he uses **structural sharing** — if a stack frame didn't change between two steps, only a reference to the previous frame is stored, not a copy.

He invented a "JSON-R" format (JSON with References) where objects can be assigned IDs (`+1 {...}`) and later referenced (`*1`), allowing the history file to share identical sub-trees across snapshots.

The insight: program state changes incrementally. Most of the state is identical between consecutive steps. Persistent/immutable data structures or copy-on-write can reduce the recording overhead from O(state_size × steps) to O(delta × steps). For programs where most variables don't change on most steps, this is a dramatic reduction.

The limitation: the history file is written to disk and the debugger reads it back after execution. This is not a live debugger — it is a post-mortem replay tool. But the offline model has advantages: the debugger never needs to re-execute the program, and the history file can be shared with colleagues.

Source: https://www.tobyho.com/video/Time-Traveling-Debugger-Part-1.html

### 5.3. Elm — Immutable State Time Travel

Elm's architecture (The Elm Architecture, TEA) naturally supports time-travel debugging because every state update produces a new immutable state value. The debugger simply holds onto each state over time and provides a slider to scrub through them.

Because Elm's model values are immutable, there is no need for deep copying or structural sharing — the runtime already shares structure between consecutive states via persistent data structures. Time-travel is essentially free.

This only works for languages designed around immutable values from the start. Retrofitting it onto mutable-state languages requires explicit snapshotting, which is expensive. Elm demonstrates that language-level design decisions can make powerful debugging features trivial to implement.

### 5.4. Pharo/Smalltalk — The Debugger as Development Tool

Pharo's debugger is not a diagnostic tool bolted onto the side — it is a primary development tool. When an error occurs (including `doesNotUnderstand:`, the equivalent of "method not found"), the debugger opens with the full execution stack. The developer can then:

- **Create the missing method on the fly** from inside the debugger, type the implementation, and **proceed** — execution resumes as if the method had always existed.
- **Restart any frame** in the call stack, re-executing from that point with modified code or variables.
- **Edit any method** in the stack and proceed with the new version.

This is possible because Smalltalk's runtime is fully reflective — `thisContext` is a first-class object representing the current stack frame, methods are objects that can be recompiled at any time, and the VM supports frame restart natively.

The philosophical point: the debugger is the IDE. Rather than a cycle of edit → compile → run → crash → read error → edit, the Smalltalk workflow is: run → crash → the debugger opens → write the code right there → proceed. The boundary between "writing code" and "debugging code" dissolves entirely.

No other mainstream language has achieved this level of integration, except Common Lisp (see below).

Source: https://pharo.org/ and https://stackoverflow.com/questions/54496857/how-does-pharo-starts-debugger-when-message-is-not-understanded

### 5.5. Common Lisp — Condition/Restart System

Common Lisp's condition system separates three concerns that most languages conflate into "exception handling":

1. **Signaling:** Code detects an error and signals a condition (like throwing an exception), but the stack is *not* unwound.
2. **Handling:** A handler higher in the call stack decides what to do — but it runs *with the signaling frame still live*. It can inspect the full stack.
3. **Restarting:** The handler invokes a *restart* — a recovery strategy established by code between the handler and the signaler. The restart runs in the signaler's frame, not the handler's frame.

The critical difference from exceptions: the stack is not unwound before the handler runs. The handler can see the full context of the error, choose a recovery strategy, and resume execution as if the error never happened.

In the interactive debugger, when an unhandled condition is signaled, the debugger presents the user with a list of available restarts. The user can choose "use this value instead," "retry the operation," "skip this item," etc. — all without losing the execution context. This is the programmatic foundation for Pharo-style "fix it in the debugger" workflows, but expressed as a language-level mechanism rather than a VM feature.

The power is that libraries can establish restarts for anticipated failures without knowing how callers will handle them, and callers can handle them without knowing the library's internals. The debugger is just one possible handler.

Source: https://lisp-docs.github.io/docs/tutorial/conditions

### 5.6. Racket — Continuation Marks

Racket extends Scheme with "continuation marks" — key-value annotations that can be attached to any continuation frame (roughly, any stack frame). They are a language-level mechanism for stack inspection that doesn't require special VM privileges.

Any code can attach a mark to the current frame with `with-continuation-mark`, and any code can read all marks on the current continuation with `current-continuation-marks`. Marks on tail-called frames replace the previous mark rather than accumulating, preserving tail-call space guarantees.

Debuggers and profilers use continuation marks to implement stack inspection, step tracing, and source location tracking entirely within the language. The DrRacket IDE's debugger is implemented using continuation marks — it annotates source expressions with marks, then reads them to determine the current source location and available bindings.

The insight: if the language provides a first-class mechanism for annotating the stack, then debuggers don't need special VM hooks. They can be written as regular library code. Continuation marks also support other use cases: dynamic scoping, cost semantics tracking, and contract blame.

The overhead is one mark allocation per annotated frame. When no marks are read, the only cost is the allocation (which is amortized by the GC). When marks are read, it is a stack walk — but only the marks with the requested key are returned, not the entire stack.

Source: https://www2.ccs.neu.edu/racket/pubs/dissertation-clements.pdf and https://srfi.schemers.org/srfi-157/srfi-157.html

---

## 6. Live Visualization

### 6.1. Andrew Reece — WhiteBox

WhiteBox compiles, runs, and "debugs" C/C++ code live, displaying execution patterns and data transforms inline alongside the source code. Every expression shows what values it took, every branch shows how many times it was taken. A timeline slider allows scrubbing through the execution.

WhiteBox is not a debugger in the traditional sense — it is a **visualizer**. There is no "pause" or "step." Instead, the program runs to completion and the results are displayed as an overlay on the source code. It supports a "black box only" recording option that is faster but less detailed.

The philosophical point: separating "running" and "debugging" is an artificial distinction. If recording is cheap enough, every execution can be a debugging session. The UI should show behavior *alongside* the code, not in a separate pane.

Source: https://whitebox.systems/

### 6.2. Bret Victor — Timeline Scrubber

Bret Victor's "Inventing on Principle" talk (2012) proposed that execution isn't something you "step through" — it is a **timeline you scrub with a slider**. Every line of code has its execution history visualized alongside it. You drag a slider and see how values change over time.

This requires recording all state, but for a simple interpreter it is tractable. The visualization shows not just "what is the current value" but "how did this value evolve over the lifetime of the program."

The deeper claim: the step-by-step debugging model is a legacy of hardware limitations. With sufficient recording, the entire execution history is available simultaneously. The UI should present time as a spatial dimension, not a sequential process. No one has fully delivered on this vision in a production tool, but it remains the aspirational end state for debugging UIs.

Source: https://vimeo.com/36579366

---

## 7. Miscellaneous

### 7.1. Ruby YARV — Trace Instructions

YARV bytecode includes `trace` instructions at compile time at line boundaries, method entries, and returns. The TracePoint API enables or disables them at runtime. When disabled, trace instructions behave as NOPs. When enabled, they call registered callbacks.

Compile-time insertion of trace points with a runtime toggle. The cost when disabled is one NOP per trace point — negligible but not zero. At last count, YARV had 202 instructions, but that number shrinks significantly if you factor out tracepoint and specialized instructions, suggesting the trace infrastructure is a substantial portion of the instruction set.

### 7.2. Apache Harmony / HotSpot — Bytecode-to-Native Source Map

JVMs maintain a bidirectional mapping between bytecode offsets and native code addresses. For every bytecode instruction, the JIT records which native instruction(s) implement it. This allows setting a breakpoint at a bytecode offset by inserting `int 3` at the corresponding native address, and mapping a native fault address back to a source line via bytecode offset → line number table.

Source maps are the critical bridge between the user's mental model (source lines) and the machine's reality (instruction pointers or bytecode offsets). Without bidirectional mapping, debugging is impossible. Every debuggable system has this mapping in some form.

### 7.3. RemedyBG — Debugger as Service with Protocol

RemedyBG is a Windows-only native debugger built by a solo developer, optimized for speed and simplicity. Its interesting contribution is the debug protocol: an external process communicates with RemedyBG via shared memory and events, allowing editors and tools to control the debugger programmatically.

This is philosophically aligned with DAP (Debug Adapter Protocol) but more lightweight and lower-latency. The shared-memory approach avoids the JSON serialization overhead of DAP for high-frequency operations like variable inspection.

Source: https://remedybg.handmade.network/blog/p/3631-remedybgs_debug_protocol

### 7.4. Jamie Brandon — The Interactive Language Sweet Spot

Jamie Brandon's research on implementing interactive languages highlights the tension between compile-time and run-time performance. His observation: *"I also have no idea how painful it is to provide debugger support for a custom compiler (either via emitting DWARF or by writing a custom debugger)."*

He notes that the OCaml native-code compiler is about half the size of its interpreter, and yet performs one to two orders of magnitude better. This suggests that the "naive compiler" sweet spot — fast compilation, reasonable execution, good tooling — is underexplored compared to both interpreters and optimizing compilers.

His key architectural observation: tiered execution (interpreted glue code + compiled hot paths) may be the practical answer for interactive languages. The debugging story for tiered systems remains unsolved.

Source: https://www.scattered-thoughts.net/writing/implementing-interactive-languages/

### 7.5. py-spy / rbspy — External Process Memory Reading

py-spy (Python) and rbspy (Ruby) are sampling profilers that read the target process's memory from an external process using `process_vm_readv` (Linux), `vm_read` (macOS), or `ReadProcessMemory` (Windows). They do not inject any code, attach any debugger, or interrupt the target in any way.

Because interpreters like CPython and CRuby store their stack frames and thread state in process memory at known offsets, an external process can read those structures, decode the call stack, and produce a profile — all without the target knowing it is being observed. The overhead to the target is literally zero: no system calls, no signals, no ptrace attachment.

The trick only works for interpreters with predictable memory layouts. But the principle generalizes: if a VM stores its state in a known memory layout, an external observer can read it at any time without disturbing execution. Tristan Hume notes this could be extended to native programs: push context info onto a known data structure, and have an external process sample it via `process_vm_readv` or eBPF.

Source: https://github.com/benfred/py-spy and https://thume.ca/2023/12/02/tracing-methods/

### 7.6. Tracy / Spall — Nanosecond Instrumentation Profilers

Tracy and Spall are instrumentation-based profilers popular in game development, where per-frame performance visibility is essential.

**Tracy** achieves ~2ns overhead per span using a lock-free queue. The client collects events and holds them in memory until a server connects and pulls the data. The server reconstructs the timeline in real-time. Tracy supports CPU sampling, GPU tracing, memory allocation tracking, lock contention visualization, and context switch recording — all in a single tool with a custom native UI that handles hundreds of millions of events.

**Spall** takes a different approach: ~12ns per span, but the output is a compact binary file viewed in a web-based UI. Spall's simplicity is the point — a single-header C library for tracing, a web frontend for viewing. It supports `clang -finstrument-functions` for automatic whole-program tracing without manual annotation.

Both demonstrate that with careful engineering, instrumentation overhead can be low enough to leave enabled in development builds permanently. The 2–12ns per span cost is negligible compared to the microseconds or milliseconds that actual work takes, but the visibility gained is transformative.

Source: https://github.com/wolfpld/tracy and https://gravitymoth.com/spall/spall-web.html

### 7.7. E9Patch — Instruction Punning for Binary Rewriting

E9Patch is a static binary rewriter for x86-64 that inserts trampolines into compiled binaries without needing to move any existing instructions. This is hard on x86 because instructions are variable-length: a 5-byte jump overwrites multiple instructions, and some of those might be jump targets.

E9Patch solves this with three novel techniques:

- **Instruction punning:** Find addresses in the binary whose raw byte values also happen to be valid x86 NOPs or traps. Jump to those addresses as trampoline targets. The bytes were already there; they just weren't being used as code.
- **Padding:** Use `int3` padding between functions (common in compiled binaries) as trampoline space.
- **Eviction:** When no punning or padding target is available, evict a short instruction by copying it to the trampoline and replacing it with a jump.

The result: any instruction in a binary can be instrumented with zero relocation of surrounding code. This enables tools like E9AFL (fuzzing), E9Tool (tracing), and custom binary analysis passes.

The general principle: "patching" doesn't require dedicated NOP slots if you're creative about using existing bytes — repurposing existing data as executable code is a form of steganographic instrumentation.

Source: https://pldi20.sigplan.org/details/pldi-2020-papers/12/Binary-Rewriting-without-Control-Flow-Recovery

### 7.8. Frida Stalker — Scriptable Dynamic Binary Instrumentation

Frida is a dynamic binary instrumentation toolkit that lets you hook into and rewrite running processes using JavaScript. Its "Stalker" engine does full dynamic recompilation: as each basic block is about to execute, Stalker copies it to a scratch buffer, inserts your instrumentation, and runs the instrumented copy. This is the same technique QEMU and Rosetta use for emulation, but exposed as a scriptable API.

You can:
- Trace every instruction, call, or return.
- Rewrite assembly as it runs (e.g., change branch targets, NOP out instructions).
- Attach JS callbacks to specific addresses.
- Follow execution across threads and even across processes.

The overhead is substantial (5–50x depending on the workload and density of instrumentation), but the power is unmatched: any binary, any architecture, no source code, no recompilation, scriptable in JavaScript.

The most creative use: fuzzing. fpicker attaches Frida Stalker to a target binary, instruments every basic block to update a coverage bitmap, and uses the coverage to guide a fuzzer — all without source code.

Source: https://frida.re/docs/stalker/

### 7.9. Erlang dbg — Match Specification Tracing in Production

Erlang's `dbg` module provides function call tracing with match specifications — pattern-matching expressions that filter which calls generate trace events. You can trace all calls to a function, or only calls where the first argument matches a specific pattern, or only calls that return a specific value.

```erlang
%% Trace calls to math:sin/1 only when argument equals 3.14
dbg:tpl(math, sin, 1, dbg:fun2ms(fun([X]) when X == 3.14 -> return_trace() end))
```

The match specification is compiled into an efficient matcher by the VM. When the pattern doesn't match, the overhead is minimal — a quick pattern check in the VM's call dispatch path. When it does match, a trace message is generated and sent to a trace handler process.

This is routinely used in production Erlang systems to diagnose issues without restarting or redeploying. The key design: tracing is an opt-in, per-function, pattern-filtered mechanism built into the VM from the start. It is not an afterthought bolted on via external tools.

The `recon` library by Fred Hebert wraps `dbg` in safety rails (rate limiting, automatic timeout) to make production tracing even safer.

Source: https://www.erlang.org/doc/apps/runtime_tools/dbg.html and https://ferd.github.io/recon/recon_trace.html

### 7.10. Cannoli — Multi-Core QEMU Trace Processing

Cannoli patches QEMU's TCG (Tiny Code Generator) JIT to log execution and memory events to a high-performance ring buffer. A Rust extension compiled as a shared library reads the ring buffer on separate cores, spreading the trace processing load across the machine.

Unlike single-threaded tracing tools, Cannoli can keep up with fast targets because the trace consumer runs in parallel with the traced program. The ring buffer acts as a decoupling layer: the traced program writes events without blocking, and the Rust consumer processes them at its own pace.

The trade-off: Cannoli is read-only — it observes but cannot modify execution. This simplifies the design enormously compared to full DBI frameworks like Frida. For tracing and analysis workloads (coverage, taint tracking, protocol reverse engineering), read-only is sufficient.

Source: https://thume.ca/2023/12/02/tracing-methods/ — "Cannoli" section.

### 7.11. Implicit In-Order Forests — Billion-Event Trace Visualization

Tristan Hume developed a data structure called "implicit in-order forests" for rendering billion-event trace timelines at 60fps. The problem: a standard trace viewer must draw millions of rectangles when zoomed out, which overwhelms both the CPU and GPU.

The solution: pre-aggregate trace events into a tree structure where each level represents a different zoom level. When zoomed out, only the aggregated nodes at the appropriate level are drawn. When zoomed in, individual events are drawn. The tree is implicit (stored in a flat array with computed indices, no pointers) and in-order (events are arranged depth-first, matching the temporal order), giving excellent cache locality.

Combined with a virtual-memory-based growable array, appends have O(log N) worst-case latency instead of O(N) for standard dynamic arrays. This makes the structure suitable for live trace recording — events can be appended in real time while the viewer renders the accumulated data.

The general principle: trace data structures should be designed for the access pattern of visualization (zoom = level-of-detail query), not for the access pattern of recording (sequential append). Pre-aggregation at write time eliminates work at read time.

Source: https://thume.ca/2021/03/14/iforests/

### 7.12. Intel Processor Trace / magic-trace — Hardware Branch Recording

Intel Processor Trace (PT), available since Skylake, records every branch taken by the CPU into a compact bitstream. The hardware writes trace packets that bypass the cache, so the only overhead is reduced memory bandwidth (~1 GB/s). On most workloads this is unmeasurable — under 5%.

Jane Street built `magic-trace` on top of PT. It uses PT's ring buffer mode combined with a hardware breakpoint trigger: PT continuously records into a ring buffer, and when a trigger function is called (or the program crashes), magic-trace snapshots the last ~10ms of execution. The result is a complete function-call timeline viewable in Perfetto.

This is transformative for tail latency debugging: leave magic-trace attached in production, trigger on "this request took too long," and you get a full trace of everything that happened leading up to the slow event. No instrumentation, no log statements, no reproduction needed.

PT can also be used via `perf record` and viewed in LLDB. But the raw instruction-level traces are overwhelming; magic-trace's contribution is making the data *useful* by converting it to a function-call timeline.

Source: https://blog.janestreet.com/magic-trace/ and https://thume.ca/2023/12/02/tracing-methods/

### 7.13. GraalVM Truffle — AST Wrapper Node Instrumentation

Truffle's instrumentation framework inserts "wrapper nodes" into the AST around instrumentable nodes. When no instrumentation is active, the wrapper node is not present — the AST is the same as the non-debug case. When a tool (debugger, profiler) requests instrumentation, Truffle inserts wrapper nodes that intercept execution events (enter, return, exception) and forward them to the tool.

The critical property: Truffle's partial evaluation and compilation pipeline treats wrapper nodes like any other AST nodes. After JIT compilation, an active wrapper's event dispatch can be inlined and optimized. When instrumentation is removed, the wrapper node is removed and the code is recompiled — returning to full speed.

This means the cost of instrumentation is paid only in interpreted mode or during recompilation. Once the JIT stabilizes with the instrumentation in place, the overhead is minimal. And when instrumentation is removed, performance returns to the non-instrumented baseline after a recompilation.

This is the most sophisticated approach to "zero overhead when disabled" that we found: not through patching, but through treating instrumentation as part of the program's optimizable AST.

Source: https://www.graalvm.org/latest/graalvm-as-a-platform/language-implementation-framework/ and https://www.graalvm.org/latest/graalvm-as-a-platform/implement-instrument/

### 7.14. Valgrind Memcheck — Shadow Memory

Valgrind's Memcheck tool maintains a "shadow" for every byte of memory in the program. Each byte has two shadow bits: an "A" (addressability) bit indicating whether the byte is legally accessible, and "V" (validity) bits indicating whether the byte's value is defined.

Every memory operation — load, store, arithmetic — is instrumented by Valgrind's dynamic binary translator (VEX) to propagate shadow state. If an undefined value flows into a branch condition or a syscall argument, Memcheck reports it.

The overhead is 10–50x slowdown, which is enormous. But the technique — maintaining a parallel "shadow" of every piece of program state — is conceptually powerful. It is the most thorough form of runtime verification: every byte is tracked, every operation is checked. Shadow memory has been adapted for other tools: taint tracking (which bytes came from untrusted input?), race detection (which bytes were accessed by multiple threads?), and type tracking.

The general principle — "for every real thing, maintain a shadow thing with metadata" — applies far beyond memory checking. Applied to a bytecode VM, a shadow buffer tracking "which instructions have been executed" or "what was the last value produced here" is the same idea at a different granularity.

Source: https://valgrind.org/docs/shadow-memory2007.pdf

### 7.15. Coz — Causal Profiling via Virtual Speedup

Coz is a causal profiler that answers the question "if I optimized this line of code, how much would my program speed up?" It does this by *virtually speeding up* a line of code — not by making it faster, but by making everything else slower. Specifically, it inserts microsecond-scale pauses into all threads except when they are executing the target line. This has the same effect on relative performance as making the target line faster.

By running the program many times with different virtual speedup amounts (0% to 100%) applied to different lines, Coz builds a profile showing the causal relationship between each line's performance and the program's overall throughput or latency.

Traditional profilers tell you where time is spent. Coz tells you where time *matters*. A line that takes 30% of total time might yield 0% speedup when optimized (because something else is the bottleneck), while a line taking 2% of total time might yield 20% speedup (because it is on the critical path). Only causal profiling can distinguish these cases.

Mean overhead is ~17%. The technique requires "progress points" — markers where the developer indicates meaningful work completion (e.g., end of a request, end of a frame). This is a small annotation burden but essential for defining what "throughput" means.

Source: https://web.cs.umass.edu/publication/docs/2014/UM-CS-2014-010.pdf and https://blog.acolyer.org/2015/10/14/coz-finding-code-that-counts-with-causal-profling/

### 7.16. Tcl — Variable and Execution Traces

Tcl provides first-class tracing as a language feature via the `trace` command. You can attach callbacks to:

- **Variable traces:** fire when a variable is read, written, or unset. `trace add variable x write myHandler` calls `myHandler` every time `x` is assigned.
- **Execution traces:** fire when a command enters or leaves execution, including per-step tracing through a procedure. `trace add execution myProc enterstep myHandler` calls `myHandler` for every command executed inside `myProc`.

The implementation is in the interpreter's variable access and command dispatch paths. When no traces are active on a variable or command, there is no overhead — the trace list is simply empty. When traces are active, each access checks the trace list and invokes callbacks.

Tcl's approach is notable for making tracing a *language-level* concept rather than a tool-level concept. Any Tcl code can set up traces on any other Tcl code's variables or commands. This enables debuggers, profilers, and monitoring tools to be written entirely in Tcl itself, with no special VM support beyond the trace mechanism.

The limitation is overhead: execution traces on hot commands add a function call per step. But for debugging and development, where the goal is understanding rather than performance, this is acceptable.

Source: https://www.tcl-lang.org/cgi-bin/tct/tip/86.html and https://wiki.tcl-lang.org/page/Tracing+with+enterstep

### 7.17. Tristan Hume — Tracing Tools Survey and Perfetto Integration

Tristan Hume (Jane Street, later Anthropic) compiled one of the most comprehensive surveys of tracing techniques. Beyond cataloguing tools, he describes a practical workflow combining multiple techniques:

**Perfetto as universal trace viewer:** Any timestamped event data can be rendered in Google's Perfetto UI by writing a trivial JSON format. This turns "I have some log data" into "I have an interactive zoomable timeline" in minutes. Hume writes custom trace exporters for each new data source, building up a library of visualization tools.

**eBPF + Perfetto for production debugging:** He attached eBPF probes to kernel network packet paths, recording every packet's metadata into a ring buffer at 1 million packets/second with no measurable overhead, then visualized the results in Perfetto alongside userspace events.

**Userspace tracing via syscall abuse:** To correlate userspace events with kernel traces, he used a clever trick: call a syscall that has a fast error path (like `faccessat2` with invalid arguments) from userspace, and trace that syscall in eBPF. The ~700ns overhead per event is comparable to dedicated instrumentation libraries, but requires no library — just a syscall.

**Fuchsia Trace Format for speed:** For low-overhead instrumentation, he used the compact binary Fuchsia Trace Format (FTF) instead of JSON, achieving <10ns per span.

The meta-insight: the tracing ecosystem is fragmented across dozens of tools, but they can be composed. The bottleneck is usually not collecting data but making it *viewable*. Perfetto's JSON format is the universal adapter that makes any data source useful.

Source: https://thume.ca/2023/12/02/tracing-methods/

### 7.18. Fast (Trapless) Kernel Probes Everywhere (USENIX ATC 2024)

Standard Kprobes in Linux rely heavily on double traps, leading to severe penalties that prevent efficient kernel instrumentation on a large scale. IBM Research developed a trapless kernel probing mechanism that applies strategically placed NOPs, slightly modifying the code layout to bypass the typical restrictions of probe optimization. This yields a 10x improvement over standard Kprobes while preserving performance across 96% of the kernel code.

Source: https://research.ibm.com/publications/fast-trapless-kernel-probes-everywhere

### 7.19. MAMBO — Low-Overhead Dynamic Binary Instrumentation (DBI)

MAMBO is a high-performance DBI tool specialized for ARM (AArch32, AArch64) and RISC-V. Unlike traditional instrumentation like DynamoRIO or PIN that often suffer from massive overhead, MAMBO utilizes dynamic recompilation interlaced with logging. It keeps the original code untouched (evading anti-debug tricks and checksums) while executing a heavily optimized cloned copy with tracing. It provides branch-level tracing capability with significantly lower overhead, ideal for embedded and RISC architectures.

Source: https://github.com/beehive-lab/mambo

### 7.20. Zero-Overhead Profiling via EM Emanations (ZoP)

Zero-Overhead Profiling takes literal "zero overhead" to the physical layer. Instead of adding software instrumentation or hardware performance counters, ZoP analyzes the electromagnetic (EM) emanations naturally generated by the CPU during program execution. The system runs a training phase to map EM waveforms to code paths, and then records the uninstrumented program during actual execution. By matching the waveforms, it tracks the execution path with >94% accuracy, completely avoiding any modifications to the target system or memory footprint.

Source: https://sites.gatech.edu/ece-alenka/wp-content/uploads/sites/463/2016/09/ZoP.pdf

### 7.21. eBPF uprobes — Zero-Overhead GPU and User-Space Monitoring

While kernel probes trace the OS, `uprobes` attach to user-space functions, offering near-zero overhead when disabled. This is highly effective for profiling closed-source drivers like NVIDIA's CUDA Runtime API (`libcudart.so`). Tools can trace exact function arguments and timing (e.g., `cuMemAlloc`, `cuLaunchKernel`) using `uprobe` and `uretprobe` directly from user space without modifying the target process or requiring a context switch to a traditional debugger.

Source: https://medium.com/@kcl17/inside-cuda-building-ebpf-uprobes-for-gpu-monitoring-449519b236ed

### 7.22. devops-rewind — Branching Terminal Session Debugger

Bringing time-travel debugging concepts to the CLI, `devops-rewind` records every command (and its stdout/stderr, exit code, and context) as a numbered "step". When a multi-step deploy script or process fails, users can "rewind" to a specific step instead of restarting from scratch. The branching engine copies the session state from 0 through N into a new object and opens a new recorder, functioning like `git checkout` but for live terminal execution history.

Source: https://dev.to/lakshmisravyavedantham/i-built-a-terminal-session-debugger-with-rewind-breakpoints-and-branching-3gka

### 7.23. Hardware LBR (Last Branch Record) for Transactional Memory Debugging

Intel processors maintain a hardware ring buffer called the Last Branch Record (LBR) which continually records the source and destination addresses of the last few dozen branches without any software overhead. As noted by kernel developer Andi Kleen, this is the only viable way to debug Hardware Transactional Memory (TSX). Normal profilers or debuggers require interrupts or breakpoints, but any interrupt instantly aborts a TSX transaction, obscuring the root cause. LBR runs silently in the background, allowing developers to inspect the exact branch history that led to an internal transaction abort without interfering with the transaction itself.

Source: https://lwn.net/Articles/680996/

### 7.24. Raven: RISC-V Physical Memory Protection (PMP) as a Debugging Primitive

Because bare-metal RISC-V often lacks standardized external debugging hardware, researchers from SUSTech designed "Raven". Instead of relying on JTAG or dedicated debug modules, Raven creatively repurposes the RISC-V Physical Memory Protection (PMP) security feature into a debugging primitive. By restricting access to specific memory regions, they trigger PMP faults that act as extremely lightweight, zero-modification watchpoints and breakpoints. This allows full kernel debugging capabilities (stepping, introspection) on completely bare-metal embedded targets with near-zero idle overhead.

Source: https://fengweiz.github.io/paper/raven-dac22.pdf (DAC '22)

### 7.25. Rust `zerogc` — Zero-Overhead Tracing GC via the Borrow Checker

While garbage collectors traditionally require runtime tracking of roots, `zerogc` is an experimental Rust project that literally offloads root tracking to the compiler. By abusing Rust's lifetime system and the borrow checker, it tracks roots and enforces safepoints entirely at compile time. Modifying pointers has mathematically zero runtime cost (the GC pointer is just a `Copy` reference). Safepoints are explicit blocks, and between those safepoints, the tracing overhead is strictly zero because the compiler guarantees mathematically that no roots are lost. 

Source: https://docs.rs/zerogc / https://github.com/DuckLogic/zerogc

### 7.26. HUGLO — Hyper-Ultra-Giga Low-Overhead Ruby Profiler

Sampling profilers often fail to catch P99.9 tail latency because they only see the *average* state of the program. Tracing profilers catch everything but are traditionally too slow for production. In a personal blog, Matt Stuchlik details building HUGLO, a Ruby tracer capturing function calls, syscalls, and thread states with strictly less than 100ns of overhead per call. By utilizing extreme micro-optimizations and native extensions to avoid Ruby-level object allocations, he built a production-safe, continuously-running tracer that catches the exact outliers that standard profilers obscure.

Source: https://blog.mattstuchlik.com/2025/04/23/low-overhead-ruby-tracing.html

---

## 8. Automated Fault Isolation

### 8.1. Delta Debugging — Minimizing Failure-Inducing Input

Andreas Zeller's delta debugging algorithm (1999) answers the question: "what is the minimal input that still triggers this bug?" Given a failing input, it systematically removes chunks — first large halves, then progressively smaller pieces — testing after each removal whether the failure persists. The result is a 1-minimal failing input: removing any single element causes the failure to disappear.

The algorithm extends beyond inputs. Applied to code changes, it answers "which of these 1000 commits introduced the bug?" by binary-searching the change history (this is the principle behind `git bisect`). Applied to program state, it can isolate the minimal state difference between a passing and failing execution.

The technique is fully automated — it requires only a test oracle (pass/fail) and a way to produce subsets of the input. No understanding of the program is needed. It is the canonical example of debugging-as-search: the bug is somewhere in a large space, and delta debugging narrows the space systematically.

The practical limitation: each test requires a full program execution. For programs that take seconds or minutes to run, minimizing a large input can take hours. But for programs that run in milliseconds (unit tests, parsers, compilers), delta debugging is transformative.

Source: https://www.debuggingbook.org/html/DeltaDebugger.html and https://www.cs.purdue.edu/homes/xyzhang/fall07/Papers/delta-debugging.pdf

### 8.2. Program Slicing — "What Affects This Variable?"

Mark Weiser introduced program slicing in 1981: given a variable at a program point, compute the subset of the program that could affect that variable's value. This "slice" is itself a valid program — it computes the same value for the variable of interest while discarding everything irrelevant.

**Static slicing** considers all possible executions: the slice includes every statement that *could* affect the variable on *any* input. Static slices tend to be large (often 30–50% of the program) but require no execution.

**Dynamic slicing** considers a specific execution: the slice includes only statements that *actually* affected the variable on *this* input. Dynamic slices are much smaller and more useful for debugging, but require running the program with the failing input.

The practical application: when a variable has a wrong value, the dynamic backward slice tells you exactly which statements contributed to that value. This is a mechanical version of what programmers do mentally — "where did this value come from?" — but computed automatically.

Dynamic slicing is closely related to Pernosco's reverse dataflow tracking. The difference is granularity: program slicing operates on source statements, while Pernosco operates on individual memory writes and register transfers.

Source: https://en.wikipedia.org/wiki/Program_slicing and http://www0.cs.ucl.ac.uk/staff/mharman/sf.html

### 8.3. Tarantula — Fault Localization via Test Coverage Coloring

Tarantula (Jones, Harrold, Stasko, 2002) is a spectrum-based fault localization technique. Given a test suite with some passing and some failing tests, it computes a "suspiciousness" score for each source line:

```
suspiciousness(s) = (fail(s) / total_fail) / (fail(s) / total_fail + pass(s) / total_pass)
```

Lines executed mostly by failing tests get high suspiciousness (close to 1.0). Lines executed mostly by passing tests get low suspiciousness (close to 0.0). Lines executed equally by both get 0.5.

The visualization colors each source line on a red-to-green gradient: red = highly suspicious (likely buggy), green = likely correct. The programmer reads the color-coded source and focuses on the red lines.

The remarkable property: Tarantula requires no program analysis, no symbolic execution, no formal methods. It requires only (a) a test suite with at least one failing test and (b) line-level coverage information for each test. Both are routinely available in modern development workflows.

Later techniques (Ochiai, DStar, etc.) improved the suspiciousness formula, but Tarantula's contribution was showing that coverage × pass/fail is sufficient to localize faults with surprising accuracy. Empirically, Tarantula examines less than 20% of the code to find the fault in most cases.

Source: https://dl.acm.org/doi/10.1145/1101908.1101949 and https://faculty.cc.gatech.edu/~harrold/6340/cs6340_fall2009/Slides/class20.pdf

---

## 9. Retroactive and Partial Evaluation

### 9.1. Replay.io — Retroactive Console.log

Replay.io is a time-travel debugger for web applications that records a browser session deterministically. The unique feature: after recording, you can **add console.log statements retroactively**. Click on a line of code, type an expression, press enter — and the logged values appear in the console as if the log statement had always been there.

The implementation: Replay maintains a pool of forked browser processes at various points in the recording. When you add a retroactive print statement, Replay finds the nearest process fork, replays forward to each point where the line executes, evaluates the expression, and returns the results. Because the work is done in parallel across multiple forks, and no fork is more than ~100ms away from a checkpoint, results appear in "low logarithmic time."

This is philosophically significant: it eliminates the "I should have added a log here" regret that plagues traditional debugging. The recording captures everything; print statements become a *query language* for the recording rather than instrumentation that must exist before the bug occurs.

Source: https://docs.replay.io/time-travel-intro/add-console-logs-on-the-fly and https://docs.replay.io/basics/time-travel/how-does-time-travel-work

### 9.2. Hazel — Live Evaluation of Incomplete Programs

Hazel is a live functional programming environment where programs with **typed holes** — missing subexpressions — can still be typechecked and partially evaluated. The editor inserts holes automatically to guarantee that every editor state is meaningful. There are no "syntax error" states where all feedback stops.

When a program has holes, Hazel evaluates as far as it can, producing partial results. A function with a hole in one branch can still be evaluated on inputs that take the other branch. A list operation with a hole in its transform function can still report the list's length. The result is continuous, live feedback even while the program is being written.

This inverts the traditional model where feedback requires a complete, parseable, compilable program. In Hazel, *every keystroke* produces feedback because *every editor state* has both static (type) and dynamic (evaluation) meaning.

The connection to debugging: Hazel dissolves the distinction between "writing" and "debugging." You see the program's behavior as you construct it, not after. Bugs are visible the moment they are introduced, because you can see values flowing through incomplete code in real time.

Source: https://hazel.org/ and https://arxiv.org/abs/1805.00155

---

## 10. DWARF Debug Information & Optimized Code Challenges

### 10.1. DWARF Location Expressions — Tracking Variables Through Optimization

DWARF debug information describes where variables live at each point in execution using "location descriptions" — stack machine programs that compute a variable's address or value. A simple local variable might be described as "register R12" or "frame pointer + 16." But after optimization, variables are split, merged, partially spilled, or eliminated entirely.

DWARF handles this with location lists: a variable's location can change at different program counter ranges. At PC 0x100–0x120, the variable is in R12. At PC 0x120–0x140, it has been spilled to [RBP-24]. At PC 0x140–0x160, it has been optimized away entirely ("value unavailable"). The debugger consults the location list for the current PC to determine where to find each variable.

The correctness problem is severe. Li et al. (PLDI 2020) presented the first systematic framework for validating debug information in optimized code, finding that both GCC and LLVM produce incorrect DWARF information that causes debuggers to display wrong variable values. Assaiante et al. (2022) studied completeness — variables whose locations should be reportable but aren't — finding that 6–22% of variable locations are unnecessarily missing across GCC and Clang.

The practical consequence: debugging optimized code is unreliable. Variables show as "optimized out" even when their value is recoverable, and worse, sometimes show incorrect values. This is why developers often resort to `-O0` for debugging, sacrificing the 2–5x performance of optimized code. A language that generates correct, complete debug information — or provides its own debugging mechanism that bypasses DWARF — sidesteps this entire class of problems.

Source: https://dwarfstd.org/doc/Debugging-using-DWARF-2012.pdf and https://faculty.cc.gatech.edu/~qzhang414/papers/pldi20_yuanbo1.pdf

### 10.2. Debug Information Across Tiers — The Mapping Problem

When a program passes through multiple compilation stages (source → AST → IR → optimized IR → machine code), each transformation must carry debug information forward. Every instruction in the final machine code should map back to a source location, and every live variable should be locatable.

The mapping problem compounds across tiers:
- **Inlining** duplicates source locations — the same source line appears at multiple machine code addresses.
- **Loop unrolling** multiplies instructions — one source loop body becomes N copies.
- **Dead code elimination** removes instructions — some source lines have no corresponding machine code.
- **Register allocation** moves values — a variable's storage changes at spill/reload boundaries.

For JIT-compiled languages, the problem is worse: the JIT must emit debug information on the fly, and deoptimization must reconstruct source-level state from optimized representations. V8's TurboFan and HotSpot's C2 both maintain "frame state" metadata that describes how to reconstruct the interpreter's stack frame from the optimized code's register allocation — enabling deoptimization at any safepoint.

The lesson: debug information is not optional metadata bolted on at the end. It is a cross-cutting concern that every compilation pass must maintain. Designing the IR with debug info propagation in mind (as LLVM does with `!dbg` metadata on every instruction) is essential.

---

## 11. Debug Adapter Protocol (DAP)

### 11.1. DAP — The LSP of Debugging

The Debug Adapter Protocol (Microsoft, 2016) does for debuggers what LSP did for language servers: it decouples the IDE from the debugger runtime via a standardized JSON-RPC protocol. Before DAP, every IDE had to implement custom integrations with every debugger (Eclipse + JDI for Java, Visual Studio + COM for C++, etc.) — an N×M problem.

DAP defines a standard set of requests (launch, attach, setBreakpoints, continue, stepIn, stackTrace, variables, evaluate) and events (stopped, output, terminated) that any IDE can send to any debug adapter. The debug adapter translates these into the native debugger's API (GDB/MI, LLDB, JDI, Chrome DevTools Protocol, etc.).

Key design decisions:
- **Stateful session model**: unlike LSP (which is largely stateless), DAP maintains a debugging session with lifecycle (initialize → launch/attach → running → stopped → terminated).
- **Thread and stack frame model**: DAP abstracts threads, stack frames, scopes, and variables into a uniform hierarchy, regardless of the underlying runtime's representation.
- **Evaluation**: the `evaluate` request allows arbitrary expression evaluation in the debugger's context, supporting watch expressions, conditional breakpoints, and REPL-style interaction.

DAP is now supported by VS Code, Neovim, Emacs (dap-mode), Helix, Zed, and many other editors. Debug adapters exist for GDB, LLDB, Chrome/Node, Python (debugpy), Go (Delve), Rust (via CodeLLDB/LLDB), Java (JDI), and dozens more.

The limitation: DAP's abstraction is lowest-common-denominator. Advanced debugger features (rr's reverse execution, Pernosco's omniscient queries, RemedyBG's shared-memory speed) are hard to express in DAP's generic request/response model. Enet et al. (2023) studied DAP's suitability for domain-specific languages and found similar friction — DSL-specific debugging concepts (model-level stepping, constraint visualization) require protocol extensions.

Source: https://microsoft.github.io/debug-adapter-protocol/ and https://hal.science/hal-04245594v1/document

---

## 12. Compiler-Based Sanitizers

### 12.1. AddressSanitizer (ASan) — Shadow Memory for Memory Safety

AddressSanitizer (Google, 2012) detects memory errors at runtime using compiler instrumentation and shadow memory. The compiler inserts checks before every memory access, and a runtime library manages a shadow memory map that tracks which bytes are valid to access.

The shadow memory scheme maps every 8 bytes of application memory to 1 byte of shadow memory. The shadow byte encodes how many of the 8 bytes are accessible (0 = all 8 accessible, k = first k accessible, negative = all inaccessible for various reasons). Before every load/store, the compiler inserts:

```
shadow_value = *(shadow_base + (addr >> 3))
if (shadow_value != 0 && (shadow_value <= (addr & 7)))
    report_error()
```

This check is 2 loads + 1 branch — roughly 2x overall slowdown. ASan detects use-after-free (by poisoning freed memory and quarantining it), heap/stack/global buffer overflows (by inserting poisoned "red zones" around allocations), and use-after-return.

### 12.2. ThreadSanitizer, MemorySanitizer, UBSan — The Sanitizer Family

The sanitizer family extends the shadow memory concept:

- **ThreadSanitizer (TSan)**: detects data races by maintaining a shadow "happens-before" clock per memory location. Every read/write records the thread's vector clock; conflicting unsynchronized accesses are reported. Overhead: 5–15x slowdown, 5–10x memory.
- **MemorySanitizer (MSan)**: detects use of uninitialized memory by tracking "definedness" bits — similar to Valgrind's V-bits but implemented via compiler instrumentation rather than binary translation, achieving 3x overhead vs Valgrind's 20x.
- **UndefinedBehaviorSanitizer (UBSan)**: detects undefined behavior (signed integer overflow, null pointer dereference, misaligned access) via targeted compiler checks. Overhead: <5%, making it viable for production use.

The key insight: compiler-based instrumentation is dramatically cheaper than binary translation (Valgrind) because the compiler knows which accesses need checking and can optimize the checks. ASan's 2x overhead vs Valgrind's 20x is the difference between "run it during development" and "run it only when desperate."

All sanitizers are integrated into Clang/LLVM and GCC. They compose with coverage-guided fuzzing (section 13) to form the most powerful automated bug-finding pipeline available.

Source: https://github.com/google/sanitizers/wiki/addresssanitizeralgorithm and https://releases.llvm.org/20.1.0/tools/clang/docs/AddressSanitizer.html

---

## 13. Coverage-Guided Fuzzing

### 13.1. AFL / libFuzzer — Mutation + Coverage Feedback

Coverage-guided fuzzing combines random input mutation with code coverage feedback to systematically explore program behavior. The fuzzer maintains a corpus of inputs and repeatedly:

1. Picks an input from the corpus.
2. Mutates it (bit flips, byte insertions, dictionary-based substitutions, splice with another input).
3. Runs the target program with the mutated input.
4. Measures code coverage (which basic blocks/edges were executed).
5. If the mutated input discovered new coverage, adds it to the corpus.

**AFL** (Zalewski, 2013) pioneered this approach using compile-time instrumentation that updates a shared-memory coverage bitmap (64KB) at each branch. The bitmap hashes `(prev_block ^ curr_block)` to record edge transitions. AFL can process thousands of executions per second and has found thousands of security vulnerabilities in real-world software.

**libFuzzer** (LLVM) takes an in-process approach: the fuzzer and target run in the same process, avoiding the `fork()/exec()` overhead of AFL. This enables millions of executions per second for small targets. libFuzzer uses LLVM's SanitizerCoverage instrumentation for feedback.

The combination of fuzzing + sanitizers is transformative: fuzzing generates inputs that explore new code paths, and sanitizers detect subtle bugs (buffer overflows, use-after-free, data races) that would otherwise produce no observable symptom. Google's OSS-Fuzz runs continuous fuzzing on 1,000+ open-source projects, finding 10,000+ bugs.

A compiler that can instrument code for coverage feedback (as LLVM's SanitizerCoverage does) gives fuzzing support for free. A language runtime with built-in coverage tracking enables fuzzing without compiler modifications.

Source: https://llvm.org/docs/LibFuzzer.html and https://lcamtuf.coredump.cx/afl/

---

## 14. Async & Coroutine Debugging

### 14.1. The Async Stack Problem

Traditional debuggers show the call stack — the chain of function calls leading to the current point. For async/await, coroutines, and goroutines, the call stack is misleading: it shows the executor/scheduler's stack, not the logical chain of `await` calls that led to the current suspension point.

The core challenge: when a coroutine suspends and later resumes, the physical stack frame that created the coroutine may no longer exist. The "parent" in the async sense is not the caller in the stack sense. Debugging an async Python program with GDB shows `_selector.poll()` at the top of every stack — the event loop — with no indication of which coroutine is stuck or why.

Solutions across languages:
- **Go's `runtime/trace`**: captures discrete scheduler events (goroutine creation, blocking, unblocking) and visualizes them as a timeline in `go tool trace`. Delve (Go debugger) understands goroutine semantics and can list/switch between goroutines, showing each goroutine's logical stack.
- **Kotlin coroutines**: IntelliJ's debugger reconstructs coroutine call chains using metadata stored in coroutine objects. The Parallel Stacks plugin (Google, 2023) provides a visual graph of coroutine relationships — which coroutines spawned which, and their current states.
- **JavaScript async stack traces**: V8 and Chrome DevTools maintain "async stack traces" by recording the call stack at each `await` point and stitching them together. This creates a synthetic stack that shows the full async call chain, even though the physical stacks are gone.
- **Rust async**: debugging Rust futures is notoriously difficult because the compiler transforms async functions into state machines. The physical stack shows the executor; the logical async chain requires interpreting the future's internal state. `tokio-console` provides runtime introspection of Tokio tasks, showing task states, waker graphs, and poll durations.

The design lesson: async debugging requires runtime cooperation. The runtime must record enough metadata (parent task, spawn site, await site) to reconstruct logical call chains. Languages that design async runtimes with debugging metadata from the start (Go, Kotlin) provide dramatically better debugging experiences than languages where async was added later (Python, Rust).

Source: https://github.com/go-delve/delve and https://kotlinfoundation.org/news/gsoc-2023-parallel-stacks/

---

## 15. Distributed Tracing & Observability

### 15.1. OpenTelemetry — The Observability Standard

OpenTelemetry (OTEL) is the emerging standard for distributed tracing, metrics, and logging across service boundaries. It defines:

- **Traces**: a tree of spans representing the path of a request through a distributed system. Each span has a trace ID, span ID, parent span ID, start/end timestamps, attributes (key-value metadata), and events (timestamped log entries within a span).
- **Context propagation**: trace context (trace ID, span ID, trace flags) is serialized into HTTP headers (W3C Trace Context format: `traceparent: 00-{trace_id}-{span_id}-{flags}`) and propagated across service boundaries. Each service extracts the context, creates a child span, and propagates the updated context to downstream calls.
- **Baggage**: arbitrary key-value pairs that propagate alongside trace context — enabling cross-cutting concerns like tenant ID, feature flags, or user ID to flow through the entire request path without explicit parameter passing.

The architecture separates concerns:
- **API**: language-specific interfaces for creating spans and propagating context. Applications code against the API.
- **SDK**: configurable implementation that batches, samples, and exports telemetry data. Swappable exporters send data to backends (Jaeger, Zipkin, Datadog, Grafana Tempo).
- **Collector**: a standalone process that receives, processes, and forwards telemetry data — acting as a proxy between applications and backends.

OTEL has become the de facto standard, with official SDKs for Go, Java, Python, JavaScript, Rust, C++, .NET, Ruby, PHP, Swift, and Erlang/Elixir. Cloud providers (AWS, GCP, Azure) and observability vendors have converged on OTEL as the common wire format.

The connection to language design: if a language runtime provides built-in context propagation (similar to Racket's continuation marks or Go's `context.Context`), distributed tracing becomes a first-class capability rather than a library concern. The runtime can automatically create spans for function calls, propagate context across async boundaries, and correlate traces with local profiling data.

Source: https://opentelemetry.io/docs/concepts/signals/traces/ and https://opentelemetry.io/docs/concepts/context-propagation/

---



## 16. Additional Implementations Worth Adding

### 16.1. JDK Flight Recorder (JFR) — Thread-Local Buffers + Global Circular Buffer

JFR is one of the cleanest examples of a production tracing system designed from the start around the "flight recorder" metaphor: keep a rolling history with low enough overhead that it can stay available in real systems. OpenJDK's design writes events lock-free to **thread-local buffers**; when those fill, they are promoted into a **global in-memory circular buffer** that keeps the most recent history. Depending on configuration, the oldest data is either discarded or flushed to disk as a `.jfr` recording.

The original angle is not just "low-overhead tracing," but **after-the-fact incident analysis without having to predict the exact failure point in advance**. JFR is explicitly designed so that you can leave it off, turn it on selectively, or keep a rolling buffer and dump the recent past when operations detect a problem. This is closer in spirit to an aircraft flight recorder than to a classic profiler session.

JEP 328 set explicit success metrics: **at most 1% out-of-the-box overhead on SPECjbb2015** and **no measurable overhead when not enabled**. That is an unusually concrete performance target for a tracing facility. Also notable is the event model: events are typed, self-describing, and can come from the JVM, JDK libraries, the OS, and user code via `jdk.jfr.Event`.

The trade-off is scope. JFR is excellent for JVM-centric troubleshooting, but it is not a source-level debugger, and it does not give you arbitrary native process introspection the way OS-level tracing or DBI tools do.

Source: https://openjdk.org/jeps/328 and https://docs.oracle.com/javacomponents/jmc-5-4/jfr-runtime-guide/about.htm

### 16.2. Event Tracing for Windows (ETW) — Provider / Controller / Consumer Sessions

ETW is the canonical Windows tracing substrate and deserves a dedicated entry. Its architecture is particularly elegant: **providers** emit events, **controllers** start/stop/configure trace sessions, and **consumers** read the resulting stream in real time or from ETL files. This separation makes tracing a dynamic system capability instead of a compile-time choice.

The implementation detail that stands out is the buffering strategy. An ETW logging session is a kernel-managed collection of **in-memory non-paged buffers**, ETW **assigns a buffer to each processor**, and event generation/buffering is **lock-free**. This is the Windows analogue of the "engineered-for-production" philosophy that makes ftrace and JFR so compelling: the hot path is optimized first, and tooling is built around that constraint.

The original side here is the **session model**. Multiple providers can be composed into a single session; sessions can be enabled or disabled dynamically without restarting the system or process; and the same infrastructure supports debugging, performance analysis, and production observability. ETW is not just a tracer — it is an operating-system-level event bus with trace semantics.

The downside is ecosystem complexity. ETW is immensely capable, but provider discovery, schema evolution, event volume management, and tooling ergonomics are all non-trivial. The mechanism is excellent; the human factors are harder.

Source: https://learn.microsoft.com/en-us/windows-hardware/test/wpt/sessions and https://learn.microsoft.com/en-us/windows-hardware/drivers/devtest/about-event-tracing-for-drivers and https://learn.microsoft.com/en-us/windows-hardware/test/wpt/event-tracing-for-windows

### 16.3. .NET EventPipe — Cross-Platform Runtime Tracing via Diagnostic Port

EventPipe is .NET's answer to the question "what is the ETW-like thing if I want it to work the same way on Windows, Linux, and macOS?" It is built into the runtime, collects events from runtime components and `EventSource` providers, serializes them to `.nettrace`, and can stream them to an external consumer through a **diagnostic port**.

Its original side is the combination of **cross-platform semantics**, **out-of-process control**, and **low operational friction**. Unlike ETW or `perf_events`, EventPipe does not require platform-specific high-privilege tracing infrastructure. Microsoft explicitly documents that, for EventPipe, the tracer can operate as the **same user** as the target process rather than requiring admin/root access. That is a strong design choice: make production diagnostics available to application teams, not just system administrators.

The session API also exposes the design directly. A client can request rundown data and specify the size of the **circular buffer** the target runtime should use while collecting events. This makes EventPipe feel less like an opaque profiler and more like a first-class, scriptable diagnostic protocol.

The limitation is scope: EventPipe only sees **managed code and the runtime itself**. If you need kernel events, native stacks for arbitrary unmanaged libraries, or whole-system scheduling context, you still have to go out to ETW, `perf_events`, or other OS-native tools.

Source: https://learn.microsoft.com/en-us/dotnet/core/diagnostics/eventpipe and https://learn.microsoft.com/en-us/dotnet/core/diagnostics/microsoft-diagnostics-netcore-client and https://learn.microsoft.com/en-us/dotnet/core/diagnostics/dotnet-trace

### 16.4. WinDbg Time Travel Debugging (TTD) — Reverse Debugging + Queryable Trace Objects

WinDbg's Time Travel Debugging is not just "reverse execution on Windows." Its original contribution is the **query model** layered on top of the recording. Microsoft describes TTD as capturing a trace of process execution and replaying it forward or backward, but the really interesting part is that the trace is exposed as a set of **data-model objects** accessible via `dx`, JavaScript, and C++.

That makes TTD closer to a searchable execution database than to a traditional breakpoint-driven debugger. The most compelling example is the `TTD.Memory(begin, end, mask)` query interface, which returns a collection of memory-access objects for an arbitrary address range, including thread ID, IP, access type, time position, size, and value. This is a very different workflow from "set a watchpoint and wait." Instead, you **search the recorded past**.

This also distinguishes TTD from rr in a useful way. rr's killer move is deterministic replay with hardware watchpoints and reverse-continue. TTD's killer move is that the recording is directly queryable with debugger-object infrastructure, which makes large-scale "who touched this?" investigations much more natural inside the debugger itself.

The trade-off is that TTD is still a recording-based system: it adds overhead while capturing, and it is Windows-centric. But as a debugger UX idea — execution history as a queryable object model — it deserves to be in any survey of original debugger designs.

Source: https://learn.microsoft.com/en-us/windows-hardware/drivers/debuggercmds/time-travel-debugging-overview and https://learn.microsoft.com/en-us/windows-hardware/drivers/debuggercmds/time-travel-debugging-object-model and https://learn.microsoft.com/en-us/windows-hardware/drivers/debuggercmds/time-travel-debugging-memory-objects

### 16.5. PANDA — Whole-System Replay with Composable Offline Analysis

QEMU record/replay is already in the document, but PANDA adds the most interesting missing layer: **analysis plugins designed to run on replayed executions**. PANDA is built on QEMU, but the crucial difference is that it couples whole-system record/replay with a reusable plugin ecosystem: `taint2` for dynamic taint analysis, `syscalls2` for syscall tracking, and `OSI` for guest OS introspection on Linux and Windows.

The original side is the plugin composition model. PANDA's plugin-to-plugin interface (PPP) lets plugins publish callbacks and APIs that other plugins consume. This means analyses are not monolithic; they are **assembled**. A file-taint analysis can reuse syscall tracking and OS introspection instead of re-implementing them. That is a much more scalable way to build deep analyses than the "single enormous analysis pass" style common in DBI research prototypes.

The other key design idea is temporal decoupling: record first with modest overhead, then run the expensive analysis offline on the replay. PANDA's own documentation explicitly recommends byte-level taint tracking on previously recorded systems because the live cost of such analyses is high. This is the same philosophy that makes rr/Pernosco powerful, but extended to the whole guest OS and packaged as a plugin platform.

The cost is the cost of emulation and whole-system setup. But as a design pattern — **record once, replay many times, compose heavyweight analyses on the replay** — PANDA is one of the best examples in the space.

Source: https://panda.re/ and https://www.ndss-symposium.org/wp-content/uploads/bar2021_23001_paper.pdf

### 16.6. Arm CoreSight ETM / PTM — Dedicated On-Chip Trace Fabric

The document already covers x86 hardware facilities like debug registers and Intel PT, but the Arm world has an equally important family of ideas under **CoreSight**. CoreSight is not a single feature; it is a **debug and trace architecture** with dedicated infrastructure for control, cross-triggering, routing, buffering, and off-chip export of trace data.

The standout piece is the Embedded Trace Macrocell / Program Trace Macrocell family. Arm's own documentation describes ETM-M33 as providing **non-intrusive program-flow trace**, generating the information needed for tools to reconstruct execution. It can trace all instructions, branch targets, exceptions, and cycle counts, with trigger/filter logic controlling exactly what is recorded. Other CoreSight components route this data onto the trace bus, into on-chip buffers such as ETB/ETF, or out through TPIU to external capture hardware.

What is original here is that the "tracer" is not a patch, a signal, a runtime callback, or a debugger instruction. It is a **hardware trace fabric designed into the SoC**. That matters enormously for real-time and embedded systems, where even a single interrupt or breakpoint can destroy the timing bug you are trying to observe. Arm's own examples emphasize this: trace can capture execution history non-intrusively while the target continues to run at full speed.

The limitation is deployment reality. Whether CoreSight is useful depends on what the SoC actually implemented, how much trace buffer space exists, and whether you have the capture hardware and tooling to retrieve it. But conceptually it is one of the cleanest answers to "how do I trace without perturbing timing?"

Source: https://developer.arm.com/documentation/102520/latest/ and https://developer.arm.com/documentation/100232/latest/ and https://developer.arm.com/community/arm-community-blogs/b/tools-software-ides-blog/posts/how-coresight-trace-helped-me-debug-my-rtos

### 16.7. HyperDbg — Ring -1 Debugging with EPT Hidden Hooks

HyperDbg takes a very different route from source-level debuggers and conventional kernel debuggers: it moves the debugger down to **ring -1**, building on Intel VT-x and EPT. The CCS 2022 paper describes HyperDbg as a hypervisor-based debugger that virtualizes an already running Windows system, aiming to be stealthy and as OS-independent as possible.

The most original mechanism is the **EPT hidden hook**. HyperDbg documents a mode that places a hidden `0xCC` breakpoint on a target function **without modifying the content of memory in the case of reading/writing**. In other words, code inspection from the guest can still see the original bytes even though execution traps on the hooked address. The docs also describe a faster `!epthook2` mode that avoids VM-exits, plus larger ambitions such as invisible read/write watchpoints, coverage measurement, and memory-move monitoring.

This is philosophically different from traditional debugger design. The debugger is not negotiating with the guest OS, nor patching visible code in the usual way. It is using the virtualization layer as the debugging primitive. That makes HyperDbg especially interesting for reverse engineering, anti-anti-debugging, and malware analysis, where being seen by the target is itself part of the problem.

The price is complexity and specificity. This is specialized machinery: Windows, VT-x, EPT, kernel-mode/hypervisor expertise, and significant engineering surface area. But it is genuinely original and belongs in the survey.

Source: https://misc0110.net/files/hyperdbg_ccs22.pdf and https://docs.hyperdbg.org/commands/extension-commands/epthook and https://github.com/HyperDbg/docs

### 16.8. GHC Eventlog + ThreadScope — Runtime-Native Parallel Timeline Debugging

GHC's eventlog mechanism is easy to underestimate if you think only in terms of line stepping and breakpoints. What it offers is a **runtime-native timeline** of the things that actually matter in parallel Haskell: HEC activity, sparks, garbage collection, scheduler events, and user-defined markers. ThreadScope then renders that into a graphical view showing spark creation, spark-to-thread promotions, and GC behavior over time.

The original side is that the trace is expressed in the runtime's own semantic units, not generic CPU samples. For parallel functional programs, that is a much better fit than a conventional profiler. A developer does not just want to know "which function used time"; they want to know whether work was balanced across capabilities, whether sparks were converted into real work, and whether GC or load imbalance dominated the run.

GHC developers also explicitly contrast eventlog with heavier profiling modes. Eventlog emission has much lower runtime impact because events are essentially **a few values written into a buffer** that the RTS flushes later, whereas full profiling changes the generated code much more substantially. That makes eventlog a better "big picture first" tool.

It is not an interactive debugger, and it is obviously Haskell-specific. But as a runtime-aware design for understanding concurrency and parallelism, it is original and worth adding.

Source: https://downloads.haskell.org/ghc/latest/docs/users_guide/runtime_control.html and https://www.haskell.org/ghc/blog/20190924-eventful-ghc.html and https://manpages.ubuntu.com/manpages/jammy/man1/threadscope.1.html


## 17. Summary by Mechanism Family

The previous flat inventory was useful as a checklist, but it mixed together three different layers:

1. **Execution interposition** — how control is intercepted or redirected.
2. **Recording substrate** — how events, states, or histories are captured.
3. **Interpretation layer** — how captured data is turned back into source-level meaning or actionable diagnosis.

Grouping the techniques by family makes the design space easier to navigate. In practice, most real systems combine one row from several families: for example, a debugger may use **cooperative safepoints** to stop code, a **metadata/protocol layer** to expose state to the IDE, and a **buffered event pipeline** to collect performance context.

Sections such as §7.4 are intentionally not given their own row here because they are best read as design perspective rather than as a distinct implementation mechanism.

### 17.1. Direct Execution Interposition

These techniques change the code path itself: patch a site, swap in a trap, or reserve a place where control can be diverted later.

| Mechanism | Discussed in | Cost When Off | Cost When On | Granularity | Representative implementations |
|---|---|---|---|---|---|
| Hook function checked every instruction | background / contrast case | Per-instruction branch | Per-instruction call | Every instruction | Lua `debug.sethook`, CPython `sys.settrace` |
| Bytecode opcode patching | §1.1 | Zero | Per-breakpoint dispatch | Per-instruction | Luau `LOP_BREAK` |
| NOP→CALL / handler-pointer patching | §§1.3, 1.5, 7.18, 7.21 | Zero or one reserved NOP | Per-probe trampoline / handler call | Per-function / per-probe | ftrace, DTrace USDT, trapless kernel probes, eBPF uprobes |
| Jump target patching | §1.2 | One predicted branch | One call | Per-function | Erlang BeamAsm |
| INT 3 instruction patching | §3.1 | Zero | Trap + context switch | Per-instruction | GDB, classic kprobes |
| NOP padding + INT 3 swap | §1.4 | One reserved NOP | Trap + context switch | Per-instruction | Wasmtime Winch |
| Compile-time trace instructions | §7.1 | One NOP / trace site | Per-trace-point call | Per-line / function | Ruby YARV |
| Compiled-in conditional breakpoint | §3.2 | Zero | One compiled branch | Per-site | Wellons `INT3;NOP` |
| In-place binary rewriting via instruction punning | §7.7 | Zero | Per-patched-site logic | Per-instruction | E9Patch |

### 17.2. Cooperative Safepoints and Managed Handoff

These do not trap at arbitrary instructions. Instead, the runtime arranges for threads to notice a stop request at points that are already safe for the implementation.

| Mechanism | Discussed in | Cost When Off | Cost When On | Granularity | Representative implementations |
|---|---|---|---|---|---|
| Memory-protection polling page | §2.1 | One cached load | SIGSEGV + handler | Per-safepoint | HotSpot JVM |
| Existing-branch piggyback (`eval_breaker` style) | §2.2 | Zero (reuses an existing branch) | One extra check + script execution | Per-safe-point | CPython PEP 768, PyPy analogue |
| Compile-time safepoint / root discipline | §7.25 | Zero between safepoints | Explicit safepoint / GC work | Per-safepoint region | Rust `zerogc` |

### 17.3. Hardware-, Hypervisor-, and Out-of-Process Observation

These techniques avoid or minimize software interposition in the target by leaning on CPU facilities, VM assists, faults, or external readers.

| Mechanism | Discussed in | Cost When Off | Cost When On | Granularity | Representative implementations |
|---|---|---|---|---|---|
| External memory reading / sampling | §7.5 | Zero on target | Sampling interval + read cost | Per-sample | py-spy, rbspy |
| Hardware debug registers | §3.3 | Zero | Trap on access | Up to 4 addresses | x86 DR0–DR3 |
| Hardware branch history / PT-style tracing | §§7.12, 7.23 | Very low bandwidth cost | Post-hoc decode / trace handling | Every branch or recent-branch window | Intel PT, magic-trace, LBR |
| Dedicated hardware trace fabric | §16.6 | Zero software overhead | Trace bandwidth / sink limits | Instruction flow | Arm CoreSight ETM/PTM |
| Hypervisor-level hidden hooks | §16.7 | Zero until armed | VM-exit / EPT cost | Per-address / per-access | HyperDbg |
| Faults repurposed as debug primitive | §7.24 | Zero until armed | Fault + handler | Per-region / per-access | Raven (RISC-V PMP) |
| Side-channel execution sensing | §7.20 | Zero runtime overhead | Offline waveform classification | Code path / phase | ZoP |

### 17.4. Buffered Event Emission and Production-Safe Trace Pipelines

Here the core idea is not “stop the program” but “emit structured events cheaply enough that continuous capture is practical.”

| Mechanism | Discussed in | Cost When Off | Cost When On | Granularity | Representative implementations |
|---|---|---|---|---|---|
| Lock-free event queue | §7.6 | Very low idle cost | Per-span overhead | Per-annotated span | Tracy, Spall |
| Match-spec filtered tracing | §7.9 | Minimal pattern/filter cost | Per-matching-call | Per-function-call | Erlang `dbg` |
| Parallel producer/consumer trace ring buffer | §7.10 | QEMU / producer cost | Multi-core consumer cost | Every instruction / memory event | Cannoli |
| Pre-aggregated trace tree | §7.11 | Zero until enabled; O(log N) append | Low-latency append + zoom aggregation | Per-event | implicit in-order forests |
| Context-propagated distributed spans | §15.1 | Context propagation | Per-span overhead | Per-service call | OpenTelemetry, Jaeger |
| Thread-local → global circular buffer recorder | §16.1 | Zero when disabled; very low active cost | Low-overhead continuous recording | Typed runtime events | JDK Flight Recorder |
| Per-CPU kernel trace sessions | §16.2 | Zero until providers are enabled | Lock-free event buffering | Per-event | ETW |
| Runtime diagnostics port + circular buffer | §16.3 | Zero until session attached | Runtime event buffering | Per-event | .NET EventPipe |
| Runtime-native scheduler event log | §16.8 | Low idle cost | Post-hoc analysis cost | Scheduler / GC / user events | GHC eventlog + ThreadScope |
| Ultra-low-overhead language tracer | §7.26 | Very low active-path cost | Per-call / syscall event cost | Per-call / syscall | HUGLO |
| Shadow buffer + ring buffer | §7.14 (conceptual extension) | Zero | ~10 instructions / step | Every instruction | shadow-state recorder design sketch |

### 17.5. Replay, Snapshots, and Omniscient History

This family pays storage and replay costs in exchange for the ability to go backwards, ask post-hoc questions, or inspect a preserved point in time.

| Mechanism | Discussed in | Cost When Off | Cost When On | Granularity | Representative implementations |
|---|---|---|---|---|---|
| Full state recording | §§5.1–5.3 | N/A (always on) | O(state × steps) or O(delta × steps) | Every step | ODB, Toby Ho, Elm |
| Nondeterminism-only recording | §4.1 | ~20% (scheduling / counter discipline) | Replay at original speed | Full program | rr |
| Queryable time-travel trace objects | §16.4 | Recording overhead | Replay + query cost | Whole-process history | WinDbg TTD |
| Post-hoc omniscient analysis | §4.2 | Recording cost (often rr) | Minutes–hours analysis | Memory / value history queries | Pernosco |
| Process fork snapshot | §4.3 | Zero until snappoint | One `fork()` per snapshot | Per-snappoint | Visual Studio Snapshot Debugger |
| Whole-system replay + plugin composition | §§4.4, 16.5 | VM / recording overhead | Offline heavyweight analyses | Whole VM | QEMU record/replay, PANDA |
| Branchable session history | §7.22 | Logging overhead | Clone / branch storage cost | Per command / session step | devops-rewind |
| Retroactive print statements | §9.1 | Recording overhead | Replay + eval per injected log | Per-expression | Replay.io |

### 17.6. Heavyweight Dynamic Instrumentation and Shadow Execution

These are the “pay real overhead to gain deep visibility” designs: DBI, shadow state, and aggressive compiler-inserted checks.

| Mechanism | Discussed in | Cost When Off | Cost When On | Granularity | Representative implementations |
|---|---|---|---|---|---|
| Dynamic binary recompilation | §§7.8, 7.19 | N/A (always on) | 5–50× slowdown, or architecture-specific lower overhead | Every basic block | Frida Stalker, MAMBO |
| Shadow memory metadata | §7.14 | N/A (always on) | 10–50× slowdown | Every byte / access | Valgrind Memcheck |
| Compiler-inserted shadow checks | §12.1 | N/A (always on) | ~2× slowdown | Per-memory-access | AddressSanitizer |
| Race-tracking shadow clocks | §12.2 | N/A (always on) | 5–15× slowdown | Per-memory-access | ThreadSanitizer |
| Definedness tracking via compiler instrumentation | §12.2 | N/A (always on) | ~3× slowdown | Per-memory-access | MemorySanitizer |
| Targeted undefined-behavior guards | §12.2 | N/A (always on) | <5% slowdown | Per-operation | UBSan |

### 17.7. Runtime-Semantic Hooks, Reconstructed Context, and Live Systems

These mechanisms work above the raw instruction stream. They capture or reconstruct concepts the runtime already knows about: variables, frames, async chains, conditions, methods, and AST nodes.

| Mechanism | Discussed in | Cost When Off | Cost When On | Granularity | Representative implementations |
|---|---|---|---|---|---|
| AST wrapper node insertion | §7.13 | Zero when wrappers are absent | Recompilation + event dispatch when active | Per-AST-node | GraalVM Truffle |
| Continuation marks | §5.6 | One mark / allocation per frame | Stack walk on read | Per-frame | Racket |
| Condition / restart system | §5.5 | Zero unless a condition is signaled | Signal + handler / restart dispatch | Per-error / recovery site | Common Lisp |
| Language-level variable / execution trace | §7.16 | Zero when traces are absent | Per-access / callback | Per-variable / command | Tcl `trace` |
| Debugger as live IDE / method editor | §5.4 | N/A | Recompilation on edit | Per-method | Pharo/Smalltalk |
| Partial program evaluation | §9.2 | N/A (always on) | Per-keystroke evaluation | Per-expression | Hazel typed holes |
| Async stack reconstruction | §14.1 | Runtime metadata capture | Stack stitch + lookup | Per-coroutine / task | V8, Go, Kotlin, tokio |

### 17.8. Search, Minimization, Causal Profiling, and Bug Discovery

These techniques do not primarily intercept execution; they search the space around a bug by repeated execution, coverage feedback, or dependency analysis.

| Mechanism | Discussed in | Cost When Off | Cost When On | Granularity | Representative implementations |
|---|---|---|---|---|---|
| Virtual speedup (causal profiling) | §7.15 | ~17% | Multiple runs with perturbation | Per-line / progress point | Coz |
| Delta debugging (input minimization) | §8.1 | N/A | Repeated test runs | Per-input element | `ddmin` |
| Program slicing | §8.2 | N/A (static / hybrid analysis) | Dataflow analysis time | Per-statement / dependency | Weiser, CodeSurfer |
| Coverage-based fault localization | §8.3 | N/A (post-hoc) | Test suite run time | Per-line | Tarantula, Ochiai |
| Coverage-guided fuzzing | §13.1 | Instrumentation cost | Thousands–millions of executions / sec | Per-edge / branch | AFL, libFuzzer |

### 17.9. Metadata, Source Reconstruction, Transport, and Visualization Layers

These are enabling layers. They usually do not capture execution by themselves, but they are what make optimized code debuggable and traces explorable.

| Mechanism | Discussed in | Cost When Off | Cost When On | Granularity | Representative implementations |
|---|---|---|---|---|---|
| Bytecode→native source map | §7.2 | Metadata in compiled code | Lookup at stop points | Per-bytecode / native PC | Apache Harmony, HotSpot |
| DWARF location expressions | §10.1 | Metadata in binary | O(log N) lookup per variable | Per-variable per PC range | GCC, Clang, Go |
| Cross-tier debug mapping | §10.2 | Metadata maintenance across tiers | Lookup / deopt / map maintenance | Per-PC / value | tiered JITs and optimizing runtimes |
| Debug adapter / frontend protocol | §11.1 | N/A (protocol) | JSON-RPC per request | Per-debug-session | DAP ecosystem |
| Shared-memory debugger transport | §7.3 | N/A (transport) | Low-latency shared-memory exchange | Per-request / variable read | RemedyBG-style debugger service |
| Timeline scrubber / universal trace-view interchange | §§6.1, 6.2, 7.17 | UI-side cost | Rendering / conversion cost | Per-event visualization | WhiteBox, Bret Victor timeline scrubber, Perfetto exporters |

The important point is that this space is **not one-dimensional**. There are at least three orthogonal axes:

- **Steady-state overhead** — what the program pays while “nothing interesting” is happening.
- **Semantic richness** — whether the tool sees bytes and PCs, language objects and frames, or whole-system histories.
- **Recoverability** — whether the tool can only observe live execution, can replay it, or can answer arbitrary post-hoc queries.

A practical debugger or tracer usually chooses one mechanism family for control, another for data capture, and a third for presentation or diagnosis.

---

## 18. References

1. Luau Performance — https://luau.org/performance/
2. PEP 768: Safe External Debugger Interface for CPython — https://peps.python.org/pep-0768/
3. JVM Anatomy Quark #22: Safepoint Polls — https://shipilev.net/jvm/anatomy-quarks/22-safepoint-polls/
4. BeamAsm, the Erlang JIT — https://www.erlang.org/doc/apps/erts/beamasm
5. Eli Bendersky: How Debuggers Work, Part 2: Breakpoints — https://eli.thegreenplace.net/2011/01/27/how-debuggers-work-part-2-breakpoints
6. eBPF Trampolines — https://docs.ebpf.io/linux/concepts/trampolines/
7. Bil Lewis: Omniscient Debugging — https://omniscientdebugger.github.io/
8. Toby Ho: Time Traveling Debugger — https://www.tobyho.com/video/Time-Traveling-Debugger-Part-1.html
9. WhiteBox Live Visualizer — https://whitebox.systems/
10. Pernosco Omniscient Debugger — https://pernos.co/
11. rr: Record and Replay Framework — https://rr-project.org/
12. Chris Wellons: Two Handy GDB Breakpoint Tricks — https://nullprogram.com/blog/2024/01/28/
13. Bret Victor: Inventing on Principle — https://vimeo.com/36579366
14. Wasmtime Debugging Design — https://hackmd.io/@hvqFkDgPTuGNcu-NiycXZQ/SyXX166Yp
15. Jamie Brandon: Implementing Interactive Languages — https://www.scattered-thoughts.net/writing/implementing-interactive-languages/
16. Debugger Breakpoints via Code Patching — https://devblogs.microsoft.com/oldnewthing/20241111-00/?p=110503
17. RemedyBG Debug Protocol — https://remedybg.handmade.network/blog/p/3631-remedybgs_debug_protocol
18. Apache Harmony: Breakpoints and Single Step in JIT Mode — https://harmony.apache.org/subcomponents/drlvm/breakpoints_and_ss.html
19. Deterministic Record-and-Replay (ACM Queue) — https://queue.acm.org/detail.cfm?id=3688088
20. Robert O'Callahan: Advanced Debugging Technology — https://robert.ocallahan.org/2024/10/debt-workshop.html
21. DTrace USDT Probes — https://blogs.oracle.com/linux/from-kernel-to-user-space-tracing
22. Intel Processor Trace — https://easyperf.net/blog/2019/08/23/Intel-Processor-Trace
23. Jane Street magic-trace — https://blog.janestreet.com/magic-trace/
24. Tristan Hume: All My Favorite Tracing Tools — https://thume.ca/2023/12/02/tracing-methods/
25. Valgrind Shadow Memory — https://valgrind.org/docs/shadow-memory2007.pdf
26. Coz Causal Profiling — https://web.cs.umass.edu/publication/docs/2014/UM-CS-2014-010.pdf
27. Visual Studio Snapshot Debugger — https://devblogs.microsoft.com/visualstudio/snapshot-debugging-with-visual-studio-2017-now-ready-for-production/
28. Pharo Debugger — https://pharo.org/
29. GraalVM Truffle Instrumentation — https://www.graalvm.org/latest/graalvm-as-a-platform/implement-instrument/
30. Tcl Trace Command — https://www.tcl-lang.org/cgi-bin/tct/tip/86.html
31. QEMU Record/Replay — https://www.qemu.org/docs/master/devel/replay.html
32. Glamorous Toolkit: Moldable Development — https://gtoolkit.com/
33. Racket Continuation Marks (dissertation) — https://www2.ccs.neu.edu/racket/pubs/dissertation-clements.pdf
34. SRFI 157: Continuation Marks — https://srfi.schemers.org/srfi-157/srfi-157.html
35. Common Lisp Condition System — https://lisp-docs.github.io/docs/tutorial/conditions
36. py-spy Sampling Profiler — https://github.com/benfred/py-spy
37. Tracy Profiler — https://github.com/wolfpld/tracy
38. Spall Profiler — https://gravitymoth.com/spall/spall-web.html
39. E9Patch Binary Rewriting — https://pldi20.sigplan.org/details/pldi-2020-papers/12/Binary-Rewriting-without-Control-Flow-Recovery
40. Frida Stalker — https://frida.re/docs/stalker/
41. Erlang dbg Module — https://www.erlang.org/doc/apps/runtime_tools/dbg.html
42. Erlang recon_trace — https://ferd.github.io/recon/recon_trace.html
43. Implicit In-order Forests — https://thume.ca/2021/03/14/iforests/
44. Delta Debugging (The Debugging Book) — https://www.debuggingbook.org/html/DeltaDebugger.html
45. Program Slicing — https://en.wikipedia.org/wiki/Program_slicing
46. Tarantula Fault Localization — https://dl.acm.org/doi/10.1145/1101908.1101949
47. Replay.io Time Travel Debugger — https://docs.replay.io/time-travel-intro/add-console-logs-on-the-fly
48. Hazel Live Programming — https://hazel.org/
49. Hazel: Live Functional Programming with Typed Holes — https://arxiv.org/abs/1805.00155
50. DWARF Debugging Format Introduction — https://dwarfstd.org/doc/Debugging-using-DWARF-2012.pdf
51. Debug Information Validation for Optimized Code (Li et al., PLDI 2020) — https://faculty.cc.gatech.edu/~qzhang414/papers/pldi20_yuanbo1.pdf
52. Where Did My Variable Go? (Assaiante et al., 2022) — https://export.arxiv.org/pdf/2211.09568v1.pdf
53. Debug Adapter Protocol — https://microsoft.github.io/debug-adapter-protocol/
54. DAP Suitability for DSLs (Enet et al., 2023) — https://hal.science/hal-04245594v1/document
55. AddressSanitizer Algorithm — https://github.com/google/sanitizers/wiki/addresssanitizeralgorithm
56. AddressSanitizer (Clang Documentation) — https://releases.llvm.org/20.1.0/tools/clang/docs/AddressSanitizer.html
57. libFuzzer — https://llvm.org/docs/LibFuzzer.html
58. AFL Fuzzer — https://lcamtuf.coredump.cx/afl/
59. Go Delve Debugger — https://github.com/go-delve/delve
60. Kotlin Parallel Coroutine Stacks — https://kotlinfoundation.org/news/gsoc-2023-parallel-stacks/
61. OpenTelemetry Traces — https://opentelemetry.io/docs/concepts/signals/traces/
62. OpenTelemetry Context Propagation — https://opentelemetry.io/docs/concepts/context-propagation/
63. JEP 328: Flight Recorder — https://openjdk.org/jeps/328
64. Oracle Java Mission Control Runtime Guide: About Java Flight Recorder — https://docs.oracle.com/javacomponents/jmc-5-4/jfr-runtime-guide/about.htm
65. Event Tracing for Windows Sessions — https://learn.microsoft.com/en-us/windows-hardware/test/wpt/sessions
66. Event Tracing for Windows Overview — https://learn.microsoft.com/en-us/windows-hardware/test/wpt/event-tracing-for-windows
67. About Event Tracing for Drivers — https://learn.microsoft.com/en-us/windows-hardware/drivers/devtest/about-event-tracing-for-drivers
68. EventPipe Overview — https://learn.microsoft.com/en-us/dotnet/core/diagnostics/eventpipe
69. Microsoft.Diagnostics.NETCore.Client API — https://learn.microsoft.com/en-us/dotnet/core/diagnostics/microsoft-diagnostics-netcore-client
70. dotnet-trace diagnostic tool — https://learn.microsoft.com/en-us/dotnet/core/diagnostics/dotnet-trace
71. WinDbg Time Travel Debugging Overview — https://learn.microsoft.com/en-us/windows-hardware/drivers/debuggercmds/time-travel-debugging-overview
72. WinDbg Time Travel Debugging Object Model — https://learn.microsoft.com/en-us/windows-hardware/drivers/debuggercmds/time-travel-debugging-object-model
73. WinDbg TTD Memory Objects — https://learn.microsoft.com/en-us/windows-hardware/drivers/debuggercmds/time-travel-debugging-memory-objects
74. PANDA.re — https://panda.re/
75. PyPANDA: Taming the PANDAmonium of Whole-System Dynamic Analysis — https://www.ndss-symposium.org/wp-content/uploads/bar2021_23001_paper.pdf
76. Arm CoreSight Debug and Trace Guide — https://developer.arm.com/documentation/102520/latest/
77. Arm CoreSight ETM-M33 Technical Reference Manual — https://developer.arm.com/documentation/100232/latest/
78. HyperDbg: Reinventing Hardware-Assisted Debugging — https://misc0110.net/files/hyperdbg_ccs22.pdf
79. HyperDbg EPT Hook Documentation — https://docs.hyperdbg.org/commands/extension-commands/epthook
80. HyperDbg Documentation Repository — https://github.com/HyperDbg/docs
81. GHC Runtime System Options / Eventlog — https://downloads.haskell.org/ghc/latest/docs/users_guide/runtime_control.html
82. Eventful GHC — https://www.haskell.org/ghc/blog/20190924-eventful-ghc.html
83. ThreadScope Man Page — https://manpages.ubuntu.com/manpages/jammy/man1/threadscope.1.html
