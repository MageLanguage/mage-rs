# Mage JIT Architecture & Module Model

This document describes the current high-level design for the Mage JIT implementation in `mage-rs`, focusing on:

- JIT compilation and execution
- Coroutine-based sandboxing
- Module and file semantics
- Import / export and the global module cache
- Calling convention and type descriptors
- Classes, procedures, and syscalls
- Runtime environment and variables

It is a design document, not a full language specification, but it aims to be precise enough for implementors and core library authors.

---

## 1. Design Goals

The architecture is based on the following goals and constraints:

1. **Per-file isolation**  
   - Each Mage source file is compiled and executed independently.  
   - Every file has its own:  
     - JIT-compiled code buffer  
     - Coroutine context including  
       - Executable mapping  
       - Stack  

2. **No merged code buffer**  
   - We never merge multiple files into a single machine-code buffer.  
   - There is no “global code segment” shared by multiple Mage files.

3. **Import / export as the only linkage**  
   - The only things that cross file boundaries are values explicitly placed into an `export` statement.  
   - An imported file is executed (its top-level script runs) and populates an export table.  
   - Other files can then import and use values from that export table.

4. **Script sandboxing**   
   - There is **no semantic return value** from a script back to Rust.  
   - Rust observes effects only via:
     - changes to the `ExportTable`,
     - side effects such as syscalls.

5. **Minimal Rust integration**  
   - Rust does not implement most of the semantics for variables, procedures, and syscalls for Mage code.  
   - Rust is responsible for:
     - Parsing, flattening (`FlatRoot`),
     - JIT code generation (`Bytecode`),
     - Executable + stack memory mappings,
     - Managing runtime helpers (import, variable ops, scope stack, procedure compile, syscall wrappers).
   - Core libraries (e.g. `core.mg` and its imports like `core/IO.mg`, `core/linux.mg`) are authored in Mage.

6. **CLI-defined top-level modules**  
   - Top-level modules are the files listed on the CLI:
     - Example:

       ```text
       mage run ../mage/core.mg ../mage-introduction-by-examples/001-hello.mg
       ```

       This should be processed as two CLI modules called:
       - `core` (from `../mage/core.mg`)
       - `001-hello` (from `../mage-introduction-by-examples/001-hello.mg`)
   - The **CLI module name** is the basename of the root file without `.mg`:
     - `../mage/core.mg` → `core`
     - `../mage-introduction-by-examples/001-hello.mg` → `001-hello`
   - Each CLI module has:
     - its own `Runtime` instance,
     - its own `variables` map and `scope_stack`,
     - its own `ExportTable`,
     - its own stack and coroutine for its top-level script.

7. **Nested imports as separate file sandboxes**  
   - A nested import is **not** a separate CLI module.  
     - Example 1: `"core/IO.mg" => import` in `core.mg` is a nested file in the `core` “module family”.
     - Example 2: `"003-import-math.mg" => import` in `003-import.mg` is a nested file in the `003-import` example family.
   - Each imported file is compiled and executed as its own **file sandbox**:
     - Its own `Bytecode`
     - Its own stack and coroutine
     - Its own `Runtime` and `ExportTable`
   - Conceptually, these files may belong to the same “module family” (e.g. `core`, `003-import`), but mechanically they are independent JIT units connected by imports / exports.

---

## 2. Per-File JIT Unit

### 2.1. From source to bytecode

Each Mage file `F` is a standalone compilation unit:

- Parse + flatten:  
  `source(F)` → `FlatRoot(F)` (via `flatten_tree`).
- JIT-compile:  
  `FlatRoot(F)` → `Bytecode(F)`.

`Bytecode` is intentionally per-file and contains:

- `code: Vec<u8>` — machine code for this **one file**.
- `registers_swap: usize` — offset of label that saves the current coroutine register set.
- `registers_exit: usize` — offset of label that restores coroutine registers and returns.
- `main: usize` — offset of the file’s top-level entry point (script body).
- `root: FlatRoot` — flattened representation retained to support lazy compilation of procedures defined in this file.

