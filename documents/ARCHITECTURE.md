# Architecture

This document describes both the current implementation state and the intended future architecture of Mage.

Mage is being built toward a runtime-first execution model where:

- everything is a variable
- every variable has a type
- files are implicit procedures
- declarations are resolved only when execution reaches them
- only currently entered executable code is compiled
- all executable code is ultimately executed as VM bytecode
- test files are the default automated tooling entrypoints for execution, diagnostics, tracing, and debugging

Where current implementation differs from the target design, that is explicitly marked.

---

## 1. Status overview

### 1.1. Implemented today

The current codebase already contains:

- UTF-8 source parsing
- flat AST representation
- source locations
- AST encoding back to source-like text
- bytecode format
- VM execution
- source maps
- a CLI with `load`, `flatten`, `compile`, and `execute` stages
- a basic language server
- a minimal debug adapter stub

### 1.2. Partially implemented today

The current codebase partially implements or prototypes:

- syntax for the language
- the required direct procedure declaration syntax
- AST encoding for the required direct procedure declaration syntax
- compiler recognition of the required direct procedure declaration shape
- stack-based calling convention
- source-to-bytecode mapping
- LSP diagnostics from parsing
- procedure-definition classification in the language server for the required direct procedure declaration shape
- bytecode reading and pretty printing

### 1.3. Not implemented yet

The following are target architecture features and are not implemented yet:

- runtime type construction
- implicit `main` procedure model for every file
- declaration-time resolution in active runtime
- entered-block-only compilation
- executable values owning bytecode pointers
- import-as-file-call semantics in current runtime
- fully test-driven diagnostics
- omniscient debugging and full tracing
- final `.bytecode` artifact generation model
- native mode compiler

---

## 2. Core principles

### 2.1. Everything is variable

If anything exists in Mage, it should be representable as a variable.

This includes:

- primitive values
- strings
- procedures
- types
- labels
- imported file values
- source blocks
- control flow helpers present in runtime
- comments

Mage intentionally avoids privileged semantic categories where possible.

### 2.2. Every variable has a type

A variable takes its first assigned value type as its type. That type cannot later change.

Mage is both:

- **statically typed** in the sense that every value has a type and the type is immutable for that variable
- **dynamically typed** in the sense that there is no fixed closed set of available types, because types themselves are values and can be constructed at runtime

### 2.3. There are no privileged builtins

Mage should not rely on a special semantic class of “builtins”.

Everything callable is just a variable that exists in runtime.

That includes names such as:

- `procedure`
- `if`
- `while`
- `return`
- `break`
- `continue`
- `import`
- `comment`
- `bytecode`

The default runtime may be populated with these variables at startup, but they remain ordinary variables from the language point of view.

In the current bootstrap semantics, `break` and `continue` are still ordinary callable names, but they are only valid when called with an explicit target label.

Current bootstrap examples:

```mage
break loop_label;
continue loop_label;
```

Unlabeled forms are not part of the current bootstrap semantics.

This is important because later Mage should support different runtime populators. For example:

- a full general-purpose runtime with procedures, control flow, and import
- a restricted runtime without `import`
- a future native-mode populator
- a secure expression-only runtime for sandboxed evaluation

### 2.4. All executable code runs in the VM

Mage does not switch to a permanent AST interpreter for lazy execution.

Even under the future lazy runtime model:

- source code is resolved on demand
- then compiled to bytecode
- then executed in the VM

The VM remains the execution engine.

---

## 3. Type system (Mostly target design)

### 3.1. Primitive types

Primitive types are types that fit into registers on target platforms.

| Type | Behaviour      | Size |
|------|----------------|------|
| U8   | Unsigned       | 8    |
| U16  | Unsigned       | 16   |
| U32  | Unsigned       | 32   |
| U64  | Unsigned       | 64   |
| S8   | Signed         | 8    |
| S16  | Signed         | 16   |
| S32  | Signed         | 32   |
| S64  | Signed         | 64   |
| F32  | Floating-point | 32   |
| F64  | Floating-point | 64   |

### 3.2. Builtin types and constructors

#### Type

`Type` is the foundation of the Mage type system.

```mage
Type : Class {
    ...
};
```

#### Link

`Link` is a special type for referencing memory. Internally it contains data and type pointers.

