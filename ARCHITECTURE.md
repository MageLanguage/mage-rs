# Mage JIT Architecture & Module Model

This document describes the current high-level design for the Mage JIT implementation in `mage-rs`, focusing on:

- Per-file JIT compilation and execution
- Module and import/export semantics
- Coroutine-based sandboxing
- How core and Linux syscalls fit into the design

The intent is to make the design explicit before deeper implementation work.

---

## 1. Design Goals

The architecture is based on the following goals and constraints:

1. **Per-file isolation**  
   - Each Mage source file is compiled and executed independently.  
   - Every file has its own:
     - JIT-compiled code buffer
     - Executable mapping
     - Stack
     - Coroutine context for its top-level script

2. **No global merged code buffer**  
   - We never merge multiple files into a single machine-code buffer.  
   - Cross-file calls are performed via **absolute addresses** between separate code mappings.

3. **Import/export as the only linkage**  
   - The only things that cross file boundaries are **exported functions/values**.  
   - Importing code simply “takes exported things” from other files via an explicit export table.

4. **Side-effect–only scripts**  
   - Scripts are executed for side effects (IO, syscalls, etc.).  
   - There is **no semantic return value** from a script back to Rust.  
   - Completion of top-level code is the notion of “done”.

5. **Core and Linux bindings remain in Mage**  
   - Core libraries (e.g. `core.mg`, `core/IO.mg`, `core/linux.mg`) are authored in Mage.  
   - Linux syscalls are expressed in these Mage files and JIT-compiled into real `syscall` instructions.  
   - Rust does not implement syscalls for user Mage code; Rust only handles JIT, memory mappings, and orchestration.

6. **CLI-defined top-level modules**  
   - Top-level “modules” are the files listed on the CLI:
     - Example: `mage run ../mage/core.mg ../mage-introduction-by-examples/001-hello.mg`
   - Import resolution respects this: user-visible modules come from the CLI, not implicit disk search.

7. **Nested imports as separate file sandboxes**  
   - A nested import like `import "core/IO.mg"` is **not** a separate module.  
   - It is compiled and executed as its own sandboxed file, but conceptually belongs to the surrounding module (e.g. the “core” family).  
   - Each nested file has its own code buffer and coroutine, and its own exports.

---

## 2. Per-File JIT Unit

### 2.1. Bytecode

Each Mage file `F` is a standalone compilation unit:

- Parse + flatten: source → `FlatRoot(F)` (via `flatten_tree`)
- JIT-compile: `FlatRoot(F)` → `Bytecode(F)`

`Bytecode` is intentionally per-file:

- Single code vector
- Offsets for coroutine helper labels
- Single entry point for that file’s top-level

Conceptually:

- `code: Vec<u8>` — machine code for this **one file**
- `registers_swap: usize` — offset of label that saves the current coroutine register set
- `registers_exit: usize` — offset of label that restores coroutine registers and returns
- `main: usize` — offset of the file’s top-level entry point (script body)

There is **no** multi-file shared buffer. Each file gets its own `Bytecode` and its own mapping.

### 2.2. Runtime Context per File

Execution context passed into compiled code is a small `Runtime` struct. Given that scripts do not return values, it exists primarily to provide:

- Command-line / environment arguments (`environment.arguments`)
- Potentially other shared environment data later

High level:

- `args_ptr: *const Arg`
- `args_len: usize`

`Arg` is a simple `(ptr, len)` pair representing one argument string.

`Runtime` is **per-file** at execution time: each file’s top-level runs with its own `Runtime` instance.

---

## 3. Coroutine-Based Execution

The JIT entry has a fixed calling convention:

```text
extern "sysv64" fn(old: &Coroutine, new: &Coroutine, ctx: &mut Runtime)
```

- `rdi` = `&Coroutine old`
- `rsi` = `&Coroutine new`
- `rdx` = `&mut Runtime`

`Coroutine` wraps a fixed set of callee-save registers and a stack pointer. The JIT-generated prologue and epilogue in each file’s code:

- At `registers_swap`:
  - Saves callee-save registers and `rsp` into `old` (`rdi`).
- At `registers_exit`:
  - Restores callee-save registers and `rsp` from `new` (`rsi`), then `ret`.

### 3.1. Per-File Stack and Entry

To execute a file `F`:

1. Map executable memory for `Bytecode(F).code`.
2. Map a stack region (e.g. 64 KiB) flagged as a stack.
3. Initialize a stack pointer such that the first `ret` will jump to `code_ptr + main`.
4. Create:
   - `old` coroutine (zeroed)
   - `new` coroutine, with its last register slot holding the file’s stack pointer
5. Construct a `Runtime` with args and any environment data.
6. Call the entry function pointer at `code_ptr` with `(old, new, &mut runtime)`.

Each file’s top-level script runs entirely within its own coroutine and stack sandbox.

---

## 4. Import / Export Model

### 4.1. Per-File Exports

Every file can export functions/constants via its Mage `export` semantics. After JIT-compilation and mapping, we build a per-file export table:

- `name: String` — logical export name (`getStdoutWriter`, `IO.Writer.write`, `add`, etc.)
- `kind: ExportKind` — function or constant (extensible later)
- `address: usize` — **absolute** machine code address inside this file’s mapping

The absolute address is computed as:

```text
export.address = code_ptr (mapping base) + label_offset (inside Bytecode.code)
```

The entire per-file module is represented on the Rust side as:

- `FileModule` — logical compiled file:
  - original path
  - `Bytecode`
  - export table (`Vec<ExportEntry>`)
- `LoadedFileModule` — runtime-loaded representation:
  - `FileModule`
  - `code_ptr` / `code_len` for mapped executable memory

### 4.2. Absolute Cross-File Calls (No Shared Buffer)

Imports must **never** cause IR or machine code to be merged into a single buffer. Instead:

1. A file `A` is compiled and executed independently, creating `LoadedFileModule(A)` with an export table.
2. A file `B` imports things from `A` (e.g. `core : "core" => import;`, `math : "003-import-math.mg" => import;`).
3. Before JIT-compiling `B`, the orchestrator:
   - Resolves these imports to concrete `LoadedFileModule`s and `ExportEntry`s.
   - Builds a mapping: **symbol name → absolute code address**.

4. The `Compiler` for `B` receives these absolute addresses in some symbol table.

5. During codegen for `B`, calls to imported functions are compiled into **absolute calls**:

- Either `call [reg]` after loading the address into a register, or
- `call absolute` if the assembler supports it.

No code is copied; `B`’s code simply calls into `A`’s code via absolute addresses.

---

## 5. CLI Modules and Nested File Imports

### 5.1. CLI-Defined Top-Level Modules

The CLI entrypoint defines which files are treated as top-level scripts:

Example:

```bash
mage run ../mage/core.mg ../mage-introduction-by-examples/001-hello.mg
```

This yields two top-level “modules” from the CLI perspective:

- `../mage/core.mg`
- `../mage-introduction-by-examples/001-hello.mg`

Each of these is a **file-level sandbox**:

- Independently parsed, flattened, JIT-compiled, and executed.
- Each has its own code buffer, stack, and coroutine for its top-level script.
- Each exposes its own export table for other files to import from.

### 5.2. Nested File Imports (Same Conceptual Module, Separate Sandboxes)

Inside a file like `core.mg`:

```mage
IO    : "core/IO.mg" => import;
linux : "core/linux.mg" => import;
```

The design treats:

- `"core/IO.mg"` and `"core/linux.mg"` as **separate file sandboxes**, just like any other file.
- Conceptually, they belong to the “core family”, but mechanically:
  - Each is parsed, flattened, compiled, mapped, and executed **separately**.
  - Each has its own `Bytecode`, stack, coroutine, and export table.
- `core.mg` imports them in the same way that any other file imports any other file:
  - Using their exports (functions/constants) via absolute addresses.