There is **no** multi-file shared buffer. Each file gets its own `Bytecode` and its own executable mapping.

### 2.2. Runtime context per file

Execution context is passed into compiled code via two main structures:

- `Runtime` — per-file runtime state.
- `ExportTable` — per-file table of exported values.

For each file execution:

- `Runtime` holds:
  - current variables map,
  - scope stack,
  - pointers to runtime helper functions (set/get var, import, get/set member, make/compile procedure, syscall helpers, etc.),
  - a pointer to the `Mage` instance for compilation,
  - a pointer to the `FlatRoot` used for lazy compilation,
  - a pointer to the global `modules` cache.
- `ExportTable` holds:
  - `values: HashMap<String, usize>`; the public API of this file.

Most variables are **per-file**:

- Local variables and lexical scopes live inside `Runtime.variables` and `Runtime.scope_stack`.
- Only values explicitly inserted into the `ExportTable` become visible across files.

The host runtime keeps a **global module cache**:

- `modules: HashMap<String, usize>` maps a string “module key” to a pointer to that file’s `ExportTable`.
- The key is the resolved import string (details in Section 4.3).

---

## 3. Coroutine-Based Execution

Each file’s JIT entry uses a fixed System V–compatible calling convention:

```text
extern "sysv64" fn(old: &Coroutine, new: &Coroutine, runtime: &mut Runtime, export_table: &mut ExportTable)
```

Register usage at entry:

- `rdi` = `&Coroutine old`
- `rsi` = `&Coroutine new`
- `rdx` = `&mut Runtime`
- `rcx` = `&mut ExportTable`

`Coroutine` wraps a fixed set of callee-save registers and a stack pointer. The JIT-generated prologue and epilogue in each file’s code contain two important labels:

- At `registers_swap`:
  - Saves callee-save registers and `rsp` into `old` (`rdi`).
- At `registers_exit`:
  - Restores callee-save registers and `rsp` from `new` (`rsi`), then `ret`.

This matches System V AMD64 ABI in spirit: the same callee-save register set is preserved, but the JIT manages them explicitly through the `Coroutine` structure.

### 3.1. When coroutines switch

A full coroutine switch (stack swap) occurs only in these scenarios:

1. **Rust calling Mage (top-level entry)**  
   - Rust:
     - allocates executable memory for `Bytecode.code`,
     - allocates a stack mapping for the file,
     - prepares `old` and `new` coroutine objects (`new` holds the initial stack pointer),
     - constructs `Runtime` and a fresh `ExportTable`,
     - calls the entry function at `code_ptr`.
   - Inside JIT code:
     - the prologue at `registers_swap` stores the host registers/stack into `old`,
     - switches to the file’s stack from `new`,
     - runs the top-level script,
     - restores registers/stack from `new` at `registers_exit`,
     - returns to Rust.

2. **Importing another file**  
   - When file A executes an `import` operator for file B:
     - Rust checks the global `modules` cache to see if B has already been executed.
     - If not:
       - it compiles B into `Bytecode(B)`,
       - allocates code/stack for B,
       - constructs `Runtime(B)` and `ExportTable(B)`,
       - enters B via the same coroutine mechanism as in (1),
       - when B completes, its `ExportTable(B)` is boxed and stored in `modules`.
     - If B is already in `modules`, A just reuses the existing export table pointer.
   - After B is initialized, A resumes execution after the `import`.

**Important:**  
Normal Mage-level function/procedure calls (including calls to procedures defined in other files) **do not** cause coroutine or stack swaps. They use the current stack and the Mage calling convention (Section 6).

### 3.2. Per-file stack and lifetime

To execute a file `F`:

1. Map executable memory for `Bytecode(F).code` with JIT permissions.
2. Map a stack region (currently a fixed **64 KiB**) with stack flags.
3. Set up the top of the stack:
   - Place a return address pointing to `code_ptr + main` so that the first `ret` jumps into the top-level script.