```mage
Link : Class {
    type_pointer : U64;
    data_pointer : U64;
};
```

#### Class

`Class` is a constructor that takes source and produces a type value.

```mage
Class : procedure Source, ^Type;
```

#### Interface

`Interface` is a constructor that takes source and produces a type value.

```mage
Interface : procedure Source, ^Type;
```

#### Enumeration

`Enumeration` constrains available values of another type.

```mage
Enumeration : procedure Source, ^Type;
```

#### Vector

Sequence of generic elements.

#### String

`String` is a source code string type.

Conceptually it is currently treated as an alias for `Vector` with element type `U8`.

#### Number

`Number` is an abstract numeric literal type.

Conceptually it is currently treated as an alias for `String`.

#### Source

`Source` is definitely a type alias for string with source code.

This is a critical rule.

A source block is representable as source code text. It is valid to store a source block in a variable and later pass that variable to a callable value.

Example:

```mage
text : { x = x + 1 };

x = 0;

if x < 1, text;
```

This means:

- a source block can become a `Source` value
- `Source` is textual source code
- deferred execution and compilation can be driven from stored source text

#### Void

`Void` is the Mage representation of nothing.

It is used as the argument for zero-parameter procedure calls.

### 3.3. Example target syntax

```mage
Counter : Interface {
    add : method {counter : Counter}, Void;
    get : method {counter : Counter}, U64;
};

InMemoryCounter : Class {
    count : U64;
};

newInMemoryCounter : implement InMemoryCounter, Counter, {
    add : procedure {in_memory_counter : ^InMemoryCounter}, Void {
        in_memory_counter.count = in_memory_counter.count + 1;
    };

    get : procedure {in_memory_counter : ^InMemoryCounter}, U64 {
        return in_memory_counter.count;
    };
};

service = Service 0;
```

---

## 4. Procedure model

### 4.1. Required syntax

The required procedure declaration syntax is:

```mage
add : procedure {x : U64; y : U64}, U64 {
    return x + y;
}
```

Call syntax is universal and left-associative.

That means:

```mage
return add 0d3, 0d4;
```

is understood as:

```mage
(return add) 0d3, 0d4;
```

If the intent is to return the result of a call, the call must be grouped explicitly:

```mage
return (add 0d3, 0d4);
```

This is the current required model for procedure declarations and call syntax.

The same universal call rule applies to bootstrap control flow helpers.

That means `break` and `continue` are not treated as special syntax with implicit targets. In the current bootstrap semantics they must be called with an explicit label:

```mage
break loop_label;
continue loop_label;
```

### 4.3. Procedures are variables

A procedure is not a privileged compiler-only declaration.

A procedure is a variable whose value is callable and executable.

A procedure value conceptually owns:

- its procedure type
- a reference to its source code
- its declaration-time environment or runtime context information
- a zero or empty pointer or link to compiled bytecode for its own source block

A procedure value should be created without compiled bytecode.

Its initial state is:

- source reference present
- bytecode link empty

On first execution, if the bytecode link is empty, the procedure’s own source block is compiled and the resulting bytecode link is stored in that same value.

A given executable value should only need to be compiled once. If another runtime situation requires a different compiled result, that should be represented by constructing another value rather than recompiling the same one.

### 4.4. `main` is a procedure

Every file is an implicit `main` procedure.

A file is not a special top-level artifact outside the procedure model.

Conceptually, every file behaves like:

```mage
main : (procedure Runtime, U64) {
    ...
}
```

or another appropriate return type depending on the file’s role.

This means:

- user-executed files are file-level procedures
- imported files are file-level procedures
- test files are file-level procedures
- there is no separate file execution model outside procedure semantics

### 4.5. Procedure compilation boundary

Compilation of a procedure should affect only the procedure’s own source block.

Nested source blocks remain independent and must only be compiled if execution reaches them.

Example:

```mage
x = 0;

if x > 0, {
    x = x + 1;
};
```

In this example, if `x > 0` is false, the `if` source block never needs to be compiled.

This is a critical architecture rule.

---

## 5. File and import model

### 5.1. Every file is implicit `main`

Every file is an implicit `main` procedure.

This applies to:

- regular executable files
- imported definition files
- test files

