# Coroutines and Yielding

Research on coroutine architectures, context switching strategies, yielding from bytecode virtual machines, debugger integration, and concurrency primitives across languages and runtimes.

---

## 1. Coroutine Taxonomy

### 1.1. Symmetric vs Asymmetric

**Symmetric coroutines** are peers: any coroutine may transfer control to any other coroutine by naming it explicitly. There is no caller/callee relationship. Modula-2 and early Simula used symmetric coroutines. The programming model is powerful but difficult to reason about — every transfer is a potential context switch to anywhere.

**Asymmetric coroutines** (also called semi-coroutines) have a hierarchical relationship: a coroutine can only yield back to whoever resumed it. This creates a natural parent/child call structure. Lua, Python generators, Kotlin coroutines, and most modern implementations use asymmetric coroutines. The asymmetry makes control flow predictable and composable.

De Moura and Ierusalimschy (2009) proved that full asymmetric coroutines have expressive power equivalent to one-shot continuations and one-shot delimited continuations. They are strictly more expressive than generators (which can only yield values, not receive them) and iterators (which are single-use generators). The key insight: asymmetric coroutines are easier to implement and understand than continuations, especially in procedural languages, while providing the same power.

Kobayashi and Kameyama (2025) formally verified the folklore that one-shot effect handlers and one-shot delimited-control operators can be macro-expressed by asymmetric coroutines, but not vice versa. Asymmetric coroutines sit at the top of the one-shot expressiveness hierarchy.

Source: https://www.cs.tufts.edu/~nr/cs257/archive/roberto-ierusalimschy/revisiting-coroutines.pdf

### 1.2. Stackful vs Stackless

**Stackful coroutines** (fibers, green threads) give each coroutine its own call stack. When a coroutine yields, its entire call stack is preserved. Any function at any depth can yield — the yield propagates transparently through the call chain. Lua coroutines, Go goroutines, Boost.Context fibers, and libfringe are stackful.

**Stackless coroutines** store only the local state of the coroutine function itself. Yielding is only possible from the coroutine's top-level function, not from nested calls (unless the entire call chain is transformed). C++20 coroutines, Rust async/await, Kotlin suspend functions, and Python generators are stackless. The compiler transforms each coroutine into a state machine, with each yield point becoming a state transition.

The trade-off is fundamental: stackful coroutines are more flexible (any function can yield) but require stack allocation and full context switches. Stackless coroutines are cheaper (no stack allocation, state machine is heap-allocated only when needed) but require explicit annotation of every function in the yield chain.