4. Construct `old` and `new` coroutines:
   - `old`: zeroed register set.
   - `new`: same as `old` except the last register entry holds the stack pointer for `F`.
5. Prepare `Runtime` and `ExportTable`.
6. Call the file entry function pointer.

**Lifetime:**

- Currently, both code and stack mappings are leaked intentionally for the lifetime of the process.
- Future work: add cleanup logic for code and stack when a module is no longer needed.

---

## 4. Modules, Files, and Import Resolution

Mage distinguishes several related concepts:

- **CLI module**: a file given on the CLI (`mage run ...`).
- **File sandbox**: an individual file compiled and run as described above.
- **Module family**: a conceptual grouping of related files (e.g. `core.mg`, `core/IO.mg`, `core/linux.mg`).
- **Module key**: a string used as a key in the `modules` cache for import resolution.

### 4.1. CLI-defined top-level modules

Example:

```text
mage run ../mage/core.mg ../mage-introduction-by-examples/001-hello.mg
```

- CLI modules:
  - `../mage/core.mg` → CLI module name `core`
  - `../mage-introduction-by-examples/001-hello.mg` → CLI module name `001-hello`
- Each CLI module:
  - is compiled and executed as an independent file sandbox,
  - has its own `Runtime` / `ExportTable` / stack / coroutine,
  - may import further files.

Top-level modules are considered **pseudo-parallel**:

- Their execution order is not semantically significant, except that:
  - if a module imports another, the imported module must be fully initialized before the importer continues;
  - shared imports are **at-most-once** initialized (first importer triggers init, others reuse).

In practice today, they may run sequentially, but the architecture does not require a fixed CLI order beyond respecting import dependencies.

### 4.2. Nested file imports

Nested imports always use paths relative to the directory where the **root module file** is stored.

Examples:

```mage
# in mage/core.mg
IO    : "core/IO.mg"      => import;
linux : "core/linux.mg"   => import;

# in mage-introduction-by-examples/003-import.mg
core : "core"                 => import;
math : "003-import-math.mg"   => import;
```

- `core.mg` lives in the `mage` repository root:
  - `"core/IO.mg"` and `"core/linux.mg"` refer to files in `mage/core/`.
- `003-import.mg` lives in `mage-introduction-by-examples`:
  - `"003-import-math.mg"` refers to a sibling file in that directory.

Each of these import strings is resolved to a file, compiled, and executed as an independent file sandbox, producing its own `ExportTable`.

### 4.3. Import resolution and global module cache

Runtime import is implemented by a Rust helper that:

1. Receives an import string (e.g. `"core/IO.mg"`, `"core"`, `"003-import-math.mg"`).
2. Uses that string (possibly with `.mg` appended) as or to derive a **module key**.
3. Checks `modules: HashMap<String, usize>`:
   - If the key exists:
     - return the cached `ExportTable` pointer.
   - If the key is not present:
     - resolve the string to a file path (using current rules),
     - read the file contents,
     - invoke the compile pipeline (`Mage` + JIT) to produce `Bytecode`,
     - execute the file via `execute_bytecode` to obtain an `ExportTable`,
     - box the `ExportTable` and store its pointer in `modules` under that module key,
     - return the pointer.

**At-most-once initialization:**

- Each module key is initialized only once.
- The first import triggers compilation and execution of the file.
- All later imports of the same key simply retrieve the already-populated `ExportTable`.

The exact normalization of module keys and search roots is an implementation choice, but the **architectural guarantee** is: for a given module key, there is at most one initialization of that file’s top-level script.

---

## 5. Import / Export Model

### 5.1. Exporting from a file

Any file can export variables, procedures, classes, and other values via the `export` operator.

Pattern:

```mage
{
    key1 : value1;
    key2 : value2;
} => export;
```

- Each `keyN` is a string key.
- Each `valueN` is represented at runtime as:
  - a `usize` (often a pointer to some allocated structure or encoded scalar),
  - plus type information carried via type descriptors when called or interpreted.

This operation writes into the file’s `ExportTable` (the instance passed to the JIT entry).

Example from `core.mg`:

```mage
IO    : IO;
linux : linux;
File            : OS.File;
File.read       : OS.File.read;
File.write      : OS.File.write;
File.open       : OS.File.open;
File.openat     : OS.File.openat;
File.close      : OS.File.close;
getStdinReader  : OS.getStdinReader;
getStdoutWriter : OS.getStdoutWriter;
getStderrWriter : OS.getStderrWriter;
} => export;
```

### 5.2. Importing and using exports

When a file imports another:

```mage
core : "core" => import;
```

- The import expression returns a pointer to the imported file’s `ExportTable`.
- Mage code can then use runtime helpers like `get_member` / `set_member` to look up fields or assign new ones.

Calling an exported procedure from another file:

1. Import the module to get its `ExportTable`.
2. Retrieve the procedure value from the table by name (procedure is a pointer to a `Procedure` structure).
3. Pass arguments to that procedure using the Mage calling convention (Section 6).
4. No coroutine switch occurs; the current stack is reused.

---

## 6. Calling Convention and Type Descriptors

Mage uses a custom calling convention inspired by System V AMD64 ABI, with an explicit type descriptor mechanism. The goal is to keep the runtime interface simple (single argument channel, single return channel) while supporting structured data via pointers.

### 6.1. Register usage

At file entry (top-level script):

| Register | Usage                    |
|---------:|--------------------------|
| `rdi`    | `&Coroutine old`         |
| `rsi`    | `&Coroutine new`         |
| `rdx`    | `&mut Runtime`           |
| `rcx`    | `&mut ExportTable`       |

For Mage procedures and syscalls:

| Register | Usage                                                        |
|---------:|--------------------------------------------------------------|
| `r8`     | **Argument Type Descriptor Pointer**                         |
| `r9`     | **Argument Value** (scalar or pointer to Class instance)    |
| `rax`    | **Return Type** (type descriptor or type ID, impl-defined)  |
| `rdx`    | **Return Value** (scalar ≤ 64 bits or pointer)              |

`rdi`, `rsi`, `rdx`, `rcx` remain reserved for runtime context in file entry; Mage-level calls reuse `r8`, `r9`, `rax`, `rdx` for argument/return passing.

### 6.2. Type descriptor structure

Each argument and return value is associated with a **type descriptor**.

- `r8` holds a pointer to a descriptor structure, whose first field is an enum:

  ```text
  kind: Uint
    0 = Void
    1 = Uint
    2 = Sint
    3 = Class
    ... (future kinds)
  ```

- Additional fields in the descriptor are type-specific metadata:
  - For `Void`: no additional fields are required.
  - For `Uint` / `Sint`: may contain bit width or other metadata.
  - For `Class`: may contain a pointer to class layout metadata (names, offsets, sizes).

These descriptors are part of the ABI between:

- the JIT (which generates references to them),
- the runtime and core libraries (which interpret them).

They are typically immutable and may be shared across calls.

### 6.3. Single-argument rule

At the ABI level:

- A procedure receives **at most one argument**:
  - type descriptor pointer in `r8`,
  - value or pointer in `r9`.

If more than one logical argument is needed:

- They must be packaged into a **Class instance** that acts as a struct of arguments.
- A pointer to that instance is passed in `r9`.
- The descriptor in `r8` describes that class.

There is no direct support for multiple positional arguments in registers. This constraint simplifies the call ABI and is central to the design.

### 6.4. Return values

Procedures return via:

- `rax` — the return type (descriptor or type ID; implementation detail),
- `rdx` — the return value or pointer:

  - For numeric scalars ≤ 64 bits: `rdx` holds the integer value.
  - For complex results (Class, String, etc.): `rdx` holds a pointer to an instance.

Void returns:

- Use a type descriptor (or type ID) indicating `Void`.
- `rdx` is ignored.

---

## 7. Classes and Data Layout

### 7.1. Classes

A **Class** in Mage is a data layout schema, conceptually similar to a C struct:

- No virtual methods or vtables are attached by default.
- Fields:

  - appear in memory in the order declared in the source,
  - are aligned and padded according to the target architecture.