### 5.2. Import semantics

`import` is not a special compiler-only mechanism.

`import` is just another callable value present in runtime.

Import behavior is:

- importing a file is a call of that file’s implicit `main`
- the call happens in the same runtime currently executing
- the argument is the current runtime state at import-call moment

So `import` means, at most:

> call this file in the same runtime with current state

Example:

```mage
example : import "example.hex"
```

This means:

- `example.hex` is executed in the same runtime
- the result of that file’s implicit `main` is assigned to `example`

### 5.3. Import returns explicit exported value

The returned value of an imported file should follow the explicit export-value model.

The current guiding example is `arcanum/core.hex`:

```mage
IO    : import "core/IO.hex";
linux : import "core/linux.hex";

OS : linux;

export Class {
    IO              : IO;
    linux           : linux;
    File            : OS.File;
    File.open       : OS.File.open;
    File.openat     : OS.File.openAt;
    File.close      : OS.File.close;
    File.read       : OS.File.read;
    File.write      : OS.File.write;
    getStdinReader  : OS.getStdinReader;
    getStdoutWriter : OS.getStdoutWriter;
    getStderrWriter : OS.getStderrWriter;
};
```

This means the imported file’s implicit `main` should return the value produced by its `export ...` statement.

The precise export surface syntax may evolve later, but the architectural rule is:

- each imported file returns one explicit public value
- import assigns that returned public value
- member access such as `example.add` works on that returned exported value

### 5.4. Runtime sharing during import

Import does **not** start a fresh runtime.

Import uses the current runtime.

This is important because it means:

- imported code can observe current runtime state
- imported code can contribute additional variables into the current runtime flow
- import participates in the same execution model as all other calls

---

## 6. Resolution model

### 6.1. Resolution happens when execution reaches declaration

Mage does not globally resolve declarations ahead of time.

A declaration is resolved only when execution reaches it.

Example:

```mage
Alias : U64;
twice : (procedure {a : Alias}, Alias) {
    return a;
}
```

This means:

- `Alias` is resolved when that declaration executes
- `twice` is created when that declaration executes
- `twice` sees whatever `Alias` means at that moment

### 6.2. Unreached code is not semantically validated yet

If a branch is lexically correct but never reached, then deeper semantic failures inside it are not static architecture problems.

They are execution and test problems.

Example:

- an unreachable branch may contain invalid unresolved code
- if no test reaches it, no execution-driven error is produced
- the language should proceed as long as the code is lexically valid

This is intentional.

Mage is not trying to prove all possible code paths ahead of time.

---

## 7. Compilation model

### 7.1. Only currently entered executable code is compiled

Mage should not resolve or compile code that has not been entered.

Only the currently entered executable code is resolved and compiled.

This rule applies to:

- file-level `main`
- procedure bodies
- `if` blocks
- `while` blocks
- future nested executable source blocks

### 7.2. Entered-block-only compilation

This means:

- when a file starts executing, only its current top-level source is compiled
- when a procedure is called, only that procedure’s own source is compiled
- nested source blocks remain deferred
- an `if` block is compiled only when execution enters it
- a `while` block is compiled only when execution enters it
- an untaken branch remains uncompiled
- an uncalled procedure remains uncompiled

### 7.3. Nested source blocks are independent compilation units

Nested source blocks are independent.

Compiling an outer executable unit does not recursively compile nested source blocks.

This is a hard rule of the architecture.

### 7.4. No separate compile cache

There should be no special separate cache service for lazily compiled code.

Instead, the value built from source should directly own:

- a source reference
- a bytecode pointer or link

Execution rule:

1. inspect the executable value
2. if its bytecode link is empty, compile its own source block
3. store the resulting bytecode link in that value itself
4. execute the compiled bytecode
5. on later execution, if the link is already present, execute it directly

A single executable value should compile at most once.

This rule is central.

---

## 8. Labels

Labels are variables too.

They are runtime variables, not just compiler symbols.

They should be created in runtime before jumping into label-related code.

This is important for:

- `break`
- `continue`
- future label-targeted control flow
- consistent “everything is variable” semantics

---

## 9. Virtual Machine mode

### 9.1. Current status

Partially implemented.

The current implementation already has:

- bytecode format
- source maps
- VM execution
- stack-based calling convention
- bytecode printing and reading

