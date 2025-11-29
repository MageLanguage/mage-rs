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
| `Number`  | Static numeric literal from source code          |
| `String`  | UTF-8 string (ptr + len)                         |
| `Source`  | Static source block reference from source code   |

These special types preserve static data from source files and enable compile-time optimizations.

### 1.3. Type Inference

Variable type is determined by the type of its first assigned value:

```mage
x : 0d5;           # x is Number
y : 0d5 => U64;    # y is U64 (explicit conversion)
z : "hello";       # z is String
```

When a specific type is needed, send the value to a type constructor via the pipe operator:

```mage
value : 0d255 => U8;
```

---

## 2. Design Goals

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

## 3. Per-File JIT Unit

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

## 4. Coroutine-Based Execution

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

## 5. Modules, Files, and Import Resolution

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

### 5.3. Import resolution and global module cache

File imports are relative to the current file's directory. If `core.mg` (located at `../mage/core.mg`) imports `"core/linux.mg"`, the runtime:

1. Gets the folder where `core.mg` is stored (`../mage/`)
2. Appends the import path (`core/linux.mg`)
3. Resolves to `../mage/core/linux.mg`


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

**Circular imports:**

- Circular imports are not currently supported and will result in an error.

**Module reloading:**

- Hot-swapping or reloading modules is not currently supported.

---

## 6. Import / Export Model

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

## 7. Calling Convention and Type Descriptors

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

## 8. Classes, Interfaces, and Enums

### 8.1. Classes

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

### 8.2. Interfaces

An **Interface** defines a contract of required methods. Interfaces use the `method` keyword to describe procedure signatures:

```mage
Reader : {
    read : {{reader : Reader; string : String} => Class; {read : Uint; error : ReaderError} => Class} => method;
} => Interface;
```

The `method` keyword is a special constructor that describes procedures required for the interface. It takes a source block where:
- First expression is the argument type
- Second expression is the return type

**Implementing an Interface:**

Interface implementation must be explicit. To implement `Reader` for `File`:

```mage
newFileReader : {
    File;

    read : {
        {file : File; string : String} => Class; {read : Uint; error : IO.ReaderError} => Class;
        {read = {file; string} => .read} => return;
    } => procedure;
} => IO.Reader;
```

This creates a constructor `newFileReader` that takes a `File` and returns an `IO.Reader` implementation.

### 8.3. Enums

An **Enum** extends another type with named variants:

```mage
ReaderError : {
    Uint;
    UnsuccessfulRead;
} => Enum;
```

This defines `ReaderError` as a `Uint` where `0` means `UnsuccessfulRead`. Enums provide semantic names for underlying numeric values.

Access enum variants via member syntax:

```mage
error : ReaderError.UnsuccessfulRead;
```

### 8.4. Strings

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

Strings are currently allocated as:
- **Constant data** for string literals in source code
- **OS-provided data** (e.g., `environment.arguments`)

String concatenation is not yet supported. Future work will add `String` to built-in types with operations like `String.from`.

### 8.5. Numeric literals

Numeric literals support multiple bases:

- `0d` decimal — e.g. `0d60`
- `0x` hex — e.g. `0x3c`
- `0o` octal
- `0b` binary

They are lowered to a numeric representation (`Uint` or `Sint`) as needed, respecting the type descriptors of their use sites.

---

## 9. Control Flow

Mage uses operator-based control flow constructs rather than keywords.

### 9.1. Conditionals (if/else)

The `if` operator takes a source block as its only argument:

```mage
{condition; {
    # body executed if condition is true
}} => if;
```

For if-else chains, add additional condition-body pairs:

```mage
{
    condition1; {
        # executed if condition1 is true
    };
    condition2; {
        # executed if condition1 is false and condition2 is true
    };
    true; {
        # else branch (always true)
    };
} => if;
```

### 9.2. Iteration (forRange)

The `forRange` operator iterates over something iterable:

```mage
{
    environment.arguments;  # iterable
    argument;               # loop variable name
    {
        # body executed for each element
    };
} => forRange;
```

Example from `002-cat.mg`:

```mage
{
    environment.arguments; argument;
    {
        {argument => core.File.open; writer} => core.IO.WriterTo.writeTo
    };
} => forRange;
```

### 9.3. Future Control Flow

- `while` / `for` loops: not yet implemented
- Pattern matching: not yet implemented

---

## 10. Operators and Expressions

### 10.1. Assignment Operators

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

### 10.2. Arithmetic Operators

| Operator | Description    |
|----------|----------------|
| `+`      | Addition       |
| `-`      | Subtraction    |
| `*`      | Multiplication |
| `/`      | Division       |
| `%`      | Modulo         |

Arithmetic expressions compile directly to machine code.

### 10.3. Comparison Operators