This means:

- There is no “single shared core code buffer”.
- Instead, core is a *set* of cooperating file sandboxes bound together by imports and exports.

---

## 6. Execution Order and Dependencies

The runtime respects dependencies between files so that:

- If file `B` imports file `A`, then **A is compiled and executed before B**.
- This extends transitively; nested imports are also satisfied first.

Example:

```bash
mage run ../mage/core.mg ../mage-introduction-by-examples/001-hello.mg
```

- If `001-hello.mg` imports `"core"`, and `core.mg` imports `"core/IO.mg"` and `"core/linux.mg"`, then:

  1. `core/linux.mg` is compiled & executed (if imported by `core.mg`).
  2. `core/IO.mg` is compiled & executed.
  3. `core.mg` is compiled & executed.
  4. `001-hello.mg` is compiled & executed.

By the time `001-hello.mg` runs, all imported files (`core.mg` and its nested imports) have:

- Completed their top-level initialization scripts in their own coroutines.
- Produced valid export tables with callable function addresses.

Imports that refer to an unknown file (i.e., not provided via CLI and not otherwise resolvable by a configured policy) result in an error.

---

## 7. Core, Linux Syscalls, and IO

### 7.1. Syscalls Owned by Core Mage Code

The core and OS binding code lives entirely in Mage source files, e.g.:

- `core.mg`
- `core/IO.mg`
- `core/linux.mg`

These files define:

- Abstract IO concepts:
  - Writers, Readers, Files, etc.
- Concrete Linux bindings:
  - `write`, `open`, `read`, `close`, etc.

The JIT knows how to recognize these syscall-level primitives in the IR and emits:

- Linux x86_64 `syscall` sequences:
  - `rax` = syscall number
  - `rdi`, `rsi`, `rdx`, `r10`, `r8`, `r9` = arguments
  - `syscall`
  - result in `rax`

Rust does not perform target syscalls for Mage code; it only arranges for the Mage JITed code to run.

### 7.2. Example Flow: `001-hello.mg`

Consider:

```mage
core : "core" => import;

{=> core.getStdoutWriter; "Hello world!"} => core.IO.Writer.write;
```

High-level flow:

1. `core` is resolved to one or more core files (`core.mg`, `core/IO.mg`, `core/linux.mg`, etc.).
2. Those files are each:
   - Compiled.
   - Executed in their own coroutines (top-level).
   - Export tables are built with functions like:
     - `getStdoutWriter`
     - `IO.Writer.write`
     - Linux syscall wrappers, etc.
3. `001-hello.mg` is compiled:
   - Imports `core` and associated exports.
   - JIT code for its top-level expression:
     - Calls the exported `core.getStdoutWriter` (absolute address from core’s export table).
     - Uses the returned writer to call `core.IO.Writer.write` with a constant `"Hello world!"` string.
     - `core.IO.Writer.write` eventually calls into a Linux syscall function from `core/linux.mg`.
     - JIT for that Linux binding emits the actual `syscall` instruction.

The top-level script in `001-hello.mg` runs to completion; no value is returned to Rust.

---

## 8. Summary

- **Granularity**:  
  Every file is its own JIT unit with its own code, stack, and coroutine.

- **Linking**:  
  Files are linked only via imports/exports:
  - Imports are resolved in Rust by mapping names to **absolute code addresses** in other files.
  - Exports are per-file and recorded in a table.

- **Isolation**:  
  Files do not share code buffers or stacks; all interaction is via function calls to other sandboxes.

- **Core & syscalls**:  
  Core and OS bindings are plain Mage source files, compiled to machine code; syscalls are implemented in those files and lowered to `syscall` instructions by the JIT.

- **No script-level result**:  
  Scripts are executed for side effects. Completion of top-level code is the end of execution; Rust does not receive a semantic return value.

This architecture is the foundation for making the examples, core libraries, and OS bindings work coherently under the JIT while keeping the system modular, explicit, and debuggable.
