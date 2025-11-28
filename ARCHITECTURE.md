# Mage JIT Architecture & Module Model

This document describes the current high-level design for the Mage JIT implementation in `mage-rs`, focusing on:

- JIT compilation and execution
- Coroutine-based sandboxing
- Module semantics and import/export
- Code interactions like variables, procedures and syscalls

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

3. **Import/export as the only linkage**  
   - The only thing that cross file boundaries are things we put into export statement.  
   - Imported file should be executed and populate modules export table.  

4. **Scripts sandboxing**   
   - There is **no semantic return value** from a script back to Rust.  

5. **Less rust integration**  
   - Rust does not implement most of variables, procedures and syscalls for Mage code; Rust only handles JIT, memory mappings.  
   - Core libraries (e.g. `core.mg` and its imports) are authored in Mage.  

6. **CLI-defined top-level modules**  
   - Top-level modules are the files listed on the CLI:
     - Example 1: `mage run test.mg` should be processed as one module called `test`.
     - Example 2: `mage run ../mage/core.mg ../mage-introduction-by-examples/001-hello.mg` should be processed as two modules called `core` and `001-hello`.
   - Each top-level module has its own export table.

7. **Nested imports as separate file sandboxes**  
   - A nested import is **not** a separate module.  
     - Example 1: `"core/IO.mg" => import` in `core.mg` should be processed as part of `core` module.
     - Example 2: `"003-import-math.mg" => import` in `003-import.mg` should be processed as part of `003-import` module.
   - It is compiled and executed as its own sandboxed file, but conceptually belongs to the surrounding module.  
   - Each nested file has its own code buffer and coroutine.

---

## 2. Per-File JIT Unit

### 2.1. Bytecode

Each Mage file `F` is a standalone compilation unit:

- Parse + flatten: source → `FlatRoot(F)` (via `flatten_tree`)
- JIT-compile: `FlatRoot(F)` → `Bytecode(F)`

`Bytecode` is intentionally per-file:

- Single code vector
- Offsets for coroutine labels
- Single entry point for that file’s top-level

Conceptually:

- `code: Vec<u8>` — machine code for this **one file**
- `registers_swap: usize` — offset of label that saves the current coroutine register set
- `registers_exit: usize` — offset of label that restores coroutine registers and returns
- `main: usize` — offset of the file’s top-level entry point (script body)

There is **no** multi-file shared buffer. Each file gets its own `Bytecode` and its own mapping.

### 2.2. Runtime Context per File

Execution context passed into compiled code via `Runtime` and `ExportTable` structures.

`Runtime` is **per-file** at execution time: each file’s top-level runs with its own `Runtime` instance.

`ExportTable` is **global** for module at execution time and should be populated with `environment` class with `arguments` field containig command line arguments passed.

---

## 3. Coroutine-Based Execution

The JIT entry has a fixed calling convention:

```text
extern "sysv64" fn(old: &Coroutine, new: &Coroutine, runtime: &mut Runtime, export_table: &mut ExportTable)
```

- `rdi` = `&Coroutine old`
- `rsi` = `&Coroutine new`
- `rdx` = `&mut Runtime`
- `rcx` = `&mut ExportTable`

`Coroutine` wraps a fixed set of callee-save registers and a stack pointer. The JIT-generated prologue and epilogue in each file’s code:

- At `registers_swap`:
  - Saves callee-save registers and `rsp` into `old` (`rdi`).
- At `registers_exit`:
  - Restores callee-save registers and `rsp` from `new` (`rsi`), then `ret`.

### 3.1. When Coroutines Switch

A full coroutine switch (stack swap) occurs only in two specific scenarios:
1. **Rust calling Mage**: Initial entry into a top-level script.
2. **Importing a File**: When File A executes an `import` statement for File B, the runtime pauses A, switches to B's coroutine to run B's top-level script (initialization), and then switches back to A upon completion.

**Note:** Regular function calls between files do **not** trigger a coroutine switch (see Section 4.2).

### 3.2. Per-File Stack and Entry

To execute a file `F`:

1. Map executable memory for `Bytecode(F).code`.
2. Map a stack region (e.g. 64 KiB) flagged as a stack.
3. Initialize a stack pointer such that the first `ret` will jump to `code_ptr + main`.
4. Create:
   - `old` coroutine (zeroed)
   - `new` coroutine, with its last register slot holding the file’s stack pointer