But it still assumes eager whole-body compilation in many places.

### 9.2. VM remains the execution target

Even under the future lazy runtime model, executable code still becomes bytecode and runs in the VM.

### 9.3. Memory management

Mage VM uses stack as its main memory storage.

Each procedure reserves memory of fixed size for its variables.

The caller places argument and return variables, and the call instruction shifts the stack down by the size of memory required for callee locals.

This exists today and is expected to remain conceptually valid, though it may need adaptation for deferred executable values.

### 9.4. Calling convention

Memory order in the caller frame:

- return variables
- return address
- argument variables

Conceptually:

```mage
0 -> Return variables -> Return address -> Argument variables
```

### 9.5. Current-style memory layout example

```mage
test2 : (procedure {c : U64}, U64) {
    return c;
};

test1 : (procedure {x : U64; y : U64; z : U64}, U64) {
    a = 0d10;
    b = test2 a;
};

test1 0d10, 0d20, 0d30;
```

**`main` frame**:

- `0`  = return variable 1 for `test1`
- `8`  = return address for `test1`
- `16` = `0d10` (`x`)
- `24` = `0d20` (`y`)
- `32` = `0d30` (`z`)

**`test1` frame**:

- `0`  = return variable 1 for `test2`
- `8`  = return address for `test2`
- `16` = `0d10` (`a`) | caller return variable 1 for `test1`
- `24` = return address for `test1`
- `32` = `0d10` (`x`)
- `40` = `0d20` (`y`)
- `48` = `0d30` (`z`)

**`test2` frame**:

- `0`  = return variable 1 for `test2`
- `8`  = return address for `test2`
- `16` = `0d10` (`a`)

### 9.6. Current compiler

Today the compiler takes a `FlatRoot` and `SourceLocations` and produces bytecode and a `SourceMap`.

Current high-level approach:

1. first pass emits instructions with placeholders
2. second pass patches fixups with resolved offsets

This exists today.

The compiler currently recognizes the required grouped procedure declaration shape:

```mage
add : (procedure {x : U64; y : U64}, U64) {
    return x + y;
}
```

The obsolete direct declaration-with-body shape is no longer the supported procedure declaration form.

However, the compiler still eagerly compiles nested source blocks in places like `if` and `while`, so entered-block-only compilation has not been implemented yet.

### 9.7. Future compiler direction

The future compiler should support:

- compiling only the currently entered executable source
- resolving declarations in active runtime
- compiling procedure bodies independently from nested source blocks
- compiling nested blocks only when entered
- attaching resulting bytecode pointers to owning executable values

This is not implemented yet.

### 9.8. Source location mapping

The compiler produces a `SourceMap` alongside bytecode.

It maps bytecode instruction offsets to source byte ranges, enabling:

- **Bytecode → source**
- **Source → bytecode**

A `LineIndex` converts byte offsets to line and column pairs via binary search.

This already exists today and should remain valid, but future lazy materialization will require source maps across multiple separately materialized bytecode areas.

---

## 10. Test-driven architecture

### 10.1. Tests are central to the language model

Mage will be developed mostly through tests.

This is not only a workflow preference. It is a core architectural rule.

Because code is resolved and compiled only when entered, tests are the primary way to:

- drive execution
- force materialization
- obtain diagnostics
- produce traces
- debug semantic failures

### 10.2. Definition files alone do not provide full diagnostics

A file can define executable values without forcing them to be compiled.

Example:

```mage
add : (procedure {a : U64; b : U64}, U64) {
    return a + b;
}
```

If no execution calls `add`, then:

- its body may remain uncompiled
- many semantic problems inside it remain undetected
- this is expected

Definition files can still provide:

- parse diagnostics
- lexical or syntax diagnostics
- shallow structural diagnostics

But execution-driven diagnostics require tests.

### 10.3. Test files are isolated automated tooling entrypoints

A `*_test.hex` file is not merely a helpful example.

It is the default automated entrypoint for tooling execution.

Difference between regular files and test files:

- **regular `.hex` file**
  - usually executed directly by a user
- **`*_test.hex` file**
  - executed automatically by tools

This includes:

- language server
- debug adapter
- tracers
- future analysis tools
- future omniscient debugging tools

