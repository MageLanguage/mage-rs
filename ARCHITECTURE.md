# Mage JIT Architecture & Module Model

This document describes the high-level design for the Mage JIT implementation in `mage-rs`, focusing on:

- Type system and type descriptors
- JIT compilation and execution
- Coroutine-based sandboxing
- Module and file semantics
- Import / export and the global module cache
- Calling convention and type descriptors
- Classes, interfaces, enums, and procedures
- Control flow and expressions
- Runtime environment and variables

It is a design document, not a full language specification, but it aims to be precise enough for implementors and core library authors.

---

## 1. Type System

Mage is both statically and dynamically typed. Scripts run via `mage run {name}` can be compiled and executed because types of global variables (runtime, environment, etc.) are known at compile time. Procedures defined in scripts are compiled lazily on first call, at which point all required type information is available.

### 1.1. Primitive Types

| Type      | Description                                      |
|-----------|--------------------------------------------------|
| `Void`    | No value                                         |
| `U8`      | Unsigned 8-bit integer                           |
| `U16`     | Unsigned 16-bit integer                          |
| `U32`     | Unsigned 32-bit integer                          |
| `U64`     | Unsigned 64-bit integer                          |
| `S8`      | Signed 8-bit integer                             |
| `S16`     | Signed 16-bit integer                            |
| `S32`     | Signed 32-bit integer                            |
| `S64`     | Signed 64-bit integer                            |
| `Uint`    | Unsigned integer matching target architecture    |
| `Sint`    | Signed integer matching target architecture      |
| `Pointer` | Pointer type (alias for `Uint`, e.g. `U64` on amd64) |

### 1.2. Special Types

| Type      | Description                                      |
|-----------|--------------------------------------------------|
| `Number`  | Abstract numeric literal from source code        |
| `String`  | Source code string (builtin alias for `Slice` with element type `U8`) |
| `Source`  | Static source block reference for lazy compilation |

These special types preserve static data from source files and enable compile-time optimizations.

### 1.3. Composite Types

| Type        | Description                                      |
|-------------|--------------------------------------------------|
| `Class`     | User-defined data structure with named fields    |
| `Interface` | Contract defining required methods               |
| `Enum`      | Named numeric variants extending a base type     |
| `Slice`     | Contiguous sequence of elements with runtime type |

### 1.4. Type Inference

Variable type is determined by the type of its first assigned value:

```mage
x : 0d5;           # x is Number (abstract)
y : 0d5 => U64;    # y is U64 (explicit conversion)
z : "hello";       # z is String
```

When a specific type is needed, send the value to a type constructor via the pipe operator:

```mage
value : 0d255 => U8;
```

**Number type resolution:**

A `Number` remains abstract until it is passed to a sized number constructor. This allows numeric literals to be used polymorphically:

```mage
a : 0d42;              # Number (abstract)
b : a => U8;           # Now U8
c : a => U64;          # Now U64
d : {a; a} => Point;   # Number used in Class construction
```

---

## 2. Type Descriptors

Type descriptors are runtime representations of types that enable dynamic dispatch, type checking, and generic operations.

### 2.1. Type Descriptor Kinds

Type descriptors have a `kind` field as their first element, identifying the type category:

| Kind      | Value | Description                                      |
|-----------|-------|--------------------------------------------------|
| `Void`    | 0     | No value                                         |
| `Uint`    | 1     | Unsigned integer with size metadata              |
| `Sint`    | 2     | Signed integer with size metadata                |
| `Class`   | 3     | Structured type with fields table                |
| `Interface` | 4   | Contract with implementation table               |
| `Enum`    | 5     | Numeric variants with parent type and values     |
| `Number`  | 6     | Abstract numeric literal                         |
| `String`  | 7     | Source code string literal                       |
| `Source`  | 8     | Source block for lazy compilation                |

### 2.2. Type Descriptor Structure

Each type descriptor has kind-specific metadata:

**Void:**
```
struct VoidDescriptor {
    kind: Uint    # 0
}
```

**Uint / Sint:**
```
struct IntDescriptor {
    kind: Uint    # 1 or 2
    size: Uint    # Size in bits (8, 16, 32, 64, or platform-native)
}
```

**Class:**
```
struct ClassDescriptor {
    kind: Uint           # 3
    fields: FieldsTable  # Pointer to fields table
}

struct FieldsTable {
    count: Uint
    fields: [Field]      # Array of field descriptors
}

struct Field {
    name: String
    type: Type           # Pointer to type descriptor
    offset: Uint         # Byte offset in instance
}
```