A non-obvious consequence: stackless coroutines infect the entire call graph. If function `A` calls `B` which calls `C`, and `C` needs to yield, then `B` and `A` must also be declared suspendable (Kotlin `suspend`, Rust `async`, C# `async`). This is the "function coloring" problem (Bob Nystrom). Stackful coroutines avoid it entirely because yielding is a property of the execution context, not the function signature.

In a bytecode VM, the distinction becomes: stackful means the VM maintains a separate virtual stack per coroutine; stackless means the VM saves and restores only the instruction pointer and locals of the current frame. VMs that already manage their own call stacks (rather than piggybacking the native C stack) get stackful coroutines almost for free — the coroutine state IS the VM state.

### 1.3. First-Class vs Constrained

**First-class coroutines** are values that can be stored in variables, passed to functions, and returned. Lua's coroutines are first-class: `coroutine.create` returns a coroutine object. This enables patterns like cooperative schedulers, producer/consumer pipelines, and backtracking search.

**Constrained coroutines** are restricted to specific syntactic contexts. Python's `yield` can only appear inside a generator function. C# `yield return` can only appear inside an iterator method. The constraint simplifies implementation (the compiler knows exactly which functions need transformation) at the cost of flexibility.

Prokopec and Liu (ECOOP 2018) introduced **coroutines with snapshots** — first-class, type-safe, stackful coroutines that can be cloned (multi-shot). Their design is sufficiently general to express iterators, single-assignment variables, async-await, actors, event streams, backtracking, symmetric coroutines, and continuations. Performance overhead was 1.03–2.11x compared to hand-written code. The snapshot capability enables exploration-based algorithms (backtracking, speculative execution) that one-shot coroutines cannot express.

A subtle point about first-class coroutines: if a coroutine handle can escape the scope that created it, the coroutine's stack must outlive that scope. This means coroutine stacks cannot live in the creating function's frame — they must be heap-allocated or arena-allocated. This is why Go allocates goroutine stacks on the heap, and why Rust's `async` blocks produce heap-allocated futures.

Source: https://drops.dagstuhl.de/storage/00lipics/lipics-vol109-ecoop2018/LIPIcs.ECOOP.2018.3/LIPIcs.ECOOP.2018.3.pdf

---

## 2. Native Context Switching

### 2.1. Full Register Save/Restore — Traditional Approach

The classical approach to stackful coroutine context switching saves all callee-saved registers, swaps the stack pointer, and restores the callee-saved registers of the target coroutine. On x86-64 System V, this means saving `rbx`, `rbp`, `r12`–`r15`, the stack pointer `rsp`, and the return address. The `mxcsr` and x87 control word are also typically saved.

Boost.Context (Oliver Kowalke) is the most widely-used production implementation of this approach. The `fcontext` API provides `make_fcontext` (create a context on a stack) and `jump_fcontext` (switch between contexts). The assembly is ~30 instructions per switch on x86-64. Boost.Context has been ported to x86, x86-64, ARM, ARM64, MIPS, PPC, RISC-V, and s390x.

The `deboost.context` project by septag extracts the Boost.Context assembly into a standalone C library with a minimal API: `fcontext_t make_fcontext(void* sp, size_t size, void (*fn)(fcontext_transfer_t))` and `fcontext_transfer_t jump_fcontext(fcontext_t to, void* vp)`. This is the simplest way to get production-quality context switching in C without pulling in Boost.

The cost of a full context switch is typically 10–20ns on modern x86-64 hardware — dominated by the memory operations for saving/restoring registers and the indirect branch for the return address.

Non-obvious pitfalls of native context switching:

- **Red zone**: System V AMD64 ABI reserves 128 bytes below `rsp` as a "red zone" that leaf functions may use without adjusting `rsp`. A context switch that writes to a new stack must ensure the red zone of the old stack is not clobbered. Boost.Context handles this by subtracting 128 from `rsp` before saving.
- **Stack alignment**: The ABI requires 16-byte stack alignment at the point of a `call` instruction. A context switch that manipulates `rsp` directly must maintain this invariant, or subsequent function calls will fault on aligned SSE moves (`movaps`).
- **Signal delivery**: On POSIX systems, signals can be delivered at any point during execution, including mid-context-switch. If a signal handler runs on the half-switched stack, the result is undefined. Solutions: block signals during the switch (expensive, ~200ns for `sigprocmask`), or ensure the switch is atomic from the signal handler's perspective (the assembly sequence must never leave the stack in an inconsistent state).
- **Valgrind and sanitizers**: Tools like Valgrind, AddressSanitizer, and ThreadSanitizer do not understand custom stack switching. Valgrind provides `VALGRIND_STACK_REGISTER` to declare custom stacks. ASan requires `__sanitizer_start_switch_fiber` / `__sanitizer_finish_switch_fiber` annotations. Omitting these causes false positives or missed bugs.
- **Windows differences**: Windows requires saving the structured exception handling (SEH) chain, the thread information block (TIB) stack limits (`StackBase`, `StackLimit`, `DeallocationStack`), and the fiber-local storage pointer. Failing to save the TIB stack limits causes stack overflow detection to malfunction.

Source: https://www.open-std.org/jtc1/sc22/wg21/docs/papers/2015/p0099r0.pdf

### 2.2. Context-Aware Context Switching (CACS) — PhotonLibOS

PhotonLibOS (ASPLOS 2024 submission) demonstrated that stackful coroutines are not intrinsically slow — they perform poorly mainly because existing implementations do not leverage the fact that control flow is cooperatively transferred. The paper proposes **Context-Aware Context Switching (CACS)**: instead of saving/restoring a full set of callee-saved registers, CACS saves only the minimum necessary set according to the caller and callee context.

The key optimizations:

- **Caller-context-aware saving**: at a yield point, the compiler knows which registers are live. Only those registers need saving. If a yield happens in a tight loop that only uses `rax` and `rcx`, saving `rbx`, `r12`–`r15` is wasted work.
- **Callee-context-aware restoring**: when resuming a coroutine, restore only the registers the callee will actually read before overwriting.
- **Inlined branch prediction**: the yield/resume branch can be inlined at the caller site, giving the branch predictor better context than a single shared trampoline.

The results are striking: CACS made stackful coroutines as fast as or faster than C++20 stackless coroutines on the P1364R0 benchmark — overturning the long-held assumption that stackless is inherently faster. The libfringe project independently explored similar minimal-save optimizations.

The non-obvious implication: the performance difference between stackful and stackless is not fundamental. It is an artifact of implementations that conservatively save all registers. When the compiler has full knowledge of register liveness at yield points — which it always does in a cooperative context — the cost can be reduced to saving only what is actually live. This applies equally to bytecode VMs: a VM that controls its own "register file" (stack frame slots) knows exactly which slots are live at each yield point, enabling CACS-style minimal saves.

Source: https://photonlibos.github.io/blog/stackful-coroutine-made-fast

### 2.3. libfringe — Minimal Safe Context Switches in Rust

libfringe (edef1c) is a Rust library providing safe, lightweight context switches without relying on kernel services. It works in both `std` and `no_std` (bare metal) environments. The design philosophy: provide only context switches (not scheduling), and provide them safely (FIFO discipline — a generator, not arbitrary context transfer).

The x86-64 implementation saves only `rbx`, `rbp`, `r12`–`r15`, and `rsp` — the minimum required by the System V ABI. It does not save floating-point state, SIMD registers, or the x87 control word, since these are caller-saved in System V. The entire switch is ~15 instructions.

libfringe's key contribution is the **safety model**: the `Generator` API enforces that control alternates between the generator and its caller. This is exactly the asymmetric coroutine discipline. The generator yields a value to the caller, and the caller sends a value back when resuming. The type system ensures you cannot resume a dead generator or yield from outside a generator context.

The omission of floating-point/SIMD state saving is safe under System V because these registers are caller-saved — the caller must assume they are clobbered by any function call, including a yield. However, this assumption breaks on Windows x64, where `xmm6`–`xmm15` are callee-saved. Any implementation must be ABI-aware.

Source: https://github.com/edef1c/libfringe

---

## 3. VM-Level Coroutine Implementations

### 3.1. Lua Coroutines — The Reference Implementation

Lua provides the canonical example of asymmetric coroutines in a bytecode VM. The API is minimal: `coroutine.create(f)` wraps a function in a coroutine, `coroutine.resume(co, ...)` runs it, and `coroutine.yield(...)` suspends it. Values flow bidirectionally: arguments to `resume` become return values of `yield`, and arguments to `yield` become return values of `resume`.

Internally, each Lua coroutine has its own Lua stack (an array of `TValue` slots) but shares the C stack with other coroutines. When a coroutine yields:

1. The current Lua instruction pointer and stack state are saved in the coroutine's `lua_State`.
2. Control returns to the `resume` call site via a `longjmp`-like mechanism.
3. No C stack frames are preserved — this is why standard Lua cannot yield across C function boundaries (the infamous "attempt to yield across a C-call boundary" error).

LuaJIT extends this: it can yield from any point, including across C frames, by saving the full C stack state. This is more expensive but more flexible.

The critical design decision: Lua coroutines are **one-shot resumable**. A coroutine that has yielded can be resumed exactly once. It cannot be cloned or forked. This simplifies the implementation enormously — the coroutine's stack is mutable in place, with no need for copy-on-write or snapshot semantics.

A non-obvious subtlety in Lua's implementation: the coroutine's `lua_State` is a full copy of the interpreter's state struct, including the instruction pointer, stack pointer, call info chain, and error handling state. Creating a coroutine allocates a new `lua_State` plus a new Lua stack. The Lua stack default size is `LUA_MINSTACK` (20 slots in PUC-Lua 5.4), and it grows dynamically by reallocation. This means coroutine stacks can move in memory during execution — code must never hold raw C pointers into a Lua coroutine's stack across a call that might trigger stack reallocation (any Lua call, any API call that pushes values).

De Moura, Rodriguez, and Ierusalimschy (2004) showed that Lua's asymmetric coroutines support easy implementations of generators, goal-oriented programming (backtracking), multitasking, and producer/consumer patterns — all without threads or callbacks.

Source: https://lua.org/doc/jucs04.pdf

### 3.2. Cobalt — Coroutines by Bytecode Rewriting

Cobalt (the Lua runtime for CC: Tweaked, a Minecraft mod) solved a challenging problem: implementing coroutines in a Lua VM running on the JVM, where `longjmp` is not available and native coroutines do not exist.

The solution: **automatic state machine transformation of JVM bytecode**. After compilation, each function that might yield is rewritten into a state machine:

1. The function's control flow graph is computed.
2. Each basic block becomes a state in a `switch` statement.
3. At each potential yield point, all live local variables are saved to a `resume` object.
4. When resumed, the `resume` object restores the state and jumps to the correct case.

The transformation is applied automatically to compiled bytecode using ASM (a JVM bytecode manipulation library). The developer writes normal code; the bytecode rewriter handles the rest.

Key insights from Cobalt:

- **The hot path must not pay for yielding.** Cobalt's transform keeps all yield/resume code out of the normal execution path. The transformed code runs within a few percentage points of the original when no yield occurs.
- **Allocation only on yield.** The `resume` object is only created when a yield actually happens, not speculatively.
- **The entire parser/compiler can be made yieldable.** Lua's `load` function accepts a reader callback that may yield. Cobalt's bytecode transform makes the entire Lua parser and compiler resumable, which no other JVM Lua runtime achieves.

Kotlin's `suspend` transformation uses the same principle (CPS + state machine), but Cobalt's benchmarks showed that Kotlin's transformation is 2x slower and allocates 3x more for the pervasive-yield use case, because Kotlin's transform is not optimized for functions where every call might yield.

A subtle correctness issue in bytecode rewriting: the transformation must correctly handle exception handlers (try/catch/finally) that span yield points. If a function has a try block around a yield, the resume must re-enter the try block's exception handler scope. Cobalt handles this by recording the active exception handler set as part of the saved state. Getting this wrong causes exceptions thrown after resume to skip handlers or unwind to the wrong catch block.

Another subtlety: the JVM verifier requires that the types of local variables at a `goto` target match the types at the source. The state machine transformation introduces `goto`-like jumps (via the `switch`) that can violate this constraint. Cobalt resolves this by inserting explicit type-narrowing casts and by carefully managing the variable liveness to satisfy the verifier.

Source: https://squiddev.cc/2023/03/29/coroutines-and-bytecode.html

### 3.3. Kotlin Coroutines — Compiler-Driven State Machines

Kotlin coroutines are the most widely deployed stackless coroutine implementation on the JVM. Every `suspend` function is transformed by the Kotlin compiler into a state machine that takes a `Continuation` parameter.

The transformation:

1. Each suspension point becomes a numbered state.
2. Local variables that are live across suspension points are hoisted into the `Continuation` object.
3. The function body is wrapped in a `when(state)` dispatch.
4. On suspension, the current state is saved and `COROUTINE_SUSPENDED` is returned.
5. On resumption, the dispatcher jumps to the saved state.

The `Continuation` is the CPS (Continuation Passing Style) callback — but rather than creating a new continuation for each call (which would explode the heap), Kotlin reuses a single state machine object per coroutine. This is the key optimization: CPS without allocation.

The JVM has no built-in coroutine support (yet — Project Loom adds virtual threads), so Kotlin's approach is pure compiler transformation. The generated bytecode is normal JVM bytecode that any JVM can run. The cost: every `suspend` call crosses an extra indirection through the state machine dispatch, and local variables that span suspension points are boxed into the continuation object.

A non-obvious performance trap: local variables of primitive types (`int`, `long`, `double`) that are live across a suspension point must be stored in the continuation object as fields. On the JVM, fields of a generic continuation object are typed as `Object`, so primitives get boxed. A tight loop that yields on every iteration and uses `int` loop variables will allocate a boxed `Integer` per iteration per live variable. Kotlin mitigates this by hoisting variables to specialized fields where possible, but the general case still boxes.

The continuation chain is also a linked list of heap objects — each suspended `suspend` call adds a frame to the chain. For deeply nested `suspend` calls, this chain can become long and cause GC pressure. This is the "continuation overhead" that stackful coroutines avoid entirely (they reuse a single contiguous stack).

### 3.4. Small VMs and Coroutines — dziban's Game VM Approach

dziban describes a game VM where hundreds of units each run their own bytecode program, and every tick each unit executes a limited number of instructions based on its virtual CPU speed. This is essentially coroutine-based concurrency: each unit is a coroutine that yields after its instruction budget is exhausted.

The implementation is remarkably simple in a stack-based bytecode VM:

1. Each coroutine has its own virtual stack, instruction pointer, and frame state.
2. `yield` simply saves the instruction pointer and returns control to the scheduler.
3. `resume` restores the instruction pointer and continues execution.
4. The scheduler round-robins through all live coroutines, giving each a tick budget.

The insight from Knuth (The Art of Computer Programming, Vol. 1): a coroutine is just a subroutine that can be suspended and resumed. In a VM that already manages its own call stack, implementing coroutines is almost trivial — the coroutine state IS the VM state (instruction pointer + stack + frame).

What makes this non-obvious: the VM's native dispatch loop (the C or assembly code that fetches, decodes, and executes bytecode instructions) runs on the host's stack. When a coroutine yields, the dispatch loop must return to the host. When resumed, the dispatch loop must be re-entered. If the dispatch loop is a simple `while` loop, this means the entire loop unwinds and re-enters on every yield/resume — which is cheap if the loop has no significant state on the C stack.

However, if the dispatch loop uses native recursion for procedure calls (calling the dispatch function recursively for each bytecode-level call), then yielding from a nested call requires unwinding the entire native recursion. This is the same "yield across C call boundary" problem that Lua has. The solution: either use an explicit VM call stack (not native recursion) for bytecode-level calls, or use native context switching (separate native stacks per coroutine, as in dziban's x64 assembly approach).

Source: https://dziban.net/note/small-vms-and-coroutines

---

## 4. Yielding for Debuggers

### 4.1. debug.js — Generators as a Stepping Debugger

Amjad Masad (creator of Repl.it) built a JavaScript VM and stepping debugger entirely in JavaScript using ES6 generators. The key insight: generators already provide the yield/resume primitive needed for a debugger's step-by-step execution.

The approach:

1. **Code transformation**: every statement in the user's program is preceded by a `yield` expression. The entire program becomes a generator function.
2. **VM stepping**: calling `generator.next()` executes exactly one statement, then yields back to the debugger.
3. **Call stack control**: function calls are wrapped in "thunks" that yield to the VM, which decides whether to step into or step over.
4. **Breakpoints**: before each yield, check if the current source position matches a breakpoint. If so, pause.
5. **Scope inspection**: each generator captures its scope, enabling watch expressions and eval-in-context.

The resulting debugger supports breakpoints, step-in/step-out/step-over, call stack inspection, and scope variable inspection — all implemented in pure JavaScript with no native debugger hooks.

James Long (Unwinder project) extended this approach using full continuations: the program is transformed into continuation-passing style, and the debugger captures and restores continuations at breakpoint locations. This enables not just pausing but also rewinding execution.

A non-obvious issue with the thunk-wrapping approach: every function call in the original program becomes a yield point. This means the VM must distinguish between "yield because this is a function call" and "yield because the debugger asked to pause." debug.js solves this by yielding special objects — thunks for calls, step markers for statements — and the VM loop dispatches based on the yielded object type. This protocol adds overhead per call, which Masad noted was the main performance bottleneck.

Source: https://amasad.me/js-debugger

### 4.2. Instruction Budgets — Yield After N Instructions

A simpler debugger integration: instead of yielding after every instruction, yield after a configurable number of instructions. This gives the debugger periodic control without the overhead of per-instruction checks.

The instruction budget approach:

1. Each coroutine has a `remaining_instructions` counter.
2. The VM loop decrements the counter on each instruction dispatch.
3. When the counter reaches zero, the VM yields with a `BudgetExhausted` reason.
4. The scheduler (or debugger) can then inspect state, check breakpoints, or resume with a new budget.

This is how game VMs (like dziban's) implement fair scheduling, and it naturally extends to debugging: set the budget to 1 for single-stepping, or to a large number for free-running with periodic breakpoint checks.

The cost in a native assembly dispatch loop is one `sub` and one conditional branch (`jz`) per instruction — negligible compared to the instruction itself. However, placing the check in the dispatch hot path means it runs even when no debugger is attached. An alternative: only insert the budget check when debugging is active, either by patching the dispatch loop or by using two separate dispatch loops (one with checks, one without). LuaJIT uses this dual-loop approach for its debug hooks.

A subtlety with instruction budgets: the budget must be checked BEFORE executing an instruction, not after. If checked after, a destructive instruction (e.g., an exit or a jump to invalid bytecode) executes before the debugger can intervene. The budget check must be the first thing in the dispatch loop iteration.

### 4.3. Cooperative Debugger Yields via Yield Instruction

The cleanest debugger integration: add a `Yield` instruction to the bytecode. The compiler inserts `Yield` at debugger-relevant points (statement boundaries, function entries/exits, loop back-edges). The VM executes normally until it hits a `Yield`, at which point it saves its state and returns to the host.

The approach:

1. A `Yield` instruction variant is added to the bytecode scheme.
2. The compiler inserts `Yield` at source-mapped statement boundaries when debug mode is enabled.
3. The VM's dispatch loop handles `Yield` by saving the current instruction pointer offset and returning to the caller.
4. The host (debugger, scheduler, or REPL) inspects the yielded state and decides whether to resume, inspect variables, or terminate.

The `Yield` instruction needs to save minimal state: just the current bytecode offset. If the VM uses offset-based frame addressing (all variable access is `base_pointer + offset`), then no variable relocation is needed — everything is already in the frame.

A non-obvious advantage: when debug mode is disabled, the compiler simply omits the `Yield` instructions. There is zero overhead in production — no budget counter, no conditional branch, no dead code. The bytecode is smaller and the dispatch loop is tighter. This contrasts with the instruction budget approach, which adds overhead even when not debugging.

A non-obvious disadvantage: two different bytecode sequences (debug and release) for the same source. This means the source map offsets differ between debug and release builds. If a debugger needs to attach to a running release build, the bytecode has no yield points. Solutions: always compile with yield points but skip them via a flag check (paying the conditional branch cost), or JIT-patch yield points into running bytecode on debugger attach.

Breakpoint implementation via `Yield` insertion: the debugger identifies the bytecode offset for a source line via the source map, then patches the instruction at that offset to a `Yield` (saving the original instruction). When the `Yield` fires, the debugger restores the original instruction, single-steps it, and re-inserts the `Yield`. This is how native debuggers implement breakpoints (`int 3` on x86), adapted to bytecode.

---

## 5. Concurrency Models Built on Coroutines

### 5.1. Cooperative Multitasking — Round-Robin Schedulers

The simplest concurrency model: a scheduler maintains a queue of coroutines and runs them in round-robin order. Each coroutine runs until it yields, then the scheduler picks the next one.

```
scheduler:
    while queue is not empty:
        coroutine = queue.pop_front()
        result = resume(coroutine)
        if result is Yielded:
            queue.push_back(coroutine)
        else if result is Completed:
            discard coroutine
```

This is how Lua's cooperative multitasking works, how game engines schedule entity scripts, and how Node.js conceptually processes its event queue (with generators/async acting as the yield mechanism).

The advantage over preemptive threads: no locks, no data races, no need for atomic operations. The disadvantage: one coroutine can starve others by not yielding. Instruction budgets (§4.2) solve this by forcing yields.

A non-obvious correctness concern: if coroutines share mutable state, the scheduler's resume order determines the interleaving. Unlike preemptive threads where interleavings are non-deterministic, cooperative coroutines have deterministic interleaving for a given schedule. This is both a strength (reproducible behavior, easier testing) and a weakness (bugs that depend on schedule order are invisible until the scheduler changes).

### 5.2. Async/Await as Coroutine Sugar

Async/await (C#, JavaScript, Python, Rust, Kotlin) is syntactic sugar over coroutines. An `async` function is a coroutine. `await` is a yield point. The runtime scheduler resumes the coroutine when the awaited operation completes.

The correspondence:

| Async/Await     | Coroutine Equivalent          |
|-----------------|-------------------------------|
| `async fn f()` | coroutine `f`                  |
| `await expr`   | `yield` to scheduler           |
| `Promise`      | coroutine handle + result slot |
| Event loop      | cooperative scheduler          |

The insight from "What Color is your Function?" (Bob Nystrom): async/await creates a "function color" problem — async functions can only be called from other async functions. Stackful coroutines avoid this entirely: any function can yield, regardless of its declaration.

A non-obvious implementation detail: async/await systems typically need a "waker" or "reactor" mechanism. When a coroutine yields because it is waiting for I/O, something must wake it up when the I/O completes. This requires an event loop (or epoll/kqueue/io_uring integration) that maps OS readiness notifications back to coroutine handles. The waker mechanism is the complex part — the coroutine yield/resume itself is straightforward. Rust's `Future` trait encodes this explicitly: `poll()` returns `Pending` (yield) with a `Waker` that the I/O system calls to schedule resumption.

### 5.3. Actor-Style Message Passing

Coroutines naturally implement actors: each actor is a coroutine that loops forever, yielding to receive messages and processing them one at a time.

```mage
counter : (procedure Void, Void) {
    count = 0;
    while 1, {
        message = yield void;
        if message == "increment", { count = count + 1; };
        if message == "get", { yield count; };
    };
};
```

The scheduler delivers messages by resuming the actor coroutine with the message as the resume value. The actor processes the message and yields again, waiting for the next one.

This pattern (coroutine + message queue = actor) is used in Lua game scripting, Erlang-style libraries in other languages, and various embedded systems. It provides concurrency without shared mutable state.

A subtlety: the actor above has two yield points with different semantics — one to receive a message and one to send a response. The caller must know which yield is which. In practice, actors typically use a single yield point (receive) and send responses through a separate channel or callback. Mixing "yield to receive" and "yield to send" in the same coroutine creates protocol ambiguity.

---

## 6. Stack Management Strategies

### 6.1. Fixed-Size Stacks

The simplest approach: allocate a fixed-size stack for each coroutine (e.g., 64KB or 1MB). This is what Go 1.0 did (8KB segments, later changed) and what Lua does (default ~1MB).

Advantages: simple allocation, no runtime checks, predictable memory usage per coroutine. Disadvantages: wastes memory if coroutines use little stack, stack overflow if they use more than the fixed size. For thousands of coroutines, the memory waste becomes significant.

### 6.2. Segmented Stacks

Used by Go (before 1.4) and GCC's split-stack feature. Each stack starts small (e.g., 4KB). When a function prologue detects that the stack is about to overflow, it allocates a new segment and chains it to the current one.

The problem: "hot split" — a function at the segment boundary repeatedly allocates and deallocates segments as it calls and returns, causing thrashing. Go abandoned segmented stacks in favor of copying stacks for this reason. The hot-split problem is especially bad for tight loops that call a function just large enough to trigger a new segment.

### 6.3. Copying (Growable) Stacks

Used by Go since 1.4. When the stack needs to grow, allocate a larger stack (2x the current size), copy all frames to the new stack, update all internal pointers, and free the old stack.

This requires that the runtime can identify and update all stack pointers — which is straightforward in Go (the compiler tracks pointer locations via stack maps) and in any VM that uses offset-based frame addressing. If all variable access is `base_pointer + constant_offset`, then relocating the stack is a `memcpy` plus adjusting the single base pointer. No interior pointer fixups are needed.

The non-obvious constraint: **copying stacks are incompatible with raw pointers into the stack.** If any external code holds a pointer to a stack-allocated variable, and the stack is copied to a new location, that pointer becomes dangling. Go solves this by not allowing pointers to stack variables to escape to the heap (escape analysis moves such variables to the heap instead). In a VM, this means the host language (Rust/C) must never hold raw pointers into a coroutine's virtual stack across a call that might trigger stack growth.

### 6.4. Virtual Memory Tricks

Allocate a large virtual address range (e.g., 8MB) but only commit physical pages as needed. The OS handles demand-paging transparently. Stack overflow triggers a page fault, which the runtime can catch and grow the committed region.

This gives the simplicity of fixed-size stacks with the memory efficiency of growable stacks: each coroutine reserves a large virtual range, but the OS only uses physical memory for pages that are actually touched.

The limitation: on 64-bit systems, virtual address space is effectively unlimited, but each mapping consumes kernel resources (VMAs on Linux). Thousands of coroutines with 8MB virtual stacks are fine; millions might hit the `vm.max_map_count` limit (default 65530 on Linux). The limit can be raised, but this is a deployment concern.

A non-obvious detail: **guard pages**. When using virtual memory stacks, a guard page (a page mapped with `PROT_NONE`) should be placed at the bottom of each stack. If a coroutine overflows its stack, it hits the guard page and gets a segfault, rather than silently corrupting adjacent memory. Without guard pages, stack overflow in one coroutine can overwrite another coroutine's stack — a catastrophic and hard-to-debug failure. Both Boost.Context and Go use guard pages.

---

## 7. Yield Value Passing

### 7.1. Unidirectional — Generator Style

The simplest model: yield sends a value out, resume sends nothing in. This is Python's basic generator:

```python
def counter():
    yield 1
    yield 2
    yield 3
```

Sufficient for iterators and lazy sequences. Not sufficient for general coroutine communication.

### 7.2. Bidirectional — Lua/Kotlin Style

Yield sends a value out, resume sends a value in. The yielded value becomes the return value of `resume`, and the resume argument becomes the return value of `yield`:

```lua
co = coroutine.create(function(initial)
    local received = coroutine.yield(initial * 2)
    coroutine.yield(received + 1)
end)

_, v1 = coroutine.resume(co, 10)  -- v1 = 20
_, v2 = coroutine.resume(co, 30)  -- v2 = 31
```

Bidirectional passing enables request/response patterns, cooperative I/O, and debugger variable injection (the debugger can modify values by sending them through resume).

A non-obvious implementation detail: the first `resume` call sends the initial arguments to the coroutine's body function (they become the function's parameters). Subsequent `resume` calls send values that appear as the return value of `yield`. This asymmetry between the first and subsequent resumes is a common source of bugs. Lua handles it by having the first resume pass arguments to the coroutine function, while subsequent resumes pass values that `yield` returns. The coroutine function's parameters and `yield`'s return value occupy different slots.

### 7.3. Multi-Value — Lua's Vararg Yields

Lua allows yielding and resuming with multiple values:

```lua
coroutine.yield(1, 2, 3)           -- yield three values
_, a, b, c = coroutine.resume(co)  -- receive three values
```

In a VM with explicit call area layouts (return value slots, return address slot, argument slots), multi-value yield/resume maps naturally to writing/reading the return value slots. The yield instruction writes to the caller's return slots, and resume reads from them.

---

## 8. Error Handling in Coroutines

### 8.1. Error Propagation on Resume

When a coroutine encounters an error, the error should propagate to the caller on the next `resume`. Lua's `coroutine.resume` returns `(false, error_message)` on error. The coroutine transitions to the "dead" state and cannot be resumed again.

### 8.2. Injecting Errors via Resume

Some systems allow the caller to inject an error into a yielded coroutine: Python's `generator.throw(exception)` causes the yield expression to raise the exception inside the generator. This enables cancellation: the caller can tell a coroutine to clean up and terminate.

Implementation: a `throw` variant of `resume` sets an error flag in the coroutine state before jumping to the saved instruction pointer. The instruction at the yield point checks the flag and branches to error handling.

### 8.3. Resource Safety — The Cancellation Problem

A non-obvious concern: what happens when a coroutine is abandoned (its handle is dropped) while it holds resources (open files, locks, allocated memory)? In Lua, an abandoned coroutine is garbage-collected, but its finalizers run at GC time, not at the point of abandonment. In Rust, a dropped `Future` runs destructors for its captured state, but this only covers state stored in the future object — not state on a separate coroutine stack.

For stackful coroutines, abandonment requires running destructors for all live objects on the coroutine's stack. If the language has RAII or defer-style cleanup, the runtime must unwind the coroutine's stack (calling destructors) when the coroutine handle is dropped. This is equivalent to injecting an uncatchable exception.

The alternative: require coroutines to handle cancellation explicitly. The caller sends a cancellation message via `resume`, and the coroutine exits its loops and returns normally. This is simpler to implement but requires every coroutine to cooperate with the cancellation protocol.

---

## 9. Stackless Coroutines Without Language Support

### 9.1. Protothreads — Coroutines via C Preprocessor Abuse

Simon Tatham (2000) described a technique for implementing coroutines in standard C using the `switch` statement and `__LINE__` macro. The core trick: a `switch` can jump into the middle of a block (including into loops), just like Duff's Device. A coroutine stores its "resume point" as a line number. On re-entry, a `switch` dispatches to that line.

```c
#define crBegin static int state=0; switch(state) { case 0:
#define crReturn(x) do { state=__LINE__; return x; case __LINE__:; } while (0)
#define crFinish }
```

A function wrapped in `crBegin`/`crFinish` becomes a coroutine: calling it repeatedly resumes from the last `crReturn`. The state is a single integer. No stack allocation, no assembly, no platform-specific code.

The critical limitation: **local variables do not survive across yields.** The `switch` re-enters the function body, but C local variables are re-initialized on each function entry. All state that must persist across yields must be lifted to `static` variables or an explicit context struct. This is exactly the same transformation that Kotlin and C++20 coroutines perform automatically — but done by hand.

Adam Dunkels' Protothreads (used in the Contiki embedded OS) package this pattern into a portable library. Protothreads provide `PT_BEGIN`, `PT_WAIT_UNTIL`, `PT_YIELD`, and `PT_END` macros that implement cooperative multitasking on bare-metal systems with as little as 2 bytes of state per protothread (just the line number). They have been deployed on 8-bit microcontrollers where no stack or heap is available.

Russ Cox (2008) noted the connection to Duff's Device: the `switch` fallthrough that makes Duff's loop unrolling work is the same mechanism that makes Tatham's coroutines work. Both exploit the fact that `case` labels in C are just goto targets that can appear inside nested blocks.

A non-obvious portability issue: `__LINE__` must be unique for each `crReturn` in a function. Two `crReturn` calls on the same source line will produce duplicate `case` labels, which is a compile error. This can be worked around with `__COUNTER__` (a GCC/Clang extension) or by ensuring each yield is on a separate line.

Source: https://www.chiark.greenend.org.uk/~sgtatham/coroutines.html

### 9.2. Wren Fibers — VM-Native Cooperative Concurrency

Wren (Bob Nystrom) is a small scripting language (~4000 semicolons of C) where fibers are a first-class part of the execution model, not a library bolted on. Every piece of Wren code runs inside a fiber. The VM's main loop directly operates on a current fiber, and switching fibers is a single pointer swap.

A Wren fiber contains: an array of call frames (each with a function pointer, instruction pointer, and stack window), and a value stack. `Fiber.yield()` saves the current instruction pointer in the current call frame and returns control to the parent fiber. `fiber.call()` saves the caller's state and jumps into the target fiber's saved instruction pointer.

Wren distinguishes two transfer modes:

- **`call`/`yield`**: asymmetric (parent/child). `call` pushes a "caller" pointer; `yield` pops it. This forms a stack of callers.
- **`transfer`**: symmetric. Switches to any fiber without establishing a caller relationship. Used for scheduler-like patterns where fibers are peers.

`Fiber.suspend()` pauses the current fiber and returns control to the C host. This is the mechanism for embedding: the host runs the VM, the VM suspends, the host does work (I/O, frame rendering, etc.), the host resumes the VM. This suspend/resume boundary is where a debugger naturally attaches.

A non-obvious design choice: Wren fibers are used for error handling. There is no try/catch. Instead, a runtime error aborts the current fiber. The caller receives the error as the return value of `call`. If the fiber was `transfer`-ed to (no caller), the error propagates to the host. This unification of error handling and concurrency eliminates the need for separate exception machinery.

Source: https://wren.io/concurrency.html

---

## 10. Scheduler Architectures

### 10.1. Go's GMP Model — M:N Scheduling with Work Stealing

Go's goroutine scheduler is the most sophisticated production coroutine scheduler. It uses an M:N model: M goroutines mapped onto N OS threads. The three entities are:

- **G** (goroutine): the coroutine. Contains a stack, instruction pointer, and status. Starts with a 2KB stack that grows via copying (§6.3).
- **M** (machine): an OS thread. The carrier that actually executes code.
- **P** (processor): a logical processor. Holds a local run queue of ready goroutines. The number of P's equals `GOMAXPROCS` (typically the number of CPU cores).

Scheduling: each P has a local run queue. An M acquires a P, then pops G's from that P's queue to execute. When a G yields (channel operation, I/O, `runtime.Gosched()`), it goes back into a run queue. When a P's local queue is empty, the M **steals** half the G's from another P's queue. This work-stealing algorithm provides automatic load balancing without a central scheduler bottleneck.

A non-obvious detail: **syscall handling**. When a goroutine makes a blocking syscall, the M is blocked in the kernel. The runtime detects this (via the `sysmon` background thread, which runs at 10ms intervals) and **detaches the P** from the blocked M. The P is then handed to a different M (or a new one is created) so the remaining goroutines can continue running. When the syscall returns, the original M tries to reacquire a P. If no P is available, the G is placed on the global run queue and the M goes to sleep.

Preemption: Go 1.14 introduced **asynchronous preemption via signals**. The `sysmon` thread detects goroutines that have been running for >10ms and sends a `SIGURG` signal to the M. The signal handler checks if the goroutine is at a safe point (identified by the compiler via stack maps) and, if so, saves the goroutine's state and yields it. Before 1.14, preemption only happened at function call prologues — a tight loop without function calls (`for { }`) could never be preempted, starving all other goroutines on that P.

A subtle interaction: **stack growth and preemption**. Before 1.14, Go's cooperative preemption checked for stack overflow at function prologues — the same code path was reused for preemption checks. This conflated two concerns (stack growth and scheduling) into a single check, which is elegant but meant that code without function calls (no prologues) was immune to both stack growth and preemption.

Source: https://go.dev/src/runtime/proc.go

### 10.2. Java Virtual Threads — Continuations on the JVM

Java 21 (Project Loom) introduced virtual threads: lightweight threads managed by the JVM rather than the OS. Under the hood, virtual threads are implemented using **continuations** — a captured snapshot of a thread's call stack that can be suspended and resumed.

The mechanism:

1. A virtual thread runs on a **carrier thread** (a platform OS thread from a `ForkJoinPool`).
2. When the virtual thread performs a blocking operation (I/O, `Thread.sleep`, lock acquisition), the JVM **unmounts** the virtual thread: it captures the current continuation (the entire Java stack frame chain) and stores it on the heap.
3. The carrier thread is freed to run other virtual threads.
4. When the blocking operation completes, the JVM **mounts** the virtual thread onto an available carrier thread and resumes the continuation.

The continuation capture is not trivial: the JVM must copy all Java stack frames (local variables, operand stack entries, monitors) from the carrier thread's native stack into a heap-allocated `Continuation` object. On resume, it copies them back. This is essentially a copying-stack approach (§6.3) at the JVM level.

**Pinning**: a virtual thread becomes "pinned" to its carrier thread when it enters a `synchronized` block or calls native (JNI) code. While pinned, the virtual thread cannot be unmounted, and its carrier thread is blocked — reverting to platform thread behavior. This is the key limitation of Project Loom and is being addressed in JEP 491 (JDK 24). The workaround is to use `ReentrantLock` instead of `synchronized`, as `ReentrantLock` was retrofitted to support unmounting.

A non-obvious detail: the continuation capture only saves Java frames, not native frames. If a virtual thread has native frames on the stack (JNI calls), the continuation cannot be captured — the thread is pinned. This is fundamentally the same problem as Lua's "cannot yield across C call boundary," reappearing at the JVM level.

Source: https://openjdk.org/jeps/444

---

## 11. Coroutines as Effect Handler Substrate

### 11.1. One-Shot Algebraic Effects as Coroutines

Kawahara and Kameyama (TFP 2020) showed that one-shot algebraic effects and handlers can be directly embedded in any language with asymmetric coroutines. The key observation: performing an effect is equivalent to yielding a value (the effect operation), and handling the effect is equivalent to resuming the coroutine with the handler's response.

The translation from effects to coroutines:

- `perform(op, arg)` → `yield (op, arg)` — the coroutine yields the effect operation to its handler.
- `handle(computation, handler)` → create a coroutine for the computation, resume it, and when it yields an effect, look up the handler, call it, and resume the coroutine with the handler's result.

They implemented this as libraries in Lua and Ruby — both languages with asymmetric coroutines — demonstrating that no language or runtime modifications are needed. The implementation is remarkably small: ~50 lines in Lua.

This means that a language with coroutines automatically gets one-shot algebraic effects for free. The reverse is not true: coroutines cannot be macro-expressed by one-shot effects (Kobayashi & Kameyama 2025), because coroutines can carry mutable state across yields, while one-shot effects are pure.

Source: https://www.logic.cs.tsukuba.ac.jp/~sat/pdf/tfp2020.pdf

### 11.2. libseff — Effect Handlers for C via Coroutines

Alvarez-Picallo et al. (OOPSLA 2024) introduced libseff, a C library implementing algebraic effect handlers on top of stackful coroutines. Unlike prior effect handler libraries for C (which targeted compiler backends), libseff is designed for direct use by C programmers.

The key design decisions:

- **Mutable coroutines as the representation of pending computations.** Rather than capturing continuations as closures (which requires heap allocation and closure conversion), libseff uses coroutines directly. The coroutine IS the continuation — resuming the continuation means resuming the coroutine.
- **Reified effects instead of closures as handlers.** When an effect is performed, the coroutine yields an effect descriptor (a tagged union of effect operations). The handler inspects the descriptor and decides what to do. This avoids the need for closures in C.
- **Performance competitive with multicore OCaml** (which has native runtime support for effects) on most benchmarks.

The Koka programming language takes a different approach: **evidence-passing** (Xie & Leijen, ICFP 2021). Instead of capturing continuations or using coroutines, Koka passes "evidence" (a pointer to the handler) through the call stack. When an effect is performed, the evidence is used to find the handler, and a "yield bubble" propagates up the stack to unwind to the handler's frame. This avoids both coroutine machinery and continuation capture — at the cost of requiring compiler support for the evidence-passing transformation.

Source: https://homepages.inf.ed.ac.uk/slindley/papers/libseff.pdf

---

## 12. Single-Header C Coroutine Libraries

### 12.1. minicoro — Cross-Platform Stackful Coroutines

minicoro (edubart) is a single-header stackful coroutine library in C, supporting x86-64, ARM, ARM64, RISC-V, and WebAssembly. The API is minimal: `mco_create`, `mco_resume`, `mco_yield`, `mco_destroy`. Each coroutine gets its own stack (user-provided or allocated by the library).

The implementation uses hand-written assembly for context switching on each platform, with a `ucontext` fallback for unsupported architectures. On x86-64, the switch saves `rbx`, `rbp`, `r12`–`r15`, `rsp`, and the return address — the same minimal set as libfringe.

minicoro provides a storage mechanism for passing data between coroutine and caller: `mco_push` writes data into the coroutine's internal buffer, and `mco_pop` reads it. This avoids the need for shared state or global variables for value passing — the coroutine carries its own mailbox.

### 12.2. Tina — Coroutines and Job Systems

Tina (slembcke) is another single-header C coroutine library, but with a twist: it includes a **job system** built on top of the coroutine primitives. A job system is a work-stealing scheduler where tasks (jobs) are coroutines that can yield to wait for dependencies, and a pool of worker threads picks up ready jobs.

Tina supports both symmetric and asymmetric coroutine transfer, and provides `tina_swap` (symmetric switch to a named coroutine) and `tina_yield` (asymmetric return to caller). The job system extension, `tina_jobs.h`, adds `tina_job_wait` (yield until a dependency completes) and `tina_job_switch_queue` (move the current job to a different priority queue).

The non-obvious insight from Tina: symmetric coroutines are needed for the scheduler itself (which switches between arbitrary coroutines), while asymmetric coroutines are the user-facing API (jobs yield to the scheduler, not to each other). The two modes coexist within the same implementation.

Source: https://github.com/edubart/minicoro, https://github.com/slembcke/Tina

### 12.3. libco — Emulator-Grade Cooperative Threading in C89

libco (byuu/higan-emu) is a cooperative threading library written in C89, originally developed for the higan multi-system emulator. Emulators are a perfect use case for coroutines: each emulated processor (CPU, GPU, audio chip) runs as a separate coroutine, and they synchronize by yielding to each other at cycle boundaries.

libco provides symmetric coroutines: `co_switch(target)` transfers control to any named coroutine, not just back to the caller. This is essential for emulators where the CPU coroutine might need to switch directly to the GPU coroutine, not to a central scheduler.

The implementation uses platform-specific assembly for x86, x86-64, ARM, ARM64, and PPC, with fallbacks to `ucontext`, Windows fibers, and `setjmp`/`longjmp`. byuu (the author) benchmarked cooperative context switching at roughly 100x faster than `swapcontext()` and 5x faster than Windows fibers, because the assembly path saves only the minimum register set and avoids kernel crossings entirely.

A non-obvious lesson from libco: the `setjmp`/`longjmp` fallback is significantly slower than the assembly path and has portability hazards (some implementations unwind the stack on `longjmp`, destroying the saved coroutine state). The `ucontext` fallback is even slower because `swapcontext` saves and restores the signal mask via a syscall on every switch (~600ns vs ~6ns for raw assembly). This is why serious coroutine libraries always provide hand-written assembly for supported platforms.

Source: https://github.com/higan-emu/libco

### 12.4. libtask — Russ Cox's Pre-Go Coroutine Library

libtask (Russ Cox, ~2005) is a simple coroutine library for C and Unix that directly influenced the design of Go's goroutines. It runs on Linux (ARM, MIPS, x86), FreeBSD, OS X, and Solaris. The API provides tasks (coroutines), channels (typed message-passing queues), and non-blocking I/O integration.

libtask gives the programmer the illusion of threads, but the OS sees only a single kernel thread. Tasks are cooperative: only one runs at a time, and switching happens only at explicit yield points or blocking I/O operations. The library wraps `read`/`write`/`accept`/`dial` with non-blocking versions that yield the current task when the operation would block, and resume it when the fd becomes ready (via `poll`/`select`).

The channel implementation is notable: `chanrecv` and `chansend` yield the current task if the channel is empty/full, and resume the waiting task when data becomes available. This is exactly Go's channel semantics, implemented four years before Go 1.0.

libtask uses `makecontext`/`swapcontext` on platforms that support it, and hand-written assembly (`asm.S`) on others. Each task gets a heap-allocated stack (default 32KB).

Source: https://swtch.com/libtask/

---

## 13. Hacker Techniques and Unusual Approaches

### 13.1. GNU Pth — The Signal Stack Trick

GNU Pth (Ralf S. Engelschall, USENIX 2000) solved the problem of creating coroutine stacks portably on Unix systems that lack `makecontext`. The technique: abuse `sigaltstack` and signal handlers to bootstrap a new execution context on a fresh stack.

The signal stack trick:

1. Allocate a block of memory for the new coroutine's stack.
2. Register it as a signal stack via `sigaltstack`.
3. Install a signal handler for a specific signal (e.g., `SIGUSR1`) that will become the coroutine's entry point.
4. Raise the signal. The kernel delivers it on the alternate stack.
5. Inside the signal handler, call `setjmp` to save the context (which now has `rsp` pointing into the new stack).
6. Return from the signal handler (restoring the original stack).
7. Later, `longjmp` to the saved context to start the coroutine on its new stack.

This is entirely portable POSIX C — no assembly, no `makecontext`, no platform-specific tricks. GNU Pth used this as a fallback when better mechanisms were unavailable. QEMU also adopted a variant of this technique for its coroutine implementation.

A non-obvious hazard: some OS/compiler combinations cache thread-local storage (TLS) addresses in registers. If a coroutine switch changes the stack pointer but not the TLS base, thread-local variable access produces garbage. This is particularly dangerous on architectures where the TLS base is stored in a dedicated register (e.g., `fs` on x86-64 Linux) that the context switch does not save.

Source: https://www.usenix.org/conference/2000-usenix-annual-technical-conference/portable-multithreading-signal-stack-trick-user

### 13.2. The setjmp/longjmp + alloca Stack Hack

Tony Finch (2010) described coroutines in less than 20 lines of standard C using `setjmp`/`longjmp`. The `coto()` function (coroutine goto) saves the current context with `setjmp`, then `longjmp`s to the target coroutine.

The real trick is **creating a new stack without assembly**. On traditional C implementations, `alloca()` (or C99 variable-length arrays) gives language-level control over the stack pointer. By calling `alloca` with a very large size, you can effectively "move" the stack pointer into a freshly allocated region on the heap — then `setjmp` to save that context. Later, `longjmp` restores the context with the stack pointer in the new region.

This is spectacularly non-portable and technically undefined behavior. Yossi Kreinin (2013) wrote up a cleaner version with an explicit per-coroutine context struct and a `switch`/`case` state machine, noting that the `setjmp` approach "works on every platform I've tried, which is a very different statement from 'it's correct.'"

Dan Luu collected working implementations of coroutines, channels, and message-passing using `setjmp`/`longjmp` and `ucontext` in a single repository, with benchmarks showing raw `setjmp`/`longjmp` switching at ~6ns per switch on x86-64 (compared to ~600ns for `swapcontext` due to the signal mask syscall).

Source: https://fanf.livejournal.com/105413.html, https://github.com/danluu/setjmp-longjmp-ucontext-snippets

### 13.3. node-fibers — Stackful Coroutines Hacked Into V8

Marcel Laverdet's node-fibers (2011) implemented stackful fibers/coroutines for Node.js by patching V8's internals. Since V8 has no coroutine support, node-fibers allocated separate native stacks and used platform-specific context switching (assembly on x86-64, `ucontext` fallback) to swap between the main V8 execution and fiber execution.

The key challenge: V8 (and any JIT-compiled runtime) stores internal state — hidden class pointers, inline caches, compiled code pointers — in ways that assume a single contiguous stack. Switching the stack pointer underneath V8 without its knowledge can corrupt the JIT's assumptions. node-fibers worked by carefully ensuring that V8's internal state was consistent at every yield point (only yielding at safe points where the JIT had no outstanding assumptions).

node-fibers was used by Meteor.js and other frameworks to write synchronous-looking code in Node.js before `async`/`await` existed. The author eventually declared it obsolete (2020) once native `async`/`await` and generators made it unnecessary, noting that maintaining compatibility with V8's ever-changing internals was an unsustainable maintenance burden.

The lesson: bolting stackful coroutines onto a runtime that wasn't designed for them is possible but fragile. Runtimes that want coroutines should build them in from the start (like Lua, Wren, and Go), not retrofit them later.

Source: https://github.com/laverdet/node-fibers

### 13.4. Stack-Safety via Coroutines — Recursive to Iterative for Free

Martin Huschenbett (2021) described a technique to transform any recursive function into an iterative one using generators/coroutines with nearly zero code changes. The idea: instead of making a recursive call, `yield` the arguments to a driver loop. The driver loop maintains an explicit stack of pending continuations and feeds results back via `resume`.

```rust
// Original recursive:
fn fib(n: u64) -> u64 {
    if n < 2 { n } else { fib(n - 1) + fib(n - 2) }
}

// Coroutine version (pseudocode):
fn fib(n: u64) -> Yield<u64> {
    if n < 2 { n } else { (yield (n-1)) + (yield (n-2)) }
}
// Driver loop manages the stack externally
```

This eliminates native stack overflow for deeply recursive functions. The coroutine's state machine replaces native stack frames with heap-allocated state, so recursion depth is limited only by heap memory, not by stack size.

The technique is useful in compilers and interpreters: tree-walking evaluators that recurse on AST nodes can overflow the native stack on deeply nested expressions. Converting the evaluator to a coroutine-based trampoline makes it stack-safe without rewriting the recursive logic.

Source: https://hurryabit.github.io/blog/stack-safety-for-free/

---

## 14. Rust Coroutine Libraries

### 14.1. may — Rust Stackful Coroutines (Go-Style)

may (Xudong Huang, 2017) is a Rust stackful coroutine library designed as a Rust version of Go's goroutines. It provides multi-core scheduling, synchronous-style I/O that is internally non-blocking, and Go-style channels for inter-coroutine communication.

Each coroutine gets its own stack (allocated via the `generator` crate, which uses assembly context switching internally). The runtime includes a work-stealing scheduler that distributes coroutines across OS threads, similar to Go's GMP model. I/O operations (`TcpStream::read`, `TcpStream::write`, etc.) are intercepted and translated to non-blocking operations + coroutine yields, so user code reads as synchronous blocking code but executes asynchronously.

A non-obvious design tension in may: Rust's ownership and borrowing system assumes that references to stack-allocated data have a lifetime tied to the stack frame. When a coroutine yields, its stack is suspended — but any references into that stack that escaped to other coroutines become dangling. may handles this by requiring that data shared between coroutines goes through channels or `Arc`, but the compiler cannot enforce this for all cases. This is a fundamental tension between Rust's stack-based lifetime model and stackful coroutines.

Source: https://github.com/Xudong-Huang/may

### 14.2. genawaiter — Stackless Generators on Stable Rust

genawaiter (whatisaphone) implements stackless generators on stable Rust by repurposing the `async`/`await` machinery. The insight: Rust's `async fn` is already a stackless coroutine (a state machine that can suspend and resume). By wrapping an `async fn` with a custom `Waker` that does nothing, you get a generator — `yield` is implemented as an `await` on a synthetic future that always returns `Pending` on the first poll and captures the yielded value.

genawaiter provides three flavors with different allocation strategies:

- **`stack::Gen`**: allocation-free. The generator state lives on the caller's stack. No `Box`, no `Pin<Box<...>>`. This is the fastest option but requires the generator to be used in a scoped, non-escaping context.
- **`rc::Gen`**: heap-allocated via `Rc`. Can escape the creating scope but is single-threaded.
- **`sync::Gen`**: heap-allocated via `Arc`. Thread-safe.

The `Coroutine` trait unifies all three: `resume_with(arg)` returns `GeneratorState::Yielded(value)` or `GeneratorState::Complete(return_value)`, providing bidirectional value passing.

A non-obvious implication: since genawaiter generators are `async` functions underneath, they can interoperate with the async ecosystem. A generator can `yield` values AND `await` real futures (I/O, timers, etc.) in the same function body. This blurs the line between generators and async streams.

Source: https://github.com/whatisaphone/genawaiter

---

## 15. Game Engine Fiber-Based Job Systems

### 15.1. Naughty Dog — Fibers for The Last of Us Remastered

Christian Gyrling (GDC 2015) described Naughty Dog's fiber-based job system used in The Last of Us Remastered. The PS3-era engine used a traditional job system where jobs always ran to completion — they could never yield. This made it nearly impossible to jobify gameplay code, which has complex control flow with many blocking points (waiting for animation, physics, I/O).

The solution: replace threads with fibers. Each worker thread runs a fiber. When a job needs to wait (for another job to complete, for I/O, for a lock), it yields the fiber — the worker thread picks up a different fiber and continues doing useful work. When the waited-on resource becomes available, the original fiber is added back to the ready queue.

Key implementation details:

- **Fiber pool**: a fixed pool of ~160 fibers, each with a 512KB stack. Fibers are recycled, never created/destroyed at runtime.
- **Wait counters**: a job is associated with a counter. Launching N sub-jobs sets the counter to N. Each sub-job decrements the counter on completion. A job can `WaitForCounter(counter, target_value)` — this yields the fiber and puts it on the counter's wait list.
- **No locks**: since fibers are cooperative, most synchronization is done via wait counters rather than mutexes. Lock-free queues are used for the fiber ready queue.
- **Memory lifetime**: fibers can outlive the frame they were created in (because they might be waiting for something in the next frame). This forced Naughty Dog to rethink memory allocation — they introduced a "tagged heap" where allocations are tagged with a scope, and the scope can be freed atomically when all fibers in that scope complete.

The fiber approach allowed Naughty Dog to achieve near-100% CPU utilization across all cores. The key insight: **the ability to yield mid-function is what makes arbitrary code parallelizable.** Without yield, every job must run to completion, which means complex multi-step gameplay code cannot be a job.

Source: https://www.gdcvault.com/play/1022186/Parallelizing-the-Naughty-Dog-Engine

---

## 16. Summary of Techniques

| Technique | Stack Cost | Switch Cost | Key Trade-off | Examples |
|---|---|---|---|---|
| Full register save/restore | One stack per coroutine | ~10–20ns (x86-64) | Simple but saves unnecessary state | Boost.Context, POSIX ucontext |
| Context-Aware (CACS) | One stack per coroutine | ~5–10ns (x86-64) | Optimal save set, requires compiler support | PhotonLibOS |
| Minimal callee-save only | One stack per coroutine | ~8ns (x86-64) | ABI-specific optimization | libfringe |
| State machine transform | Heap-allocated state | ~1–5ns (indirect call) | No stack allocation, only top-level yields | Kotlin, C++20, Rust async |
| Bytecode rewriting | Heap-allocated state | ~2–10ns (switch dispatch) | Automatic, any function can yield | Cobalt (JVM Lua) |
| Throw/unwind + replay | No extra stack (reuses native) | ~50–100ns (exception path) | Expensive yield, cheap normal path | PUC Lua, some JVM impls |
| Separate thread per coroutine | OS thread stack (1–8MB) | ~1–10μs (kernel context switch) | Simplest, most expensive | LuaJ, early Cobalt fallback |
| Generator/yield transform | Single frame on heap | ~1–3ns (state machine step) | Only single-function generators | Python, JS generators |
| VM instruction budget | Shared VM stack | ~0.5ns (counter decrement) | Fairness without per-instruction yield | Game VMs, embedded scripting |
| Yield instruction in bytecode | Per-coroutine VM stack | ~2–5ns (save IP + stack swap) | Clean, VM-native, debugger-friendly | dziban, Wren |
| Protothreads (switch/case) | 2 bytes (line number) | ~0ns (switch dispatch) | No stack, no locals survive yield | Contiki, embedded systems |
| M:N work-stealing scheduler | Per-G stack + per-P queue | ~100–300ns (full G switch) | Automatic load balancing, syscall handling | Go GMP |
| JVM continuation capture | Heap-copied Java frames | ~500ns–2μs (frame copy) | Transparent to user code, pinning hazard | Java 21 virtual threads |
| Effect handlers via coroutines | Coroutine per handler scope | Coroutine switch cost | Effects for free with coroutines | libseff, Lua/Ruby libraries |
| setjmp/longjmp raw switch | One stack per coroutine | ~6ns (no signal mask) | Portable-ish, technically UB for stack swap | fanf coto(), Dan Luu snippets |
| Signal stack trick (GNU Pth) | One stack per coroutine | ~600ns (includes sigprocmask) | Portable POSIX, no assembly | GNU Pth, QEMU sigaltstack |
| Async/await repurposed as generator | Heap or stack state machine | ~1–3ns (poll + waker) | Zero-cost on stable Rust | genawaiter |
| Fiber pool + wait counters | Fixed pool, recycled stacks | ~10–20ns (fiber switch) | Near-100% CPU utilization, no locks | Naughty Dog, Our Machinery |
| Recursive-to-iterative trampoline | Heap-allocated continuation | ~5–10ns (yield + driver loop) | Stack-safe recursion, minimal code change | Huschenbett technique |

---

| Asymmetric coroutines | Parent/child hierarchy | Predictable control flow | Less flexible than symmetric | Lua, Kotlin, Python |
| Symmetric coroutines | Peer-to-peer transfer | Maximum flexibility | Hard to reason about | Modula-2, Simula |
| One-shot coroutines | No cloning overhead | Mutable state in place | Cannot backtrack | Lua, most implementations |
| Multi-shot (snapshots) | Clone/copy cost | Backtracking, speculation | Memory overhead for snapshots | Prokopec & Liu (ECOOP 2018) |
| Fixed-size stacks | O(N × stack_size) | No runtime checks | Wastes memory for small coroutines | Go 1.0, Lua |
| Copying/growable stacks | O(N × used_size) | Occasional copy cost | Efficient memory, relocation needed | Go 1.4+ |
| Virtual memory stacks | O(N × page_size) for used | Demand-paged by OS | Large virtual range per coroutine | Many game VMs |
| Bidirectional value passing | Two value slots | Full request/response | Slightly more complex protocol | Lua, Kotlin |
| Debugger via yield points | Per-statement yield check | Conditional branch cost | Unified debugger/coroutine mechanism | debug.js, Unwinder |
| Symmetric + asymmetric hybrid | Both modes in one runtime | Same as underlying switch | Scheduler uses symmetric, user uses asymmetric | Wren, Tina |
| Emulator per-chip coroutines | Symmetric switch, per-chip stack | ~6ns (assembly) | Cycle-accurate interleaving | libco (higan) |
| Channel-integrated tasks | Per-task stack + channel queues | Task switch cost + queue ops | CSP-style concurrency | libtask (Russ Cox) |

---

## 17. References

1. Revisiting Coroutines (de Moura & Ierusalimschy, TOPLAS 2009) — https://www.cs.tufts.edu/~nr/cs257/archive/roberto-ierusalimschy/revisiting-coroutines.pdf
2. Coroutines in Lua (de Moura, Rodriguez & Ierusalimschy, 2004) — https://lua.org/doc/jucs04.pdf
3. Theory and Practice of Coroutines with Snapshots (Prokopec & Liu, ECOOP 2018) — https://drops.dagstuhl.de/storage/00lipics/lipics-vol109-ecoop2018/LIPIcs.ECOOP.2018.3/LIPIcs.ECOOP.2018.3.pdf
4. Expressive Power of One-Shot Control Operators and Coroutines (Kobayashi & Kameyama, 2025) — https://arxiv.org/pdf/2509.11901
5. Stackful Coroutine Made Fast — PhotonLibOS (ASPLOS 2024) — https://photonlibos.github.io/blog/stackful-coroutine-made-fast
6. A Low-Level API for Stackful Context Switching — P0099R0 (Kowalke & Goodspeed, WG21 2015) — https://www.open-std.org/jtc1/sc22/wg21/docs/papers/2015/p0099r0.pdf
7. libfringe — Safe Lightweight Context Switches in Rust — https://github.com/edef1c/libfringe
8. deboost.context — Standalone C Context Switching — https://github.com/septag/deboost.context
9. Efficient Coroutines by Rewriting Bytecode — Cobalt/CC:Tweaked (SquidDev, 2023) — https://squiddev.cc/2023/03/29/coroutines-and-bytecode.html
10. Building an In-Browser JavaScript VM and Debugger Using Generators (Amjad Masad, 2014) — https://amasad.me/js-debugger
11. Implementing a Stepping Debugger in JavaScript — Unwinder (James Long, 2016) — https://archive.jlongster.com/Implementing-Stepping-Debugger-JavaScript
12. Small VMs and Coroutines — dziban — https://dziban.net/note/small-vms-and-coroutines
13. Implementation of Lua Coroutines (Ramsey & Cooper, COMP 250RTS 2017) — https://www.cs.tufts.edu/comp/250RTS/handouts/1106coroutines2c.pdf
14. What Color is your Function? (Bob Nystrom) — https://journal.stuffwithstuff.com/2015/02/01/what-color-is-your-function/
15. Building Lightweight Coroutines in Rust: rust-fibers (Sariteke, 2025) — https://medium.com/@ksaritek/building-lightweight-coroutines-in-rust-introducing-rust-fibers-53b91625a9de
16. Anatomy of Coroutines — No Brainer Games (2025) — https://nobrainergames.com/engine/2025/05/01/anatomy-of-coroutines.html
17. Coroutines Examined: Use Cases, Implementation Patterns, and Practical Critique — https://www.der-it-pruefer.de/programming/Coroutines-Examined-Use-Cases-Implementation-Critique
18. How Stackful Coroutines Work — kormang (2025) — https://kormang.github.io/2025/01/26/AC-part9-how-stackful-coroutines-work.html
19. A Field Guide to Lua Coroutines (Scott Vokes, Atomic Object) — https://spin.atomicobject.com/lua-coroutines
20. Bytecode Pattern — Game Programming Patterns (Robert Nystrom) — https://gameprogrammingpatterns.com/bytecode.html
21. Coroutines in C (Simon Tatham, 2000) — https://www.chiark.greenend.org.uk/~sgtatham/coroutines.html
22. On Duff's Device and Coroutines (Russ Cox, 2008) — https://research.swtch.com/duff
23. Protothreads: Lightweight Stackless Threads — Contiki — https://docs.contiki-ng.org/en/develop/_api/group__pt.html
24. Wren Concurrency — Fibers — https://wren.io/concurrency.html
25. Fibers: Coroutines in Finch (Bob Nystrom, 2010) — https://journal.stuffwithstuff.com/2010/07/13/fibers-coroutines-in-finch/
26. JEP 444: Virtual Threads (Java 21) — https://openjdk.org/jeps/444
27. The Basis of Virtual Threads: Continuations — https://foojay.io/today/the-basis-of-virtual-threads-continuations/
28. One-Shot Algebraic Effects as Coroutines (Kawahara & Kameyama, TFP 2020) — https://www.logic.cs.tsukuba.ac.jp/~sat/pdf/tfp2020.pdf
29. Effect Handlers for C via Coroutines — libseff (Alvarez-Picallo et al., OOPSLA 2024) — https://homepages.inf.ed.ac.uk/slindley/papers/libseff.pdf
30. Generalized Evidence Passing for Effect Handlers (Xie & Leijen, ICFP 2021) — https://xnning.github.io/papers/multip.pdf
31. minicoro — Single Header Stackful Coroutines in C — https://github.com/edubart/minicoro
32. Tina — Coroutine and Job Library — https://github.com/slembcke/Tina
33. Ruby 3.0 Fiber Scheduler Interface — https://www.wjwh.eu/posts/2020-12-28-ruby-fiber-scheduler-c-extension.html
34. Go Runtime Scheduler Source — https://go.dev/src/runtime/proc.go
35. Goroutine Preemption in Go — https://go.dev/doc/go1.14
36. libco — Cooperative Threading Library for Emulators (byuu) — https://github.com/higan-emu/libco
37. libtask — Coroutine Library for C and Unix (Russ Cox) — https://swtch.com/libtask/
38. GNU Pth — Portable Multithreading: The Signal Stack Trick (Engelschall, USENIX 2000) — https://www.usenix.org/conference/2000-usenix-annual-technical-conference/portable-multithreading-signal-stack-trick-user
39. Coroutines in less than 20 lines of standard C (Tony Finch, 2010) — https://fanf.livejournal.com/105413.html
40. Coroutines in one page of C (Yossi Kreinin, 2013) — https://yosefk.com/blog/coroutines-in-one-page-of-c.html
41. setjmp-longjmp-ucontext-snippets (Dan Luu) — https://github.com/danluu/setjmp-longjmp-ucontext-snippets
42. node-fibers — Fiber/Coroutine Support for V8 and Node (Marcel Laverdet, 2011) — https://github.com/laverdet/node-fibers
43. Stack-safety for free? (Martin Huschenbett, 2021) — https://hurryabit.github.io/blog/stack-safety-for-free/
44. may — Rust Stackful Coroutine Library (Xudong Huang) — https://github.com/Xudong-Huang/may
45. genawaiter — Stackless Generators on Stable Rust — https://github.com/whatisaphone/genawaiter
46. Parallelizing the Naughty Dog Engine Using Fibers (Christian Gyrling, GDC 2015) — https://www.gdcvault.com/play/1022186/Parallelizing-the-Naughty-Dog-Engine
47. Fiber Based Job System — Our Machinery — https://ruby0x1.github.io/machinery_blog_archive/post/fiber-based-job-system/index.html
48. Let's Write a setjmp (Chris Wellons, 2023) — https://nullprogram.com/blog/2023/02/12/
49. Rust Coroutines on AArch64 — https://monoid.github.io/posts/arm-coroutines/