| Operator | Description           |
|----------|-----------------------|
| `==`     | Equal                 |
| `!=`     | Not equal             |
| `<`      | Less than             |
| `>`      | Greater than          |
| `<=`     | Less than or equal    |
| `>=`     | Greater than or equal |

### 10.4. Logical Operators

| Operator | Description |
|----------|-------------|
| `&&`     | Logical AND |
| `\|\|`   | Logical OR  |

### 10.5. Member Access Operator

The `.` (extract) operator accesses members:

```mage
table.field       # access field from export table or class
module.Procedure  # access exported procedure from module
instance.field    # access class instance field
```

**Prefix form (`.method`):**

The `.method` syntax is reserved for future use. It will mean "call the method on the implicit receiver from the argument":

```mage
{file; string} => .read   # future: call file.read with string argument
```

### 10.6. Prefix Operators

Some operators allow omitting the left operand:

```mage
+5    # means 0 + 5
-5    # means 0 - 5
.foo  # reserved for future use
```

---

## 11. Procedures and Lazy Compilation

### 11.1. Procedure representation

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

### 11.2. Lazy compilation on first call

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

### 11.3. Procedure signature

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

The `return` operator returns a value from the procedure. Its type must match the declared return type.

### 11.4. Procedure overloading

Procedure overloading is **not supported**. Defining two procedures with the same name is an error:

```mage
# ERROR: duplicate procedure definition
foo : { Uint; Uint; x => return; } => procedure;
foo : { Sint; Sint; x => return; } => procedure;
```

---

## 12. Syscalls

Syscalls are special procedures created by the `syscall` operator.

### 12.1. Syntax and structure

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

### 12.2. Execution of a syscall procedure

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

## 13. Execution Order and Dependencies

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

## 14. Environment, Variables, and Export Tables

### 14.1. Variables and scopes

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

### 14.2. The Environment class

A special built-in class `Environment` provides access to runtime information:

```mage
Environment : {
    arguments : Slice(String);  # CLI arguments passed to the process
} => Class;
```

The `environment` global variable is an instance of `Environment` available to all scripts:

```mage
{
    environment.arguments; arg;
    {
        # process each argument
    };
} => forRange;
```

### 14.3. Export tables

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

## 15. Memory Management

### 15.1. Current model

Memory management in the current implementation:

- **Code and stack mappings**: Leaked intentionally for the lifetime of the process.
- **Stack-allocated values**: Live until the end of the current procedure.
- **Export table values**: Owned by the process and persist for its lifetime.

### 15.2. Future work

The following features are planned but not yet implemented:

- **Heap allocator**: For dynamic memory allocation of Class instances and Strings.
- **Garbage collection**: Automatic memory reclamation.
- **RAII-style cleanup**: Deterministic resource cleanup (files, memory, etc.).

---

## 16. Error Handling

Error handling is not yet implemented. Current status:

- Errors are represented as values (e.g., `ReaderError` enum).
- No exception-like mechanism exists.
- Behavior on division by zero, integer overflow, or null pointer access is undefined.

Future work will define a standard error handling pattern.

---

## 17. Platform Support

### 17.1. Current target

- **Linux x86_64** (amd64): Primary development target.

### 17.2. Future platforms

Potential future support (no timeline):

- FreeBSD x86_64
- macOS x86_64 / ARM64
- Linux ARM64

The coroutine mechanism and syscall interfaces will need platform-specific implementations.

---

## 18. Implementation Status

This section tracks what is implemented vs. planned.

### 18.1. Implemented (basic/prototype)

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

### 18.2. Not yet implemented

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
| Garbage collection     | Not implemented                             |
| String operations      | Not implemented                             |
| Error handling         | Not implemented                             |
| Environment.arguments  | Partially wired                             |
| Language server        | Skeleton exists                             |
| Debugging / source maps| Not implemented                             |

### 18.3. Development approach

- Tests are added during the development process.
- The current Rust code is a prototype to validate the architecture.
- No formal roadmap exists; features are implemented as needed.

---

## 19. Summary

The Mage JIT architecture is built around a few strong constraints:

- **Per-file isolation:** each file is a separate JIT unit with its own code buffer, stack, coroutine, runtime, and export table.
- **Import/export as the only linkage:** files interact exclusively via explicitly exported values stored in `ExportTable`s and retrieved through imports.
- **At-most-once module initialization:** the first import of a given module key compiles and executes that file; subsequent imports reuse its export table.
- **Lightweight ABI for calls:** one argument (type descriptor + value) and one return channel (type + value), with more complex data passed via pointers to Class instances.
- **Classes define memory layout:** class fields follow source order and ABI alignment/padding, making them C-struct-like.
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
| `.method`  | Method call on implicit receiver                |
| `Struct`   | Value type (like Class but with copy semantics) |
| `Union`    | Tagged union / sum type                         |
| `while`    | While loop                                      |
| `for`      | General for loop                                |
| `match`    | Pattern matching                                |