**Interface:**
```
struct InterfaceDescriptor {
    kind: Uint                    # 4
    methods: MethodsTable         # Required method signatures
    implementations: ImplTable    # Table of implementing classes
}

struct ImplTable {
    count: Uint
    entries: [ImplEntry]
}

struct ImplEntry {
    class_type: Type              # Pointer to Class descriptor
    procedures: ProcedureTable    # Procedures implementing methods
}
```

**Enum:**
```
struct EnumDescriptor {
    kind: Uint           # 5
    parent_type: Type    # Base type (e.g., Uint)
    variants: VariantsTable
}

struct VariantsTable {
    count: Uint
    variants: [Variant]
}

struct Variant {
    name: String
    value: Uint          # Numeric value of variant
}
```

### 2.3. Allocation

Type descriptors are **dynamically allocated** because they can be created during lazy function compilation. Descriptors are allocated on the heap and persist for the lifetime of the process.

### 2.4. Generic Types via Runtime Descriptors

Generic-like behavior is achieved through runtime type descriptors rather than compile-time generics. For example, `Slice` carries its element type at runtime:

```mage
Slice : {
    ptr: Pointer;
    len: Uint;
    type: Type;
} => Class;
```

A `Slice` of strings has `type` pointing to a `String` descriptor. Operations on the slice use this descriptor for element access and type checking.

---

## 3. Builtin Types

### 3.1. Slice

`Slice` is a builtin type representing a contiguous sequence:

```mage
Slice : {
    ptr: Pointer;    # Pointer to first element
    len: Uint;       # Number of elements
    type: Type;      # Element type descriptor
} => Class;
```

### 3.2. String (Runtime)

Runtime `String` is a **builtin alias** for `Slice` where the `type` field is set to `U8`:

```mage
# Conceptually:
String : Slice;  # with type = U8 descriptor
```

This means a runtime string is:
```
struct String {
    ptr: Pointer;    # Pointer to UTF-8 bytes
    len: Uint;       # Length in bytes (not including null terminator)
    type: Type;      # U8 descriptor
}
```

**Null-termination rules:**

- Strings are **not null-terminated** in general (they use ptr + len).
- **Exception 1:** Strings from OS (e.g., `environment.arguments`) are null-terminated, but `len` excludes the null byte.
- **Exception 2:** String literals from source code are null-terminated, but `len` excludes the null byte.

This allows efficient slicing while maintaining C interoperability when needed.

---

## 4. Design Goals

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
   - There is no "global code segment" shared by multiple Mage files.

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
   - **Root modules can only be created from CLI.** There is no other way to create a root module.

7. **Nested imports as separate file sandboxes**  
   - A nested import is **not** a separate CLI module.  
     - Example 1: `"core/IO.mg" => import` in `core.mg` is a nested file in the `core` "module family".
     - Example 2: `"003-import-math.mg" => import` in `003-import.mg` is a nested file in the `003-import` example family.
   - Each imported file is compiled and executed as its own **file sandbox**:
     - Its own `Bytecode`
     - Its own stack and coroutine
     - Its own `Runtime` and `ExportTable`
   - Conceptually, these files may belong to the same "module family" (e.g. `core`, `003-import`), but mechanically they are independent JIT units connected by imports / exports.

---

## 5. Per-File JIT Unit

### 5.1. From source to bytecode

Each Mage file `F` is a standalone compilation unit:

- Parse + flatten:  
  `source(F)` → `FlatRoot(F)` (via `flatten_tree`).
- JIT-compile:  
  `FlatRoot(F)` → `Bytecode(F)`.

`Bytecode` is intentionally per-file and contains:

- `code: Vec<u8>` — machine code for this **one file**.
- `registers_swap: usize` — offset of label that saves the current coroutine register set.
- `registers_exit: usize` — offset of label that restores coroutine registers and returns.
- `main: usize` — offset of the file's top-level entry point (script body).
- `root: FlatRoot` — flattened representation retained to support lazy compilation of procedures defined in this file.

There is **no** multi-file shared buffer. Each file gets its own `Bytecode` and its own executable mapping.

### 5.2. Runtime context per file

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

- `modules: HashMap<String, usize>` maps a string "module key" to a pointer to that file's `ExportTable`.
- The key is the resolved import string (details in Section 7.3).

---

## 6. Coroutine-Based Execution

Each file's JIT entry uses a fixed System V–compatible calling convention:

```text
extern "sysv64" fn(old: &Coroutine, new: &Coroutine, runtime: &mut Runtime, export_table: &mut ExportTable)
```

Register usage at entry:

- `rdi` = `&Coroutine old`
- `rsi` = `&Coroutine new`
- `rdx` = `&mut Runtime`
- `rcx` = `&mut ExportTable`

`Coroutine` wraps a fixed set of callee-save registers and a stack pointer. The JIT-generated prologue and epilogue in each file's code contain two important labels:

- At `registers_swap`:
  - Saves callee-save registers and `rsp` into `old` (`rdi`).
- At `registers_exit`:
  - Restores callee-save registers and `rsp` from `new` (`rsi`), then `ret`.

This matches System V AMD64 ABI in spirit: the same callee-save register set is preserved, but the JIT manages them explicitly through the `Coroutine` structure.

### 6.1. When coroutines switch

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
     - switches to the file's stack from `new`,
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
Normal Mage-level function/procedure calls (including calls to procedures defined in other files) **do not** cause coroutine or stack swaps. They use the current stack and the Mage calling convention (Section 9).

### 6.2. Per-file stack and lifetime

To execute a file `F`:

1. Map executable memory for `Bytecode(F).code` with JIT permissions.
2. Map a stack region (**64 KiB**) with stack flags.
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

## 7. Modules, Files, and Import Resolution

Mage distinguishes several related concepts:

- **CLI module**: a file given on the CLI (`mage run ...`).
- **File sandbox**: an individual file compiled and run as described above.
- **Module family**: a conceptual grouping of related files (e.g. `core.mg`, `core/IO.mg`, `core/linux.mg`).
- **Module key**: a string used as a key in the `modules` cache for import resolution.

### 7.1. CLI-defined top-level modules

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

### 7.2. Nested file imports

Nested imports use paths **relative to the directory of the importing file**.

Examples:

```mage
# in mage/core.mg (located at /path/to/mage/core.mg)
IO    : "core/IO.mg"      => import;   # resolves to /path/to/mage/core/IO.mg
linux : "core/linux.mg"   => import;   # resolves to /path/to/mage/core/linux.mg

# in mage-introduction-by-examples/003-import.mg
core : "core"                 => import;   # CLI module lookup
math : "003-import-math.mg"   => import;   # resolves to same directory
```

**Import resolution algorithm:**

1. Get the directory of the current file.
2. Append the import path to that directory.
3. If the path doesn't end with `.mg`, append `.mg`.
4. Resolve to an absolute path.

Each of these import strings is resolved to a file, compiled, and executed as an independent file sandbox, producing its own `ExportTable`.

### 7.3. Import resolution and global module cache

Runtime import is implemented by a Rust helper that:

1. Receives an import string (e.g. `"core/IO.mg"`, `"core"`, `"003-import-math.mg"`).
2. Uses that string (possibly with `.mg` appended) to derive a **module key**.
3. Checks `modules: HashMap<String, usize>`:
   - If the key exists:
     - return the cached `ExportTable` pointer.
   - If the key is not present:
     - resolve the string to a file path (using current file's directory),
     - read the file contents,
     - invoke the compile pipeline (`Mage` + JIT) to produce `Bytecode`,
     - execute the file via `execute_bytecode` to obtain an `ExportTable`,
     - box the `ExportTable` and store its pointer in `modules` under that module key,
     - return the pointer.

**At-most-once initialization:**

- Each module key is initialized only once.
- The first import triggers compilation and execution of the file.
- All later imports of the same key simply retrieve the already-populated `ExportTable`.

**Circular imports:**

- Circular imports are **not supported**.
- Detection occurs at the first circular `import` call during execution.
- The runtime should track currently-initializing modules and error if a cycle is detected.

**Module reloading:**

- Hot-swapping or reloading modules is not currently supported.

---

## 8. Import / Export Model

### 8.1. Exporting from a file

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

This operation writes into the file's `ExportTable` (the instance passed to the JIT entry).

Example from `core.mg`:

```mage
{
    IO              : IO;
    linux           : linux;
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

### 8.2. Re-exporting

Re-exporting works by **aliasing**. When you write:

```mage
{
    IO : IO;
} => export;
```

The exported `IO` is an alias pointing to the same `ExportTable` as the imported `IO`. No copying occurs.

### 8.3. Importing and using exports

When a file imports another:

```mage
core : "core" => import;
```

- The import expression returns a pointer to the imported file's `ExportTable`.
- Mage code can then use runtime helpers like `get_member` / `set_member` to look up fields or assign new ones.

Calling an exported procedure from another file:

1. Import the module to get its `ExportTable`.
2. Retrieve the procedure value from the table by name (procedure is a pointer to a `Procedure` structure).
3. Pass arguments to that procedure using the Mage calling convention (Section 9).
4. No coroutine switch occurs; the current stack is reused.

---

## 9. Calling Convention and Type Descriptors

Mage uses a custom calling convention inspired by System V AMD64 ABI, with an explicit type descriptor mechanism. The goal is to keep the runtime interface simple (single argument channel, single return channel) while supporting structured data via pointers.

### 9.1. Register usage

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
| `r9`     | **Argument Value** (scalar or pointer to Class instance)     |
| `rax`    | **Return Type** (type descriptor pointer)                    |
| `rdx`    | **Return Value** (scalar ≤ 64 bits or pointer)               |

`rdi`, `rsi`, `rdx`, `rcx` remain reserved for runtime context in file entry; Mage-level calls reuse `r8`, `r9`, `rax`, `rdx` for argument/return passing.

### 9.2. Single-argument rule

At the ABI level:

- A procedure receives **at most one argument**:
  - type descriptor pointer in `r8`,
  - value or pointer in `r9`.

If more than one logical argument is needed:

- They must be packaged into a **Class instance** that acts as a struct of arguments.
- A pointer to that instance is passed in `r9`.
- The descriptor in `r8` describes that class.

There is no direct support for multiple positional arguments in registers. This constraint simplifies the call ABI and is central to the design.

### 9.3. Calling without arguments

When a procedure is called without arguments (e.g., `=> someProc`):

- `r8` (type) should be the `Void` type descriptor
- `r9` (value) should be `0`

```mage
# Calling a no-argument procedure
writer : => core.getStdoutWriter;
```

### 9.4. Return values

Procedures return via:

- `rax` — the return type descriptor pointer,
- `rdx` — the return value or pointer:

  - For numeric scalars ≤ 64 bits: `rdx` holds the integer value.
  - For complex results (Class, String, etc.): `rdx` holds a pointer to an instance.

Void returns:

- `rax` points to a `Void` type descriptor.
- `rdx` is ignored (typically 0).

### 9.5. Argument unpacking in procedures

Procedure arguments are **unpacked at the point of usage**, not at procedure entry.

When a procedure declares:

```mage
add : {
    {a : Uint; b : Uint} => Class;  # argument type
    Uint;                            # return type
    a + b => return;                 # body
} => procedure;
```

The variables `a` and `b` are not immediately extracted into local variables. Instead, when the body references `a`, the runtime reads the value at the appropriate offset from `r9`

This lazy unpacking allows efficient access patterns and avoids unnecessary copying.

---

## 10. Classes

### 10.1. Definition

A **Class** in Mage is a data layout schema, conceptually similar to a C struct:

- No virtual methods or vtables are attached by default.
- Fields:
  - appear in memory in the order declared in the source,
  - are aligned and padded according to System V AMD64 ABI.

```mage
Point : {
    x : Uint;
    y : Uint;
} => Class;
```

### 10.2. Memory layout

On System V AMD64:

- The layout is compatible with typical C struct layout:
  - each field is aligned to its natural alignment,
  - padding inserted as needed,
  - struct size rounded up to the maximum alignment.

**Nested classes are stored as pointers:**

```mage
Line : {
    start : Point;   # Pointer to Point instance
    end : Point;     # Pointer to Point instance
} => Class;
```

This means a `Line` instance contains two pointer-sized fields, not two inline `Point` structures.

### 10.3. Class instance allocation

Currently, class instances are **stack-allocated** via `sub rsp, size`.

Future work may add heap allocation for cases where instances must outlive their creating scope.

### 10.4. Class instance construction

There are two ways to construct a Class instance:

**1. Positional construction (by field order):**

```mage
Point : {x : Uint; y : Uint} => Class;

# Arguments match declaration order: x=10, y=20
p : {0d10; 0d20} => Point;
```

**2. Named construction (any order):**

```mage
Point : {x : Uint; y : Uint} => Class;

# Arguments can be in any order when named
p : {y = 0d20; x = 0d10} => Point;
```

The named form uses the `=` (variable) operator to specify field names explicitly.

### 10.5. Passing rules

When a variable or argument has a Class type:

- the value is passed as a **pointer** in `r9`,
- the type descriptor in `r8` describes the Class.

---

## 11. Interfaces

### 11.1. Definition

An **Interface** defines a contract of required methods. Interfaces use the `method` keyword to describe procedure signatures:

```mage
Reader : {
    read : {
        {reader : Reader; buffer : String} => Class;
        {read : Uint; error : ReaderError} => Class
    } => method;
} => Interface;
```

The `method` keyword takes a source block where:
- First expression is the argument type
- Second expression is the return type

### 11.2. Interface descriptor structure

An interface descriptor contains:

1. **Methods table**: Signatures of required methods
2. **Implementations table**: Per-class mapping of procedures that implement the methods

```
InterfaceDescriptor {
    kind: Uint                    # 4 (Interface)
    methods: MethodsTable
    implementations: ImplTable
}
```

Interface tables are **per-interface** and stored as regular variables.

### 11.3. Implementing an Interface

Interface implementation must be explicit. To implement `Reader` for `File`:

```mage
newFileReader : {
    File;

    read : {
        {file : File; buffer : String} => Class;
        {read : Uint; error : IO.ReaderError} => Class;
        {read = {file; buffer} => File.read} => return;
    } => procedure;
} => IO.Reader;
```

This creates a constructor `newFileReader` that:
1. Takes a `File` argument
2. Returns an `IO.Reader` implementation
3. Registers the `File` class and its `read` procedure in the `IO.Reader` implementation table

### 11.4. Method dispatch

When calling a method through an interface:

```mage
{reader; buffer} => reader.read;
```

The runtime:
1. Gets the actual type of `reader` from its type descriptor
2. Looks up that type in the interface's implementation table
3. Finds the corresponding procedure
4. Calls it with the arguments

---

## 12. Enums

### 12.1. Definition

An **Enum** extends another type with named variants:

```mage
ReaderError : {
    Uint;              # Parent type
    UnsuccessfulRead;  # First variant (value = 0)
} => Enum;
```

### 12.2. Variant numbering

There are two numbering schemes:

**1. Automatic numbering (starting from 0):**

```mage
Creature : {
    Uint;
    Human;   # value = 0
    Elf;     # value = 1
    Dwarf;   # value = 2
} => Enum;
```

**2. Explicit numbering:**

```mage
Creature : {
    Uint;
    Human : 0d20;   # value = 20
    Elf : 0d40;     # value = 40
    Dwarf : 0d60;   # value = 60
} => Enum;
```

### 12.3. Accessing enum variants

When an enum variant is accessed, it produces its numeric value:

```mage
error : ReaderError.UnsuccessfulRead;  # produces 0
creature : Creature.Elf;               # produces 40
```

### 12.4. Enums with associated data

Enums can serve as both simple numeric tags and carriers of associated data, depending on usage context. The parent type determines the underlying representation.

---

## 13. Procedures and Lazy Compilation

### 13.1. Procedure representation

A **Procedure** is a first-class callable value with lazy compilation:

```
struct Procedure {
    code: Pointer       # Pointer to machine code (0 if not compiled yet)
    source_index: Uint  # Index into FlatRoot for this procedure's source
    root: Pointer       # Pointer to the FlatRoot needed for compilation
}
```

Creation:

- `make_procedure(source_index)` allocates a new `Procedure` with:
  - `code = 0`,
  - `source_index` as given,
  - `root` pointing to the file's `FlatRoot`.

### 13.2. Lazy compilation on first call

When a procedure is invoked:

1. If `procedure.code != 0`:
   - Use it directly as a function pointer.
2. Otherwise:
   - Compile the required portion of `FlatRoot` into machine code (using `compile_lazy`).
   - Map that code into executable memory.
   - Store the entry pointer in `procedure.code`.
   - Call it.

The compiled code is intentionally leaked to keep it alive for the lifetime of the process.

### 13.3. Procedure signature

A procedure declaration specifies argument type, return type, and body:

```mage
add : {
    {a : Uint; b : Uint} => Class;  # argument type
    Uint;                            # return type
    a + b => return;                 # body
} => procedure;
```

- **Argument type**: A Class that describes the parameter structure. Arguments are passed as a pointer to an instance of this class.
- **Return type**: The type of value returned by the procedure.
- **Body**: Expressions executed when the procedure is called.

The `return` operator returns a value from the procedure. Return type compatibility is verified the same way as any procedure call verifies type compatibility.

### 13.4. Procedure overloading

Procedure overloading is **not supported**. Defining two procedures with the same name in the same scope is an error:

```mage
# ERROR: duplicate procedure definition
foo : { Uint; Uint; x => return; } => procedure;
foo : { Sint; Sint; x => return; } => procedure;
```

---

## 14. Control Flow

Mage uses operator-based control flow constructs rather than keywords.

### 14.1. Conditionals (if/else)

The `if` operator takes a source block as its only argument:

```mage
{condition; {
    # body executed if condition is true
}} => if;
```

For if-else chains, add additional condition-body pairs. They are processed **sequentially** (like `if-else if-else` in C):

```mage
{
    condition1; {
        # executed if condition1 is true
    };
    condition2; {
        # executed if condition1 is false and condition2 is true
    };
    0d1; {
        # else branch (always true, since 1 is truthy)
    };
} => if;
```

**Boolean representation:**
- `true` = `1` (or any non-zero value)
- `false` = `0`

**Condition requirement:** Conditions **must** be boolean (numeric) values.

### 14.2. Iteration (forRange)

The `forRange` operator iterates over an **Iterable**:

```mage
{
    environment.arguments;  # iterable
    argument;               # loop variable name
    {
        # body executed for each element
    };
} => forRange;
```

**Iterable interface:**

An Iterable is a generic interface with a `next` method that returns the next element:

```mage
Iterable : {
    next : {
        {iterable : Iterable} => Class;
        {value : T; done : Uint} => Class
    } => method;
} => Interface;
```

**Loop variable semantics:**

The loop variable is allocated once, and its value is **overwritten** for each iteration. The same memory address is reused:

```mage
{
    items; item;   # 'item' is allocated once
    {
        # 'item' contains current element, same memory location each iteration
    };
} => forRange;
```

### 14.3. Future Control Flow

- `while` / `for` loops: reserved but not yet implemented
- Pattern matching (`match`): reserved but not yet implemented

---

## 15. Operators and Expressions

### 15.1. Assignment Operators

| Operator | Name     | Description                              |
|----------|----------|------------------------------------------|
| `:`      | Constant | Declares a constant (cannot be reassigned) |
| `=`      | Variable | Declares or reassigns a variable         |

```mage
PI : 0d3;        # constant, cannot be reassigned
count = 0d0;     # variable, can be reassigned
count = 0d1;     # OK: reassignment
PI = 0d4;        # ERROR: cannot reassign constant
```

### 15.2. Arithmetic Operators

| Operator | Description    |
|----------|----------------|
| `+`      | Addition       |
| `-`      | Subtraction    |
| `*`      | Multiplication |
| `/`      | Division       |
| `%`      | Modulo         |

Arithmetic expressions compile directly to machine code.

**Overflow behavior:** Overflows are **ignored** (wrap around), like in C or Go.

**Division by zero:** Behavior matches C (undefined behavior / hardware fault).

### 15.3. Comparison Operators

| Operator | Description           |
|----------|-----------------------|
| `==`     | Equal                 |
| `!=`     | Not equal             |
| `<`      | Less than             |
| `>`      | Greater than          |
| `<=`     | Less than or equal    |
| `>=`     | Greater than or equal |

Comparison results are numeric: `1` for true, `0` for false.

### 15.4. Logical Operators

| Operator | Description |
|----------|-------------|
| `&&`     | Logical AND |
| `\|\|`   | Logical OR  |

Logical operators use **short-circuit evaluation** (like C or Go):
- `&&` evaluates right operand only if left is truthy
- `||` evaluates right operand only if left is falsy

### 15.5. Member Access Operator

The `.` (extract) operator accesses members:

```mage
table.field       # access field from export table or class
module.Procedure  # access exported procedure from module
instance.field    # access class instance field
```

### 15.6. Method Call Syntax (`.method`)

The `.method` syntax checks the **first field's type** of the argument and calls a procedure with that name:

```mage
{file; string} => .read
```

This:
1. Looks at the first field of the argument (`file`)
2. Gets its type (`File`)
3. Calls `File.read` with the full argument

This is equivalent to:
```mage
{file; string} => File.read
```

### 15.7. Prefix Operators

Some operators allow omitting the left operand:

```mage
+5    # means 0 + 5
-5    # means 0 - 5
.foo  # method call syntax (see above)
```

---

## 16. Syscalls

Syscalls are special procedures created by the `syscall` operator.

### 16.1. Syntax and structure

```mage
{ SYSCALL_NUMBER; ARGUMENT_TYPE } => syscall
```

- `SYSCALL_NUMBER` — OS syscall number (e.g., `0d60` for Linux `exit`).
- `ARGUMENT_TYPE` — a `Class` that describes the syscall's arguments.

Example (Linux `exit`):

```mage
exit : {0d60; {code : Uint} => Class} => syscall;
{0d1} => exit;
```

Here:

- `{0d60; {code : Uint} => Class} => syscall` creates a syscall procedure:
  - stores the syscall number `60`,
  - expects an argument of Class `{code : Uint}`.
- `{0d1}` constructs an instance of that Class with `code = 1`.
- `{0d1} => exit;` passes a pointer to that instance to the syscall wrapper.

### 16.2. Execution of a syscall procedure

At call time:

1. The syscall procedure receives:
   - `r8` — type descriptor pointer for the argument Class,
   - `r9` — pointer to the argument instance in memory.
2. It unpacks fields from the struct at `r9` and loads them into the Linux syscall argument registers:
   - `rdi`, `rsi`, `rdx`, `r10`, `r8`, `r9` (in order).
3. It sets `rax` to the syscall number.
4. It executes the `syscall` instruction.
5. Results:
   - For void-like syscalls (e.g. `exit`), no result is propagated back.
   - For others (e.g. `read`, `write`), the return value in `rax` can be wrapped into a Mage value.

The current implementation targets **Linux x86_64** semantics.

---

## 17. Execution Order and Dependencies

The runtime enforces dependency-based ordering:

- If file `B` imports file `A`, then:
  - A is compiled and executed first,
  - B then resumes after the import with A's export table available.
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
  - others reuse its export table.

---

## 18. Environment, Variables, and Export Tables

### 18.1. Variables and scopes

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
- constructing export tables from scopes.

Most variables exist only within a single file's `Runtime` and are not shared globally.

### 18.2. The Environment class

A built-in class `Environment` provides access to runtime information:

```mage
Environment : {
    arguments : Slice;  # CLI arguments (Slice of Strings)
} => Class;
```

The `environment` global variable is an instance of `Environment` available to all scripts. It is initialized like any other value.

```mage
{
    environment.arguments; arg;
    {
        # process each argument
    };
} => forRange;
```

### 18.3. Export tables

Each file execution creates an `ExportTable`:

```
struct ExportTable {
    values: HashMap<String, usize>
}
```

- `export` statements insert entries into this table by name.
- After execution, this table represents the public API of the file.
- The runtime stores a pointer to it in the global `modules` cache under the module key.

Importing code obtains pointers to `ExportTable` instances and can query them using runtime helpers such as `get_member(table_ptr, name)`.

---

## 19. Memory Management

### 19.1. Current model

Memory management in the current implementation:

- **Code and stack mappings**: Leaked intentionally for the lifetime of the process.
- **Stack-allocated values**: Live until the end of the current procedure.
- **Stack size**: Fixed at **64 KiB** per file.
- **Export table values**: Owned by the process and persist for its lifetime.
- **Type descriptors**: Dynamically allocated on the heap, persist for process lifetime.

### 19.2. Allocators

**Current:** Stack allocator is used for Class instances and Strings.

**Future:** Heap allocator will be added for cases where values must outlive their creating scope.

### 19.3. Future work

The following features are planned but not yet implemented:

- **Heap allocator**: For dynamic memory allocation of Class instances and Strings.
- **Garbage collection**: Automatic memory reclamation.
- **RAII-style cleanup**: Deterministic resource cleanup (files, memory, etc.).

---

## 20. Error Handling

### 20.1. Current status

- Errors are represented as **Enum** values (e.g., `ReaderError`).
- No exception-like mechanism exists.

### 20.2. Planned: `try` procedure

A generic `try` procedure will be added that:

1. Takes any Class with an `error` field
2. If `error` is non-zero, calls `return` with the error (propagating it up)
3. Otherwise, continues execution

```mage
# Future syntax
result : {file; buffer} => File.read => try;
```

This provides explicit error propagation similar to Go's error handling pattern.

### 20.3. Undefined behavior

The following currently result in undefined behavior:

- Division by zero
- Integer overflow (wraps silently)
- Null pointer access

---

## 21. Platform Support

### 21.1. Current target

- **Linux x86_64** (amd64): Primary development target.

### 21.2. Future platforms

Potential future support (no timeline):

- FreeBSD x86_64
- macOS x86_64 / ARM64
- Linux ARM64

The coroutine mechanism and syscall interfaces will need platform-specific implementations. Syscalls will use highly optimized assembler code (as currently done for amd64).

---

## 22. Implementation Status

### 22.1. Implemented (basic/prototype)

| Feature                | Status                                      |
|------------------------|---------------------------------------------|
| Parsing (tree-sitter)  | Working                                     |
| Flattening (AST)       | Working                                     |
| JIT compilation        | Basic, syscall/export/import/procedure work |
| Coroutine execution    | Working                                     |
| Module imports         | Basic                                       |
| Export tables          | Working                                     |
| Syscalls               | Working (Linux x86_64)                      |
| Procedures             | Basic lazy compilation                      |
| Classes                | Skeletal                                    |

### 22.2. Not yet implemented

| Feature                | Notes                                       |
|------------------------|---------------------------------------------|
| Arithmetic operators   | Parsing works, JIT codegen needed           |
| Comparison operators   | Parsing works, JIT codegen needed           |
| Logical operators      | Parsing works, JIT codegen needed           |
| Control flow (if)      | Not implemented                             |
| Control flow (forRange)| Not implemented                             |
| Interfaces             | Skeletal                                    |
| Enums                  | Skeletal                                    |
| Type checking          | Not implemented                             |
| Heap allocation        | Not implemented                             |
| String operations      | Not implemented                             |
| Error handling (try)   | Not implemented                             |
| Environment.arguments  | Partially wired                             |
| Language server        | Skeleton exists                             |

### 22.3. Compilation architecture (v1.0.0 target)

The expected compilation architecture for version 1.0.0 is a **per-project compilation server** that:

- Works in sync with the language server and CLI tool
- Maintains project-level cache
- Enables incremental compilation

---

## 23. Summary

The Mage JIT architecture is built around these core principles:

- **Per-file isolation:** each file is a separate JIT unit with its own code buffer, stack, coroutine, runtime, and export table.
- **Import/export as the only linkage:** files interact exclusively via explicitly exported values stored in `ExportTable`s and retrieved through imports.
- **At-most-once module initialization:** the first import of a given module key compiles and executes that file; subsequent imports reuse its export table.
- **Runtime type descriptors:** enable generic-like behavior through dynamic type information rather than compile-time generics.
- **Lightweight ABI for calls:** one argument (type descriptor + value) and one return channel (type + value), with complex data passed via pointers to Class instances.
- **Classes define memory layout:** class fields follow source order and System V ABI alignment/padding.
- **Nested classes as pointers:** Class fields that are themselves Classes store pointers, not inline data.
- **Lazy procedure compilation:** procedures are compiled on first use and then cached.
- **Syscalls as procedures:** syscalls are defined in Mage as specialized procedures, bridging structured Mage data to OS-level syscall registers.

This architecture supports core libraries (e.g. `core`, `core/IO.mg`, `core/linux.mg`) that provide higher-level abstractions like `File`, `Reader`, `Writer`, and enables example programs (like `001-hello.mg`, `002-cat.mg`, `003-import.mg`) to be implemented entirely in Mage on top of a minimal and clearly defined JIT + runtime surface.

---

## Appendix A: Operator Precedence

From highest to lowest precedence:

| Precedence | Operator(s)              | Associativity |
|------------|--------------------------|---------------|
| 7          | `.` (member/extract)     | Left          |
| 6          | `*`, `/`, `%`            | Left          |
| 5          | `+`, `-`                 | Left          |
| 4          | `==`, `!=`, `<`, `>`, `<=`, `>=` | Left  |
| 3          | `&&`                     | Left          |
| 2          | `\|\|`                   | Left          |
| 1          | `=>` (pipe/call)         | Left          |
| 0          | `:`, `=` (assignment)    | Right         |

Parentheses `()` can be used to override precedence.

---

## Appendix B: Reserved Syntax

The following syntax is reserved for future features:

| Syntax     | Intended Use                                    |
|------------|-------------------------------------------------|
| `Struct`   | Value type (like Class but with copy semantics) |
| `Union`    | Tagged union / sum type                         |
| `while`    | While loop                                      |
| `for`      | General for loop                                |
| `match`    | Pattern matching                                |

---

## Appendix C: Numeric Literals

Numeric literals support multiple bases:

| Prefix | Base    | Example  | Decimal Value |
|--------|---------|----------|---------------|
| `0b`   | Binary  | `0b1010` | 10            |
| `0o`   | Octal   | `0o17`   | 15            |
| `0d`   | Decimal | `0d42`   | 42            |
| `0x`   | Hex     | `0x2A`   | 42            |

All numeric literals produce a `Number` type until explicitly converted to a sized type.