### 10.4. Test isolation rules

Each test file must run in a fresh isolated runtime.

Imported modules must be reloaded for each test run.

Each test file is isolated from all other test files.

Omniscient records must be per such isolated test execution.

These are hard architecture requirements.

### 10.5. Example test style

```mage
example : import "example.hex"

assert (example.add 2, 2), 4
```

A test file should:

- import definition files
- call exported values
- force lazy materialization of reachable code
- validate correct and incorrect cases
- serve as the automated execution unit for tooling

### 10.6. LSP diagnostics come from real test execution

The language server should not simulate tests.

It should literally execute tests, collect tracing, and provide diagnostics from actual runs.

This is a crucial rule.

### 10.7. Execution-driven diagnostics

The following categories are execution-driven diagnostics:

- unknown identifier in never-entered code
- invalid return count in never-called code
- type mismatch in body expression that was never reached
- incorrect branch semantics in unreachable code
- many other semantic failures that depend on actual execution

These should be found through tests, traces, and debugging, not through speculative whole-program analysis.

---

## 11. Tracing and debugging

### 11.1. Current status

The current debug adapter is only a minimal protocol stub.

The future architecture must be designed for tracing from the start.

### 11.2. Test files are the primary tooling execution units

Tracing and debugging should center on `*_test.hex` runs.

A test run is the right unit for:

- execution recording
- semantic diagnostics
- replay
- debugger entry
- future omniscient analysis

### 11.3. Omniscient debugging target

The long-term target includes omniscient debugging focused on isolated test runs.

Important facts to capture:

- variable assignments
- call and return events
- declaration resolution events
- block entry events
- bytecode materialization events
- source-to-bytecode mapping for each materialized area

### 11.4. Comments are real calls

Comments are calls to the special `comment` procedure.

Example:

```mage
comment "This is a comment";
```

Comments are not separate syntax.

They are normal calls in the language model.

### 11.5. Low-overhead debugging direction

Tracer research strongly suggests that bytecode patching is the correct future direction for breakpoints and low-overhead debugging.

That means:

- little or no overhead when debugging is off
- targeted patching of materialized bytecode
- debugger behavior centered on actual executable bytecode, not on always-on interpreter hooks

---

## 12. `.bytecode` artifacts

### 12.1. Current `--save` is temporary

The current save behavior is temporary and not the final architecture.

### 12.2. Future `.bytecode` generation model

In the future, `.bytecode` files should be generated explicitly from procedure values.

Example:

```mage
main : (procedure Void, U8) {
    return 0d1;
};

bytecode "example", main;
```

This means:

- build a `.bytecode` file named `example`
- use procedure `main` as the entry procedure for the artifact

### 12.3. Meaning of `.bytecode`

A `.bytecode` artifact should be generated from resolved procedure values rather than from an unspecified whole-project snapshot.

The intent is:

- choose a procedure value
- resolve and compile what is needed for that artifact
- write resulting `.bytecode`

This follows the same explicit exported-value and explicit procedure-entry model used elsewhere in Mage. Artifact generation should be explicit and driven from chosen values, not from hidden whole-project policy.

This is future work.

---

## 13. Native mode

Native mode is **not** the current implementation target.

It will be added later.

When added, there should be:

- a compiler to machine code instead of `.bytecode`
- a runtime populator appropriate for native mode
- eventually switchable runtime populators

Default calling convention target is System V.

No further design work for native mode is required before the current VM-centered architecture is built.

---

## 14. Target architectures and operating systems

Mage targets:

- `amd64`
- `arm64`

Primary operating system target:

- Linux

Secondary operating system targets:

- macOS
- FreeBSD

---

## 15. Syntax

### 15.1. Source encoding

Mage source files are UTF-8 encoded.

The parser processes Unicode text.

### 15.2. Identifiers

Identifiers can contain:

- letters from any language using Unicode identifier categories
- decimal digits
- underscores

An identifier must not start with a digit.

Example:

```mage
counter : 0d0;
user_name : "Alice";
счётчик : 0d0;
计数器 : 0d0;
カウンター : 0d0;
データ_count_數據 : 0d0;
```

### 15.3. String literals

String literals are enclosed in double quotes.

They support UTF-8 content.

Newlines are not allowed inside string literals.