On System V AMD64:

- The layout is compatible with typical C struct layout:
  - each field is aligned to its natural alignment,
  - padding inserted as needed,
  - struct size rounded up to the maximum alignment.

Passing rules:

- When a variable or argument has a Class type:
  - the value is passed as a **pointer** in `r9`,
  - the type descriptor in `r8` describes the Class.

Example:

```mage
MyStruct : {
    Field1 : Uint;
    Field2 : String;
} => Class;
```

At runtime, `MyStruct` instances are contiguous blocks of memory with `Field1` (Uint) then `Field2` (String, itself a Class).

### 7.2. Strings

A `String` is a special Class with at least two fields:

1. `ptr` — pointer to UTF-8 bytes.
2. `len` — length in bytes (`Uint`).

A typical in-memory representation on 64-bit platforms:

```text
offset 0: ptr (usize)
offset 8: len (usize)
```

Passing rules:

- The argument type descriptor (`r8`) describes this String Class.
- The argument value (`r9`) points to a String instance.

### 7.3. Numeric literals

Numeric literals support multiple bases:

- `0d` decimal — e.g. `0d60`
- `0x` hex — e.g. `0x3c`
- `0o` octal
- `0b` binary

They are lowered to a numeric representation (`Uint` or `Sint`) as needed, respecting the type descriptors of their use sites.

---

## 8. Procedures and Lazy Compilation

### 8.1. Procedure representation

A **Procedure** is a first-class callable value with lazy compilation:

- In Rust, a `Procedure` contains:
  - `code: usize` — pointer to machine code (0 if not compiled yet),
  - `source_index: usize` — index into `FlatRoot` for this procedure’s source,
  - `root: usize` — pointer to the `FlatRoot` needed for compilation.

Creation:

- `make_procedure(source_index)` allocates a new `Procedure` with:
  - `code = 0`,
  - `source_index` as given,
  - `root` pointing to the file’s `FlatRoot`.

### 8.2. Lazy compilation on first call

When a procedure is invoked:

1. If `procedure.code != 0`:
   - Use it directly as a function pointer.
2. Otherwise:
   - Compile the required portion of `FlatRoot` into machine code (using `compile_lazy`).
   - Map that code into executable memory.
   - Store the entry pointer in `procedure.code`.
   - Call it.

The compiled code is intentionally leaked to keep it alive for the lifetime of the process.

Procedure calls follow the same calling convention as in Section 6.

---

## 9. Syscalls

Syscalls are special procedures created by the `syscall` operator.

### 9.1. Syntax and structure

Syntax:

```mage
{ CONSTANT_NUMBER; ARGUMENT_TYPE } => syscall
```

- `CONSTANT_NUMBER` — OS syscall number (e.g., `0d60` for Linux `exit`).
- `ARGUMENT_TYPE` — a `Class` that describes the syscall’s arguments.

Example (Linux `exit`):

```mage
exit : {0d60; {code : Uint} => Class} => syscall;
{0d1} => exit;
```

Here:

- `{0d60; {code : Uint} => Class} => syscall` creates a syscall procedure:
  - stores the syscall number `0d60`,
  - expects an argument of Class `{code : Uint}`.
- `{0d1}` constructs an instance of that Class with `code = 1`.
- `{0d1} => exit;` passes a pointer to that instance to the syscall wrapper.

### 9.2. Execution of a syscall procedure

At call time:

1. The syscall procedure receives:
   - `r8` — type descriptor pointer for the argument Class,
   - `r9` — pointer to the argument instance in memory.
2. It unpacks fields from the struct at `r9` and loads them into the Linux syscall argument registers:
   - typically `rdi`, `rsi`, `rdx`, `r10`, `r8`, `r9`.
3. It sets `rax` to the syscall number.
4. It executes the `syscall` instruction.
5. Results:
   - For void-like syscalls (e.g. `exit`), no result is propagated back.
   - For others (e.g. `read`, `write`), the return value in `rax` can be wrapped into a Mage value (with a type descriptor in `rax` and a numeric value in `rdx`), depending on how the core library models them.