5. Construct a `Runtime` and `ExportTable`.
6. Call the entry function pointer at `code_ptr` with `(old, new, &mut runtime &mut export_table)`.

Each file’s top-level script runs entirely within its own coroutine and stack sandbox.

---

## 4. Import / Export Model

### 4.1. Per-File Exports

Every file can export variables, procedures, and classes via the `export` operator.

Syntax:
```mage
{
    key : value;
    func : myProcedure;
} => export;
```

This operation populates the `ExportTable` for the current module. The keys become available to other files that import this module.

### 4.2. Absolute Cross-File Calls (No Shared Buffer)

When File A calls an exported function from File B, **no coroutine switch occurs**. File A simply executes the machine code of the function in File B using File A's current stack.

**Mechanism:**
1. When File B is imported, its top-level script runs (context switch).
2. File B populates the `ExportTable` with its procedures (compiled code addresses or lazy compilation stubs).
3. File A retrieves the procedure from the `ExportTable` and calls it directly.

**Calling Convention:**
Mage uses a custom calling convention inspired by System V AMD64 ABI:

| Register | Usage |
| :--- | :--- |
| `rdi` | `&Coroutine old` (Context save) |
| `rsi` | `&Coroutine new` (Context restore) |
| `rdx` | `&mut Runtime` |
| `rcx` | `&mut ExportTable` |
| `r8` | **Argument Type** |
| `r9` | **Argument Value** (Scalar `u64`/`s64` or Pointer) |

Since `rdi`, `rsi`, `rdx`, and `rcx` are reserved for the runtime context, arguments are passed starting from `r8`.
*Note: This structure implies Mage functions typically accept a single argument (which may be a pointer to a Class/Tuple containing multiple fields).*

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
IO : "core/IO.mg" => import;
```

The design treats:

- `"core/IO.mg"` as **separate file sandbox**, just like any other file.
- Conceptually, they belong to the “core family”, but mechanically:
  - Each is parsed, flattened, compiled, mapped, and executed **separately**.
  - Each has its own `Bytecode`, stack, coroutine.
- `core.mg` imports them in the same way that any other file imports any other file.

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

  1. `core/IO.mg` is compiled & executed (if imported by `core.mg`).
  2. `core/linux.mg` is compiled & executed.
  3. `core.mg` is compiled & executed.
  4. `001-hello.mg` is compiled & executed.

By the time `001-hello.mg` runs, all imported files (`core.mg` and its nested imports) have:

- Completed their top-level initialization scripts in their own coroutines.

---

## 7. Classes

A **Class** in Mage is a data layout schema, similar to a `struct` in Go.

- **No VTable**: Classes do not carry virtual tables or methods directly.
- **Data Layout**: It defines the memory structure of an object.
- **Passing Mechanism**: When a variable or procedure argument is of type `Class`, it is passed as a **pointer** to the data block in memory (passed in `r9`).

Example:
```mage
MyStruct : {
    Field1 : Uint;
    Field2 : String;
} => Class;
```

---

## 8. Procedures and Syscalls

### 8.1. Procedures

A **Procedure** is a hybrid between a Go function and a JavaScript function.

- **Structure**: A Procedure variable holds a pointer to a structure containing:
  - Function Source Text
  - `FlatSource` (Intermediate Representation)
  - Optional `Compiled Code` address
- **Lazy Compilation**: The code for a procedure is not JIT-compiled immediately upon definition. Instead, it is compiled on the **first call**, and the resulting machine code address is cached for subsequent reuse.
- **Invocation**: Uses the calling convention described in Section 4.2.

### 8.2. Syscalls

Syscalls are special procedures created via the `syscall` receiver.

- **Mechanism**: The `=> syscall` operator takes a definition (syscall number and argument layout) and produces a Procedure.
- **Execution**: When this generated procedure is called, it executes the corresponding OS syscall (e.g., Linux syscalls).

Example:
```mage
# Define a procedure 'close' that calls Linux syscall #3
close : {3; {FD : Uint} => Class} => syscall;
```

---

## 9. Summary

The Mage JIT relies on strict per-file isolation for initialization, using coroutines to switch contexts during the `import` phase. Once initialized, modules interact via a global `ExportTable` and standard function calls (using a custom System V-like convention), without the overhead of coroutine switching for every call. Data is structured via lightweight `Class` schemas, and behavior is encapsulated in lazily-compiled `Procedures`.