Supported escapes:

- `\n`
- `\"`
- `\\`

Example:

```mage
greeting : "Hello, 世界! 🌍";
```

### 15.4. Numeric literals

Numeric literals use a `0` followed by a radix letter, or a bare `0`.

| Prefix | Radix   | Example  | Decimal value |
|--------|---------|----------|---------------|
| `0b`   | Binary  | `0b1010` | 10            |
| `0o`   | Octal   | `0o52`   | 42            |
| `0d`   | Decimal | `0d42`   | 42            |
| `0x`   | Hex     | `0xFF`   | 255           |

Underscores are allowed after the prefix and ignored.

Example:

```mage
bits  : 0b11010;
perms : 0o755;
value : 0d42;
color : 0xFF8800;
```

### 15.5. Source blocks

Source blocks are enclosed in curly braces.

They contain zero or more statements and represent source code that can be stored, passed, or later compiled.

Example:

```mage
empty : {};

params : {
    x : U64;
    y : U64;
};
```

### 15.6. Operators

Mage supports binary operators at three precedence levels.

**Multiplicative**:

| Operator | Description    |
|----------|----------------|
| `*`      | Multiplication |
| `/`      | Division       |
| `%`      | Modulo         |

**Additive**:

| Operator | Description |
|----------|-------------|
| `+`      | Addition    |
| `-`      | Subtraction |

**Comparison**:

| Operator | Description            |
|----------|------------------------|
| `<`      | Less than              |
| `>`      | Greater than           |
| `<=`     | Less than or equal     |
| `>=`     | Greater than or equal  |
| `==`     | Equal                  |
| `!=`     | Not equal              |

Parentheses override precedence.

Example:

```mage
return 0d3 + 0d2 * 0d4;
return (0d3 + 0d2) * 0d4;
```

### 15.7. Statements

A source file or source block contains statements separated by `;`.

Trailing semicolons are optional.

Empty statements are ignored.

Statement forms:

- constant binding
- variable assignment
- multiple-variable assignment
- expression statement

Examples:

```mage
x : 0d42;
name : "Alice";
File.open : (procedure {path : String}, File) { ... };

x = 0d10;
x = x + 0d1;

quotient, remainder = divide_mod 0d17, 0d5;

print "hello";
```

### 15.8. Calls

A call is formed when a callable value is followed by a value.

Arguments are comma-separated and each argument is a full expression.

Examples:

```mage
return 0d42;
syscall.write file_descriptor, pointer, length;
test void;
return double factorial 0d5;
(f x) y;
return (fibonacci n - 0d1) + (fibonacci n - 0d2);
```

### 15.9. Everything is a call

Procedures, control flow, types, imports, comments, and modules are all intended to use the same call syntax.

Target-style examples:

```mage
add : (procedure {x : U64; y : U64}, U64) {
    if x == 0d0, { return y; };
    return x + y;
};

divide_mod : (procedure {a : U64; b : U64}, {q : U64; r : U64}) {
    return a / b, a % b;
};

get_answer : (procedure Void, U64) {
    return 0d42;
};

x = get_answer void;
```

### 15.10. Member access

The dot operator accesses a member of a value.

Examples:

```mage
file.file_descriptor;
IO.Reader;
syscall.openat directory_file_descriptor, path;
```

### 15.11. Grammar summary

The parser behavior is currently close to this shape, while future runtime semantics extend the meaning of some forms:

```mage
source_file  :=  statement (';' statement)*

statement    :=  name ':' expression
             |   name '=' expression
             |   name (',' name)+ '=' expression
             |   expression
             |   ε

name         :=  identifier ('.' identifier)*

expression   :=  arithmetic (comparison_op arithmetic)?
comparison_op:=  '<' | '>' | '<=' | '>=' | '==' | '!='

arithmetic   :=  term (('+' | '-') term)*

term         :=  call (('*' | '/' | '%') call)*

call         :=  callable argument (',' argument)*
             |   atom

callable     :=  identifier | name | '(' expression ')'
argument     :=  expression

atom         :=  '(' expression ')'
             |   '{' source_file '}'
             |   '"' string_chars '"'
             |   '0' radix digits_or_underscores
             |   '0'
             |   identifier ('.' identifier)*
```