The current implementation targets Linux x86_64 semantics.

---

## 10. Execution Order and Dependencies

The runtime enforces dependency-based ordering:

- If file `B` imports file `A`, then:
  - A is compiled and executed first,
  - B then resumes after the import with A’s export table available.
- Transitive imports are resolved similarly.

Example:

```text
mage run ../mage/core.mg ../mage-introduction-by-examples/001-hello.mg
```

Assume:

- `001-hello.mg` imports `"core"`.
- `core.mg` imports `"core/IO.mg"` and `"core/linux.mg"`.

Conceptual order:

1. `"core/IO.mg"` → compile & execute (if imported by `core.mg`).
2. `"core/linux.mg"` → compile & execute.
3. `core.mg` → compile & execute.
4. `001-hello.mg` → compile & execute.

By the time `001-hello.mg` runs, `core` and its nested imports are fully initialized and their exports are available.

Multiple CLI modules:

- All share the same `modules` cache.
- If multiple CLI modules import the same child module (same module key):
  - the child is initialized only once,
  - others wait (conceptually) for its completion and then reuse its export table.

---

## 11. Environment, Variables, and Export Tables

### 11.1. Variables and scopes

Each file execution maintains:

- `variables: HashMap<String, usize>` — current scope variables.
- `scope_stack: Vec<*mut HashMap<String, usize>>` — stack of previous scopes.

Runtime helpers:

- `push_scope()`:
  - push `variables` pointer onto `scope_stack`,
  - allocate a new `HashMap` as the current scope.
- `pop_scope() -> ExportTable*`:
  - pop current scope map,
  - wrap it into a new `ExportTable`,
  - restore previous scope from `scope_stack`.

This mechanism supports:

- lexical block scopes,
- constructing export tables from scopes (e.g. `{ ... } => export` or similar constructs that capture current scope).

Most variables exist only within a single file’s `Runtime` and are not shared globally.

### 11.2. Export tables

Each file execution creates an `ExportTable` with:

```rust
pub struct ExportTable {
    pub values: HashMap<String, usize>,
}
```

- `export` statements insert entries into this table by name.
- After execution, this table represents the public API of the file.
- The runtime may:

  - keep this `ExportTable` “owned” by the current process,
  - store a pointer to it in the global `modules` cache under the module key.

Importing code obtains pointers to `ExportTable` instances and can query them using runtime helpers such as `get_member(table_ptr, name)`.

A special exported value is **environment** (planned / architectural goal):

- An `environment` variable in `ExportTable` is a Class instance with at least an `arguments` field describing the CLI arguments passed to the process.
- This is the preferred way for Mage code to access arguments, instead of directly via legacy runtime fields.

---

## 12. Summary

The Mage JIT architecture is built around a few strong constraints:

- **Per-file isolation:** each file is a separate JIT unit with its own code buffer, stack, coroutine, runtime, and export table.
- **Import/export as the only linkage:** files interact exclusively via explicitly exported values stored in `ExportTable`s and retrieved through imports.
- **At-most-once module initialization:** the first import of a given module key compiles and executes that file; subsequent imports reuse its export table.
- **Lightweight ABI for calls:** one argument (type descriptor + value) and one return channel (type + value), with more complex data passed via pointers to Class instances.
- **Classes define memory layout:** class fields follow source order and ABI alignment/padding, making them C-struct-like.
- **Lazy procedure compilation:** procedures are compiled on first use and then cached.
- **Syscalls as procedures:** syscalls are defined in Mage as specialized procedures, bridging structured Mage data to OS-level syscall registers.

This architecture supports core libraries (e.g. `core`, `core/IO.mg`, `core/linux.mg`) that provide higher-level abstractions like `File`, `Reader`, `Writer`, and enables example programs (like `001-hello.mg`, `002-cat.mg`, `003-import.mg`) to be implemented entirely in Mage on top of a minimal and clearly defined JIT + runtime surface.