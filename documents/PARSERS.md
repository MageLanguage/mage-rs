# Parsers and Compilers

Research on parser architectures, source location strategies, compilation techniques, intermediate representations, and memory management across languages and toolchains.

---

## 1. Source Location Strategies

### 1.1. Spans on AST Nodes — rustc, swc, Go, Cuik

**rustc** went through three iterations of span size: 16 bytes → 4 bytes → 8 bytes. The current design packs `BytePos lo`, `BytePos hi`, and `SyntaxContext` into 8 bytes. 99.9% of spans fit inline; rare overflows spill to a thread-local interner. A separate `SourceMap` stores line-start offsets and converts byte positions to line/column lazily via binary search. The key decision: store byte offsets everywhere, compute line/column only when needed for diagnostics. This avoids the cost of tracking newlines during parsing.

**swc** mirrors rustc: `Span { lo: BytePos(u32), hi: BytePos(u32), ctxt: SyntaxContext }` = 12 bytes per node. `BytePos(0)` is reserved for compiler-synthesized spans. The cost is real — on a large AST with millions of nodes, spans alone can consume tens of megabytes.

**Go** uses a single `token.Pos` integer per node. All files in a compilation share a virtual address space via `FileSet.AddFile(name, base, size)`. Each file occupies `[base, base+size]`. Resolution is a binary search on the `FileSet`, then on the file's line-start table. Only start position is stored; end positions are recovered from AST structure. This is the most space-efficient per-node approach: 4 bytes per node, no end position stored.

**Cuik** packs source locations into `u32` with bit fields: 1 bit macro flag, 14 bits file ID, 17 bits file position. `SourceRange` is two of these = 8 bytes. Every `Stmt` and `Subexpr` carries a `SourceRange`. Line/column resolution uses a binary-searchable `line_map` per file. The bit-packing is impressively tight but limits file size to 128KB and file count to 16384 — acceptable for C but not for arbitrary use.

### 1.2. No Stored Positions — Zig, Roslyn, rowan

**Zig** AST stores token indices, not byte offsets. Tokens store only their start byte offset. When converting AST → ZIR, each instruction stores a `node_offset` relative to the parent declaration. After ZIR generation, the AST and source text can be freed. From the Zig documentation: "The minimum amount of information needed to represent a list of ZIR instructions. Once completed, it can be used to generate AIR, followed by machine code, without any memory access into the AST tree token list, node list, or source bytes." This is a radical approach — the IR is designed to be completely self-contained, with no backward references to the source.

**Roslyn** (C#) uses immutable red-green trees. Green nodes store width (character count), not absolute offsets. The red tree layer computes absolute positions by summing widths from the root. This means zero storage cost per node for positions, but O(depth) to compute any node's position. The primary motivation is incremental reparsing: because green nodes have no absolute positions, they can be reused across edits without invalidation.

**rowan** (rust-analyzer) follows Roslyn's design. Green nodes are position-free and reusable across edits. Red (syntax) nodes compute `TextRange(u32, u32)` on demand. The green tree is essentially a persistent data structure — edits produce new trees that share most of their structure with the old tree.

### 1.3. Bytecode-to-Source Side Tables — JVM, CPython, Lua, DWARF, JS Source Maps

**JVM** `LineNumberTable`: array of `(bytecode_pc: u16, line_number: u16)` pairs per method. 4 bytes per entry. Line-only granularity. Always generated even without `-g`. `LocalVariableTable` is separate: 10 bytes per variable entry. The line-only granularity is a deliberate trade-off — it's sufficient for stack traces and debugger stepping, and keeps class files compact.

**CPython PEP 657** (Python 3.11+): added `(start_line, end_line, start_col, end_col)` per instruction. Column offsets stored as `uint8_t` (0-255). Location tables became ~9x larger than in 3.10, overall `.pyc` size increased ~22%. Opt-out via `PYTHONNODEBUGRANGES` or `-Xno_debug_ranges`. The motivation was to show exact error locations in tracebacks (the `^^^` under the offending expression), which is worth the size increase for developer experience.

**Lua**: standard Lua uses 4 bytes per instruction for line info. Lua Compact Debug (NodeMCU) replaces this with run-length encoding of line deltas. LuaJIT uses adaptive encoding: `u8` for ≤255 lines, `u16` for ≤65535, `u32` otherwise. The adaptive approach is elegant — most functions are short, so most line numbers fit in a byte.

**DWARF** `.debug_line`: state machine program. Opcodes advance address and line registers simultaneously. Special opcodes (1 byte) encode both a line increment and address increment. Standard opcodes use LEB128 for larger deltas. Most compact of all approaches — a single byte often encodes a full statement. The state machine design means the decoder must process opcodes sequentially, but the encoding density is unmatched.

**JS Source Maps** (ECMA-426): VLQ delta-encoded Base64 string. Each segment encodes: generated column delta (reset per line), source file index delta, original line delta, original column delta, optional name index delta. Semicolons separate generated lines. Reduced source map sizes by 50% vs v2. Bidirectional lookup via binary search on sorted segments. The delta encoding exploits the fact that consecutive generated positions usually map to nearby source positions.

---

## 2. Parser Architectures

### 2.1. Pratt Parsing — Top-Down Operator Precedence

Vaughan Pratt's 1973 algorithm for expression parsing assigns a "binding power" (integer precedence) to each operator. The parser consists of two functions per token: `nud` (null denotation, for prefix position) and `led` (left denotation, for infix/postfix position). The core loop is remarkably simple:

```
fn parse(min_bp):
    left = nud(next_token())
    while bp(peek()) > min_bp:
        left = led(left, next_token())
    return left
```

Bob Nystrom's explanation in "Pratt Parsers: Expression Parsing Made Easy" made the algorithm accessible to a much wider audience than Pratt's original paper. The key insight: recursive descent is natural for statements (which start with keywords), and Pratt parsing is natural for expressions (which start with operands). The two compose perfectly — use recursive descent at the statement level, Pratt parsing within expressions.

The elegance is in the extensibility: adding a new operator requires only specifying its binding power and its `led` function. No grammar rewriting, no precedence table refactoring. This is why Pratt parsers dominate in language implementations that need to evolve their operator set.

Source: https://journal.stuffwithstuff.com/2011/03/19/pratt-parsers-expression-parsing-made-easy/

### 2.2. PEG Parsing and Packrat Memoization

Parsing Expression Grammars (Bryan Ford, 2004) define syntax in terms of recognition rather than generation. The choice operator is ordered: `A / B` tries `A` first, and only tries `B` if `A` fails. This eliminates ambiguity by construction — every valid input has exactly one parse tree.

PEG parsers are naturally recursive descent, with one function per grammar rule. The problem: backtracking can cause exponential time. Packrat parsing solves this by memoizing every rule's result at every input position, guaranteeing linear time at the cost of O(input × rules) memory.

The Squirrel Parser (Hutchison, 2025) demonstrates that left recursion — traditionally impossible in PEG — can be handled via cycle detection and fixed-point iteration, while maintaining linear time. It also derives a provably optimal error recovery strategy from first principles using constraint satisfaction.

GPeg (Yedidia & Chong, 2021) makes packrat parsing incremental by storing the memoization table as an interval tree with support for shifting intervals. This enables reparsing in time logarithmic in the input size for typical edits, compared to linear for standard incremental packrat parsing.

The practical limitation: packrat parsing's memory consumption. For a 1MB source file with 100 grammar rules, the memoization table is ~100MB. Various strategies exist (lazy memoization, bounded tables, selective memoization), but the space-time trade-off is fundamental.

Source: https://we-like-parsers.github.io/pegen/peg_parsers.html

### 2.3. Tree-sitter — Incremental LR Parsing with Error Recovery

Tree-sitter is a parser generator that produces incremental GLR parsers in C. Its design priorities — in order — are: robustness (useful results with syntax errors), speed (parse on every keystroke), generality (any language), and dependency-freedom (pure C11 runtime).

The incremental parsing algorithm: when the source is edited, tree-sitter identifies which portions of the old syntax tree are invalidated by the edit, then reparses only those portions, reusing unchanged subtrees. The key data structure is the syntax tree itself — nodes store their byte ranges, and the parser can skip over subtrees whose ranges weren't affected by the edit.

Error recovery is built into the LR automaton. When the parser encounters an unexpected token, it tries multiple recovery strategies in parallel (insertion, deletion, wrapping in an ERROR node) and picks the one that consumes the most input successfully. The result is that tree-sitter always produces a complete syntax tree, even for wildly malformed input — critical for editor use cases where the source is constantly in an invalid state.

Tree-sitter grammars are written in a JavaScript DSL that generates C parsing tables. The generated parsers are typically 50–200KB of C code per language. The runtime library is ~50KB. This is small enough to embed in any application.

The adoption is remarkable: tree-sitter is used in Neovim, Helix, Zed, GitHub code navigation, and many other tools. It has effectively replaced regex-based syntax highlighting in modern editors.

Source: https://tree-sitter.github.io/tree-sitter

### 2.4. Accelerated-Zig-Parser (Validark) — SIMD Tokenization

SIMD-accelerated tokenizer achieving 2.75x faster and 2.47x less memory than mainline Zig tokenizer.

Key techniques:
- **SIMD bitstring scanning**: produces bitstrings per 64-byte chunk for identifiers, quotes, whitespace, and comments simultaneously, then uses vector compression to find token extents.
- **Perfect hash functions**: keywords and operators mapped into 7-bit address space. Single 16-byte comparison per identifier to check against keyword table. Uses Phil Bagwell's array-mapped trie compression for packed lookup buffers.
- **Token length encoding**: stores token lengths as `u8` instead of absolute `u32` start offsets. Almost all tokens are <256 bytes. A `0` sentinel indicates the next 4 bytes contain the true length. This is a 4x memory reduction for the common case.
- **Sentinel padding**: source buffer padded with sentinel characters at the end. Eliminates bounds-checking in inner loops — the SIMD scan cannot overrun because sentinels terminate every scan pattern.
- **Newline bitmap as reusable artifact**: SIMD scanning produces a non-newline bitmap as a side product. Later pipeline stages reuse it for line-number computation without re-scanning the source.

The general principle: tokenization is embarrassingly SIMD-friendly because it operates on independent byte-level predicates. The same 64-byte chunk can be tested for all token-start conditions simultaneously.

### 2.5. Meriyah — Opt-In Location Tracking

100% ECMAScript-compliant JavaScript parser. Key design: location tracking is opt-in via boolean flags (`ranges`, `loc`). When both are off, AST nodes carry zero location overhead. When on, the parser captures its current position into nodes.

This is a useful pattern for multi-pass architectures: the first pass can parse without locations (fast), and a second pass can re-parse specific regions with locations enabled (only for diagnostics or tooling). The cost of location tracking is not just storage — it's also the cost of maintaining line/column state during scanning, which involves checking for newlines on every character advance.

### 2.6. GLL Parsing — Generalized Recursive Descent

GLL (Generalized LL) parsing, described by Scott and Johnstone (2010), extends recursive descent parsing to handle all context-free grammars, including left-recursive and ambiguous grammars. The key idea: when the parser reaches a point where multiple alternatives could apply, it forks, exploring all possibilities in parallel using a graph-structured stack (GSS) to share common prefixes.

The GSS is what makes GLL practical: instead of an exponential number of independent parser stacks, shared stack nodes mean the parser runs in cubic time (O(n³)) in the worst case and linear time for unambiguous grammars. The output is a Shared Packed Parse Forest (SPPF) representing all possible derivations compactly.

GLL is attractive because it retains the structure of recursive descent — each grammar rule maps directly to a parsing function — while removing all restrictions on the grammar. No left-recursion elimination, no ambiguity resolution, no lookahead constraints. Afroozeh and Izmaylova (2015) demonstrated practical GLL with speedups of 1.5–5.2x over the original algorithm on Java, C#, and OCaml grammars.

The Iguana parsing framework implements optimized GLL and has been used for data-dependent grammars that unify lexing and parsing (scannerless parsing), handle operator precedence, indentation sensitivity, and preprocessor directives — all in a single formalism.

Source: https://pure.royalholloway.ac.uk/en/publications/purely-functional-gll-parsing

### 2.7. Scannerless Parsing — No Separate Lexer

Scannerless (lexerless) parsing eliminates the traditional lexer/parser pipeline, using a single grammar formalism from characters to syntax trees. The grammar describes both token structure and phrase structure in one unified specification.

The primary advantage: compositional grammars. When two languages are embedded (e.g., SQL in Java, HTML in PHP, regex in any language), their token rules may conflict. A traditional lexer cannot handle multiple token grammars simultaneously. A scannerless parser treats everything as characters, avoiding the conflict entirely.

Scannerless GLR parsing (SGLR) has been used in the Spoofax language workbench and the Rascal meta-programming language for exactly this purpose — parsing real-world programs in languages with complex lexical interactions.

The cost: scannerless grammars are more ambiguous than tokenized ones (because character-level alternatives create more nondeterminism), requiring a more powerful — and slower — parsing algorithm. SRNGLR (Economopoulos et al., 2009) is on average 33% faster than SGLR, and 95% faster on highly ambiguous grammars.

The relevance: any language that supports string interpolation, heredocs, or embedded DSLs faces the same lexer composition problem. Scannerless parsing is the principled solution.

Source: https://en.wikipedia.org/wiki/Scannerless_parsing and https://ir.cwi.nl/pub/24027/24027B.pdf

### 2.8. TCC — No AST, Direct Code Emission

Fabrice Bellard's Tiny C Compiler parses C and emits machine code in a single pass, with no AST. Source locations flow from the lexer's current position directly into DWARF debug info during code generation. Each time the code generator emits an instruction, it records the current source line.

This is the extreme end of the "no intermediate representation" spectrum. The benefit is speed and simplicity: TCC compiles C faster than GCC can preprocess it. The cost is optimization — without an IR, there is no opportunity for analysis or transformation between parsing and code generation. TCC-compiled code runs 3–10x slower than GCC -O2.

For interactive use cases (compile-and-run scripts, rapid iteration), TCC's approach is compelling. The compilation is so fast that the compile-time component of edit-compile-run is effectively zero.

### 2.9. Lezer — Incremental Parsing for Code Editors

Lezer is the parser system built for CodeMirror 6. Like tree-sitter, it is designed for incremental parsing, error tolerance, and providing a syntax tree for editor tooling. However, unlike tree-sitter's C/C++ foundation, Lezer generates JavaScript modules that run directly in the browser without WebAssembly overhead.

It uses an LR parsing algorithm but is highly specialized for JavaScript's execution model. Lezer outputs a compact, non-abstract syntax tree (where nodes keep track of their width and structure rather than being full JS objects), ensuring low memory consumption and high locality. The parser seamlessly recovers from syntax errors, guaranteeing that an editor always has a workable syntax tree for highlighting and code navigation.

### 2.10. SIMD-Accelerated Structural Parsing (simdjson)

Geoff Langdale and Daniel Lemire pioneered parsing gigabytes of data per second by heavily utilizing SIMD instructions (e.g., AVX-512) for more than just tokenization. In libraries like `simdjson` and `simdcsv`, the parser operates in two distinct stages:
1. **Structural Index Generation:** SIMD instructions simultaneously identify all structural characters (quotes, colons, brackets, delimiters) across a block of 64 bytes. This produces a bitmap index of all structurally significant locations.
2. **Structural Processing:** A second pass iterates only over the structural characters identified in the first stage to build the actual parsed representation. 

By eliminating byte-by-byte loops and branch mispredictions, this approach can parse at gigabytes per second, frequently bottlenecking on main memory bandwidth rather than the CPU.

### 2.11. Parsing with Derivatives

Introduced by Matt Might et al., this is an elegant approach that extends Brzozowski’s derivative for regular expressions to arbitrary Context-Free Grammars (CFGs). A parser evaluates the "derivative" of a grammar with respect to the first token of input, returning a new grammar that matches the remainder of the input. 

While its naive implementation can suffer from exponential blowup, with appropriate memoization, laziness, and fixed-point operations, it translates into concise, purely functional code that can parse ambiguous and left-recursive grammars. Later optimizations (like Zippy LL(1) Parsing with Derivatives) have reduced its time complexity to linear for restricted grammars, turning an interesting functional pearl into a practically viable parsing strategy.

---

## 3. Flat and Compact AST Representations

### 3.1. Cuik — Postfix Expression Encoding

Cuik stores expressions as a flat `Subexpr` array in reverse-Polish order. Child references are implicit from position, not pointers. From the source: "To represent a metric shitload of expressions in Cuik we compact them using a postfix notation. Instead of using pointers to refer to inputs it's implicit."

The benefit: excellent cache locality (sequential memory access), no pointer overhead (typically 8 bytes per child), and trivial serialization. The cost: random access to a specific subexpression requires walking from the beginning. For a compiler that processes expressions sequentially (evaluation, code generation), this is a net win.

### 3.2. Roslyn/rowan — Immutable Red-Green Trees

The red-green tree pattern splits the syntax tree into two layers:

- **Green tree**: immutable, position-free, structure-only. Nodes store their kind and width (character count). Children are referenced by index. Because nodes carry no absolute position, structurally identical subtrees can be shared — even across different files or across edits.
- **Red tree**: ephemeral, position-aware, computed on demand. Wraps green nodes with absolute text ranges computed by summing widths from the root. Red nodes are created lazily as the user navigates the tree.

The power is incremental reparsing: when the user edits a file, the parser produces a new green tree that shares most of its nodes with the old tree. Only the edited region and its ancestors are rebuilt. The red tree is discarded entirely — it's cheap to recompute.

rust-analyzer's rowan library demonstrates this at scale: it provides sub-millisecond reparsing for most edits on files of any size, while supporting full-fidelity syntax trees (every character of the source is represented, including whitespace and comments).

### 3.3. Zig — Token-Indexed AST with Relative Offsets

Zig's AST references tokens by index, not by source byte offset. The AST nodes form a flat array (struct-of-arrays layout for cache efficiency). When lowered to ZIR (Zig Intermediate Representation), each instruction stores a `node_offset` relative to its parent declaration rather than an absolute token index.

The consequence: after ZIR generation, the AST, token list, and source text can all be freed. The ZIR is self-contained. This is critical for Zig's compilation model, where the compiler processes many files and needs to minimize peak memory usage. The relative offsets can be resolved back to source positions by walking the declaration tree — an O(depth) operation done only for error reporting.

---

## 4. Compilation Techniques

### 4.1. Copy-and-Patch — Stencil-Based Code Generation

Haoran Xu and Fredrik Kjolstad (Stanford, 2021) introduced copy-and-patch compilation: instead of emitting machine code instruction by instruction, the compiler pre-compiles a library of "stencils" — code snippets for each bytecode operation with "holes" for operands. Code generation becomes: copy the stencil, patch the holes with concrete values. Done.

The results are striking: compilation is 4–6x faster than the fastest existing baseline compilers (like Liftoff in V8), while producing code of comparable quality. The compilation time is dominated by `memcpy` — there's almost no per-instruction decision-making.

The technique has historical roots: QEMU's original "dyngen" backend by Fabrice Bellard used a similar approach in 2003, compiling C stencils and extracting relocations. Copy-and-patch modernizes this by using LLVM as the stencil compiler, enabling better stencil quality and automatic relocation extraction.

CPython 3.13 adopted copy-and-patch for its experimental JIT. The LuaJIT Remake project by Haoran Xu applies the technique to Lua. The technique is particularly well-suited for tier-1 (baseline) compilers in tiered JIT systems, where compilation speed matters more than peak code quality.

Source: https://arxiv.org/abs/2011.13127

### 4.2. Tiered Compilation — Interpreter → Baseline → Optimizing

Modern VMs typically have two or three execution tiers:

1. **Interpreter**: zero compilation latency, slow execution. Good for code that runs once.
2. **Baseline compiler**: fast compilation (~1ms), moderate execution speed. Good for warm code.
3. **Optimizing compiler**: slow compilation (~100ms), fast execution. Good for hot loops.

V8 (JavaScript): Ignition (interpreter) → TurboFan (optimizing). SpiderMonkey (JavaScript): interpreter → baseline → IonMonkey. JVM HotSpot: interpreter → C1 (baseline) → C2 (optimizing).

The key observation from Jamie Brandon: for interactive languages, the combined compile-time + run-time matters more than either alone. A program that compiles in 0.1s and runs in 1s is better than one that compiles in 0s and runs in 2s (interpreter) or compiles in 10s and runs in 0.5s (LLVM -O3). The sweet spot — fast compilation, reasonable execution — is underserved by existing tools.

The debugging story for tiered systems is unsolved. When the interpreter hits a breakpoint, the variable layout is different from the baseline compiler, which is different from the optimizing compiler. On-stack replacement (OSR) — switching tiers while a function is executing — complicates this further.

Source: https://www.scattered-thoughts.net/writing/implementing-interactive-languages/

### 4.3. Futamura Projections — Compilers from Interpreters

Yoshihiko Futamura's 1971 insight: if you have a partial evaluator (a program that specializes a program given some of its inputs), you can derive compilers mechanically:

- **First projection:** Partially evaluate an interpreter with respect to a specific source program. The result is a compiled version of that program — the interpreter's dispatch overhead has been "baked away," leaving only the operations specific to the source program.
- **Second projection:** Partially evaluate the partial evaluator with respect to the interpreter. The result is a compiler for that language — a program that transforms source programs into compiled programs.
- **Third projection:** Partially evaluate the partial evaluator with respect to itself. The result is a compiler generator — a program that transforms interpreters into compilers.

The Truffle/Graal system (GraalVM) is the most successful practical application: Truffle interpreters written in Java are partially evaluated by Graal's JIT compiler, producing optimized native code. The programmer writes only the interpreter; the compiler is derived automatically.

Supercompilation (Turchin, 1986) generalizes partial evaluation by "driving" through all possible generalized execution histories of the original program, reducing redundancy of any kind — not just known inputs. In practice, supercompilers are harder to control than partial evaluators but can discover optimizations that partial evaluation misses.

The practical lesson: writing an interpreter is much easier than writing a compiler. If you have a sufficiently powerful partial evaluator or JIT, the interpreter *is* the compiler. This is why Truffle languages (Ruby, Python, JavaScript, R on GraalVM) can achieve near-native performance with relatively simple interpreter implementations.

Source: https://arxiv.org/pdf/2411.10559 and https://mazdaywik.github.io/direct-link/The%20Concept%20of%20a%20Supercompiler.pdf

### 4.4. Sea of Nodes and E-Graphs

The **Sea of Nodes** IR (Cliff Click, mid-1990s) represents a program as a graph where both data flow and control flow are edges. Unlike traditional SSA form, there is no fixed instruction ordering within a basic block — instructions are ordered only by their data dependencies. This gives the optimizer maximum freedom to move instructions.

The Sea of Nodes is used in HotSpot's C2 compiler, V8's TurboFan, and Graal. The main benefit: optimizations like common subexpression elimination, loop-invariant code motion, and instruction scheduling fall out naturally from the graph structure. The main cost: the IR is harder to understand and debug than linear SSA.

**E-graphs** (equality graphs) take this further. An e-graph represents not one program but an equivalence class of programs simultaneously. When an optimization rule fires (e.g., `x * 2 → x << 1`), both the original and the rewritten form are kept in the graph. After all rules have been applied to saturation, an extraction pass selects the best program from the equivalence class.

Cranelift (the Wasmtime compiler) adopted e-graph-based optimization, replacing its previous peephole optimizer. The `egg` library by Max Willsey provides a general-purpose e-graph implementation in Rust. The technique is particularly powerful for phase-ordering problems — the order in which optimizations are applied no longer matters, because all possible rewrites coexist in the e-graph.

Source: https://github.com/bytecodealliance/rfcs/blob/main/accepted/cranelift-egraph.md

### 4.5. Nanopass Compiler Frameworks

Traditional compilers typically use a small number of monolithic passes (e.g., parsing, semantic analysis, lowering, optimization, code generation). The Nanopass approach—most famously used by Chez Scheme—breaks the compilation process into dozens or even hundreds of extremely small, focused passes ("nanopasses").

In a nanopass compiler, each pass performs a single, specific transformation, and the intermediate language (IL) is formally defined at each step. A domain-specific language (DSL) generates the boilerplate for traversing the AST and ensuring that the IL conforms to the required grammar before and after each pass. The perceived downside—excessive compilation time due to so many passes—was proven false by Chez Scheme, which remains one of the fastest Scheme compilers available. The benefit is an incredibly maintainable, easily testable compiler architecture.

### 4.6. Surgical Monomorphization and Lambda Sets

Monomorphization (used heavily in Rust and C++) replaces polymorphic functions with concrete implementations for each specific type, which can lead to severe code bloat. The Roc programming language introduces "Surgical Monomorphization" using a technique called "Lambda Sets."

At compile time, Roc tracks the exact set of functions (lambdas) that can inhabit a function type at any call site. It then performs defunctionalization: turning higher-order functions into first-order functions by replacing function pointers with a tag and a switch statement. The "surgical" part means the compiler only specializes the code exactly where it yields a performance benefit or avoids heap allocation (boxing). The result is C-like performance and highly predictable memory usage in a purely functional language, without the pervasive code bloat of traditional monomorphization.

### 4.7. Dynamic Superinstructions & Direct Threaded Code

In bytecode interpreters, dispatch overhead (reading the next opcode and branching to its implementation) is a major bottleneck. Direct threaded code replaces opcodes with arrays of pointers directly to the machine code implementing each operation.

To further reduce dispatch overhead, systems like GForth use "Dynamic Superinstructions." Instead of executing individual primitives (e.g., `dup`, `*`), the VM dynamically identifies common sequences of bytecodes and compiles them into a single "superinstruction" on the fly, copying the machine code routines into a contiguous block. This eliminates the dispatch boundaries between those instructions and allows the CPU to execute them as a single block of native code. It bridges the gap between a pure interpreter and a JIT compiler by performing ultra-lightweight native code stitching at runtime.

---

## 5. Memory Management in Compilers

### 5.1. Arena Allocation — Bump Allocators

Arena (bump) allocation is the dominant memory management strategy in compilers. The pattern: allocate a large contiguous region, bump a pointer forward for each allocation, never free individual objects. When the compilation phase ends, free the entire arena at once.

Rust's `bumpalo` crate is the canonical implementation: allocation is a pointer bump + alignment check (~2ns). There is no per-object deallocation, no free list, no fragmentation. The trade-off: objects in the arena cannot be individually freed. This is acceptable in compilers because AST nodes, IR instructions, and type objects all have the same lifetime — they live for one compilation phase and die together.

The `bump-scope` crate extends this with allocation scopes/checkpoints: you can "reset" the arena to a previous point, freeing everything allocated after that point. This supports nested phases (e.g., parse a function body, optimize it, emit code, reset the arena for the next function).

Arena allocation also enables pointer-free representations: instead of storing pointers to children, store indices into the arena. Indices are typically 32 bits (vs 64-bit pointers), saving 4 bytes per reference. This is the approach used by ECS (Entity Component System) architectures in game engines, and it maps directly to flat AST representations.

### 5.2. String Interning and Hash Consing

**String interning**: store each unique string once, refer to it by index or pointer. Two interned strings can be compared for equality in O(1) by comparing their indices. Every major compiler uses this for identifiers — rustc's `Symbol`, V8's `InternedString`, Go's `string` (which are immutable and can be compared by pointer in some cases).

The implementation is a hash map from string content to index. The strings themselves are stored in a contiguous buffer (often arena-allocated). The lookup cost is one hash + one comparison per new string; subsequent uses are free.

**Hash consing** generalizes interning to structured data. Instead of deduplicating strings, it deduplicates entire data structures: if two AST nodes are structurally identical, they share the same allocation. Equality testing becomes pointer comparison — O(1) regardless of structure size.

Hash consing is used in BDD libraries, symbolic computation (JuliaSymbolics reported up to 100x faster numerical evaluation), and persistent data structures. In compilers, it's most useful for type representations — many expressions share the same type, and deduplication can dramatically reduce memory usage.

The key property: hash-consed structures are automatically persistent (immutable). Modifications create new structures that share unmodified subparts. This enables efficient incremental computation, undo/redo, and structural diff.

Source: https://en.wikipedia.org/wiki/Hash_consing

### 5.3. Struct-of-Arrays Layout

Instead of an array of structs (`[{kind, span, left, right}, ...]`), store parallel arrays (`kinds[], spans[], lefts[], rights[]`). This is the data-oriented design approach used by Zig's AST, ECS game engines, and many database column stores.

Benefits: better cache utilization when accessing one field across many nodes (e.g., iterate over all `kind` fields without touching `span` or children); enables SIMD processing of uniform arrays; smaller alignment padding waste.

Costs: more complex access patterns when you need all fields of one node; more arrays to manage; harder to add/remove fields.

The Zig compiler uses struct-of-arrays for its AST (`std.MultiArrayList`) and reports significant performance improvements over pointer-based trees. The Accelerated-Zig-Parser stores token lengths in a separate `u8[]` array (4x smaller than the `u32[]` offset array), exploiting the fact that almost all tokens are short.

---

## 6. Value Representation

### 6.1. NaN Boxing — 64 Bits for Everything

NaN boxing exploits the IEEE 754 double-precision format: any value with all 11 exponent bits set and a non-zero mantissa is a NaN (Not a Number). There are 2⁵² possible NaN bit patterns, but only one is needed for the canonical NaN. The remaining ~2⁵² patterns can encode pointers, integers, booleans, nil, and other tagged values — all in 64 bits.

SpiderMonkey (Firefox), LuaJIT, and JavaScriptCore use NaN boxing. The advantage: every value fits in a 64-bit word, enabling register-width operations and avoiding heap allocation for numbers. A double is stored directly; everything else is stored in the NaN mantissa bits with a type tag.

The disadvantage: extracting a pointer requires masking off tag bits, and storing a double requires checking for (and avoiding) the NaN range. Some operations need extra branches. Also, only 48-bit pointers fit (sufficient on current x86-64 hardware, but not future-proof).

### 6.2. Tagged Pointers — Low-Bit Type Tags

Tagged pointers store type information in the low bits of a pointer, exploiting alignment guarantees. If all heap objects are 8-byte aligned, the low 3 bits are always zero and can store a type tag with 8 possible values.

V8 (Chrome), Ruby (CRuby), and OCaml use tagged pointers. OCaml's convention: the low bit distinguishes integers (bit 0 = 1) from pointers (bit 0 = 0). This means OCaml integers are 63 bits, not 64 — a trade-off for O(1) type checking on every value.

### 6.3. ExBoxing — Bridging Tagged Pointers and NaN Boxing

Kannan Vijayan (SpiderMonkey team) proposed ExBoxing: use low-bit tagging (like V8) for the common case, but reserve a special tag for "extended" values that encode floating-point numbers using an exponent-biased scheme. The most commonly used doubles (small integers, common fractions) fit in the extended encoding; rare doubles fall back to heap allocation.

This captures most of NaN boxing's benefit (common numbers as immediates) while retaining tagged pointers' advantages (simpler pointer extraction, no NaN-range checks, natural null representation).

Source: https://medium.com/@kannanvijayan/exboxing-bridging-the-divide-between-tag-boxing-and-nan-boxing-07e39840e0ca

---

## 7. Register Allocation

### 7.1. Graph Coloring — The Classic Approach

Chaitin (1981) formulated register allocation as graph coloring: build an "interference graph" where nodes are live ranges and edges connect simultaneously-live ranges, then color the graph with K colors (K = number of physical registers). If a node cannot be colored, it is "spilled" to memory.

Graph coloring produces excellent code but is expensive: the interference graph can be quadratic in the number of live ranges, and coloring is NP-complete in general (though heuristics work well in practice). GCC and LLVM both use graph-coloring register allocators.

### 7.2. Linear Scan — Fast Allocation for JITs

Poletto and Sarkar (1999) proposed linear scan: number all instructions, compute live intervals [start, end] for each variable, then scan intervals left-to-right, assigning registers greedily. When all registers are occupied, spill the interval with the farthest endpoint.

Linear scan is O(N log N) — vastly faster than graph coloring. The code quality is slightly worse (1–5% more spills), but the compilation time reduction is dramatic: 15–68x faster than graph coloring in the Extended Linear Scan comparison.

Every tier-1 (baseline) JIT uses linear scan or a variant: V8's Liftoff, SpiderMonkey's baseline compiler, HotSpot's C1. The optimizing tier (C2, TurboFan, IonMonkey) uses graph coloring. The two approaches serve different points on the compile-time vs code-quality trade-off.

Source: https://web.cs.ucla.edu/~palsberg/course/cs132/linearscan.pdf

---

## 8. Error Recovery

### 8.1. Panic Mode — Skip to Synchronization Token

The simplest error recovery: when the parser encounters an unexpected token, skip tokens until reaching a "synchronization point" (typically a semicolon, closing brace, or keyword that starts a new statement). Resume parsing from there.

The advantage: trivially simple to implement, predictable behavior, prevents cascading errors. The disadvantage: potentially skips large amounts of valid code, producing a sparse AST with many missing nodes. For editor use cases where every keystroke may produce a syntax error, panic mode is too destructive — it discards too much context.

### 8.2. Tree-sitter's Parallel Error Recovery

Tree-sitter tries multiple recovery strategies simultaneously when it encounters an error: insert a missing token, delete the unexpected token, or wrap a sequence of tokens in an ERROR node. Each strategy forks the parser state, and the one that successfully parses the most subsequent input wins.

This is expensive (multiple parallel parse attempts), but the result is dramatically better than panic mode: the syntax tree preserves as much structure as possible even in the presence of errors. For syntax highlighting and code navigation, this is essential — the user needs correct highlighting for the 99% of the file that is valid, even when one line has a typo.

### 8.3. Insertion-Only Error Correction

Röhrich (1980) showed that for LL(k) and LR(k) parsers, error correction can be achieved by inserting terminal symbols to the right of the error location, never deleting. The output always corresponds to a syntactically valid program. The correction algorithm requires only one character per parser state, making it extremely cheap in space.

The insight: insertion-only correction is sufficient because any syntax error can be "fixed" by inserting the missing tokens needed to close the current syntactic context. A missing semicolon, a missing closing brace, a missing `then` keyword — all can be corrected by insertion. Deletion is needed only for truly garbage input, which is rare in practice.

### 8.4. Hazel's Approach — Typed Holes as Error Recovery

Hazel inserts "typed holes" wherever the parser encounters incomplete or missing expressions. Unlike traditional error recovery which produces error nodes, holes are semantically meaningful — they have types, and the surrounding program can still be type-checked and partially evaluated.

This turns error recovery from a hack (skip some tokens, hope for the best) into a language feature (every intermediate editor state is a valid program with holes). The trade-off: it requires the entire language and type system to be designed around the possibility of holes, which is a much larger commitment than bolting error recovery onto an existing parser.

Source: https://hazel.org/

---

## 9. Miscellaneous Techniques

### 9.1. DWARF State Machine — Compact Debug Line Info

DWARF's `.debug_line` section encodes source location information as a state machine program. The state machine has registers for address, line, column, file, etc. Opcodes advance these registers. A "special opcode" (single byte) encodes both a line delta and an address delta, covering the common case of sequential statements in compact form.

The encoding achieves remarkable density: for typical C/C++ programs, the line table is 1–5% of the text section size. For comparison, naively storing `(address, line)` pairs would be ~8 bytes per source line, while DWARF typically uses 1–2 bytes per source line.

The design principle: exploit the regularity of compiler output. Consecutive machine instructions usually correspond to consecutive or nearby source lines. Delta encoding captures this regularity.

### 9.2. JS Source Maps — VLQ Delta Encoding

JavaScript source maps (ECMA-426) encode mappings as VLQ (Variable Length Quantity) delta-encoded Base64 strings. Each segment encodes: generated column delta (reset per line), source file index delta, original line delta, original column delta, optional name index delta.

The delta encoding is key: in minified JavaScript, consecutive generated positions map to consecutive original positions (since minification reorders very little). Each delta is typically 0–2 Base64 digits. For a 1MB minified file, the source map is typically 200–500KB — smaller than the original source.

Bidirectional lookup (generated→original and original→generated) is implemented via binary search on sorted segments. This enables both "jump to source" (click on minified code, see original) and "jump to generated" (set breakpoint in source, find generated location).

### 9.3. Keyword Recognition — Perfect Hashing

Recognizing keywords during lexing can be done by string comparison against a list, by a trie, or by a perfect hash function that maps keyword strings to a compact integer range with no collisions.

`gperf` is the classic tool for generating perfect hash functions from a keyword list. The generated code is typically a single hash computation + one string comparison (to verify). This is O(1) per keyword check, with very small constant factors.

The Accelerated-Zig-Parser uses a variant: keywords and operators mapped into a 7-bit address space via a perfect hash, then verified with a single 16-byte SIMD comparison. This eliminates branching in the keyword recognition path.

For small keyword sets (<50 keywords, which covers most languages), a simple sorted array with binary search or even linear scan is often faster than a perfect hash due to better branch prediction and cache behavior. The perfect hash wins for larger sets or when the hash can be folded into SIMD processing.

### 9.4. Qualifiers in Pointer Bits

Cuik (and Clang before it) stores type qualifiers (`const`, `volatile`, `restrict`, `atomic`) in the bottom bits of type pointers. This requires types to be allocated at 16-byte alignment, ensuring the bottom 4 bits are always zero. A "qualified type" is just a pointer with some bits set — no additional allocation needed.

This trick saves enormous amounts of memory in C/C++ compilers where every expression has a qualified type. Without it, each qualified type would need a separate wrapper allocation. With it, `QualType` is the same size as a raw pointer and can be passed by value.

The general principle: if your allocations have known alignment, the bottom bits of pointers are free storage. This is used beyond compilers — tagged pointers in NaN-boxing (JavaScript engines), small integer optimization (Ruby's Fixnum), and discriminated unions (Rust's `Option<&T>` uses the null pointer niche).

### 9.5. Cuik's line_map and Go's FileSet — Position Resolution

Both Cuik and Go use the same fundamental approach for resolving byte offsets to line/column positions: a sorted array of line-start byte offsets. Given a byte position, binary search the array to find the line number (index of the greatest line-start ≤ position), then subtract the line-start from the position to get the column.

Construction is a single linear scan of the source (find all `\n` bytes, record their offsets). Resolution is O(log lines) per query. Memory is one `u32` per source line — typically 1–4KB per file.

Go's `FileSet` extends this to multiple files by giving each file a non-overlapping range in a virtual address space. A `token.Pos` from any file in the compilation can be resolved by first binary-searching the `FileSet` for the file, then binary-searching the file's line table. Two binary searches, constant per file.

---

## 10. Intermediate Representations Beyond SSA

### 10.1. CPS, ANF, and SSA — The Triad of Compiler IRs

Three IR forms dominate compiler construction, and they are deeply related:

**Static Single Assignment (SSA)**: each variable is assigned exactly once, and φ-functions at join points select values from incoming edges. Used by LLVM, GCC, Go, and almost every modern AOT compiler. SSA makes def-use chains trivial and enables powerful optimizations (constant propagation, dead code elimination, global value numbering) as direct graph operations. The dominant form for imperative language compilers.

**Continuation-Passing Style (CPS)**: every function call passes an explicit continuation — a closure representing "what to do next." Function calls never return; they instead invoke their continuation. CPS was pioneered by Steele (1978) in the Rabbit compiler for Scheme and used extensively in Appel's SML/NJ compiler. CPS makes all control flow explicit, including non-local returns, exceptions, and coroutines. Kelsey (1995) showed a formal correspondence: CPS and SSA are essentially the same — SSA's φ-functions correspond to CPS's continuation parameters, and SSA's basic blocks correspond to CPS's continuation lambdas.

**A-Normal Form (ANF)**: Flanagan et al. (1993) proposed ANF as a simpler alternative to CPS. In ANF, every intermediate result is named (let-bound), and every function argument is a trivial expression (a variable or constant). ANF captures the same sequencing guarantees as CPS without the syntactic overhead of explicit continuations. Most modern functional compilers (GHC's Core, OCaml's Flambda) use ANF or ANF-like forms.

CPS is the most expressive (it can directly represent delimited continuations, algebraic effects, and coroutines), but SSA is the most widely tooled and understood. For languages with advanced control flow (effects, async, generators), CPS or a CPS-like IR may be the natural first choice, with lowering to SSA for backend optimization. ANF is the pragmatic middle ground — simpler than CPS, more structured than SSA.

Source: https://bernsteinbear.com/assets/img/kelsey-ssa-cps.pdf and https://www.cs.princeton.edu/~appel/papers/cpcps.pdf

### 10.2. MLIR — Multi-Level Intermediate Representation

MLIR (Lattner et al., 2020) is an extensible compiler infrastructure built within the LLVM project. Its core insight: instead of a single monolithic IR (like LLVM IR), MLIR supports multiple "dialects" — each defining its own operations, types, and semantics — that coexist within the same framework and can be progressively lowered from high-level to low-level.

Key design properties:
- **SSA-based with regions**: operations can contain nested regions (like LLVM basic blocks, but hierarchically nestable), enabling natural representation of loops, conditionals, and structured control flow at any abstraction level.
- **Open type system**: dialects define their own types. A tensor dialect has `tensor<4x4xf32>`, a GPU dialect has `gpu.thread_id`, a linalg dialect has structured loop nests. No hardcoded type zoo.
- **Progressive lowering**: a high-level "linalg on tensors" operation can be lowered to "linalg on buffers," then to "scf.for loops," then to "llvm dialect," then to LLVM IR proper. Each step is a well-defined transformation between dialects.
- **Pattern-driven rewriting**: optimizations are expressed as pattern-match-and-rewrite rules (similar to e-graph rewriting but on SSA-based IR). The Transform dialect allows users to script compiler transformations externally.

MLIR was born from the AI compiler fragmentation problem — TensorFlow, PyTorch, XLA, TVM, Glow, and ONNX all invented incompatible graph IRs. Chris Lattner (who created LLVM) designed MLIR at Google to unify these efforts. It is now used in TensorFlow/XLA, IREE (for edge ML), Torch-MLIR (PyTorch bridge), Triton (GPU kernel compiler), and hardware synthesis tools.

Source: https://mlir.llvm.org/ and https://arxiv.org/abs/2202.03293

---

## 11. Lightweight Compiler Backends

### 11.1. QBE — 70% of LLVM in 10% of the Code

QBE is a compiler backend by Quentin Carbonneaux that targets the sweet spot between TCC (no optimization) and LLVM (full industrial optimizer). Written in ~14,000 lines of C99 with zero dependencies, QBE provides:
- Uniform SSA-based IL used at all compilation stages
- Copy elimination, sparse conditional constant propagation, dead instruction elimination
- Registerization of small stack slots
- Linear register allocator with hinting (split spiller + allocator, enabled by SSA)
- Full C ABI support on amd64, arm64, and riscv64

Benchmarks show QBE-compiled code runs roughly 50–70% the speed of LLVM -O2. Compilation is near-instant (~2 seconds for the compiler itself with `-O2`). The `cproc` C11 compiler (8,000 lines of C) uses QBE as its backend and can build GCC 4.7, binutils, git, and much more.

The lesson for new languages: you don't need LLVM to get reasonable codegen. QBE provides a self-contained, hackable, and understandable backend that any language can target. For a bootstrapping compiler or interpreter-to-native tier, QBE is a compelling choice.

Source: https://c9x.me/compile/

### 11.2. Cranelift & ISLE — Fast Compilation with DSL-Driven Instruction Selection

Cranelift is an optimizing compiler backend developed by the Bytecode Alliance (originally by Mozilla for Wasmtime). Unlike LLVM, Cranelift prioritizes compilation speed while maintaining reasonable code quality — making it ideal for JIT and baseline compilation tiers.

Key innovations:
- **ISLE DSL** (Instruction Selection Lowering Expressions): instruction selection rules are written in a statically-typed term-rewriting DSL, compiled to Rust match trees. This replaces hand-written C++ lowering code with declarative rules that are easier to verify, fuzz, and maintain. The ISLE compiler merges all rules into a decision tree, sharing work where possible.
- **regalloc2**: a novel register allocator by Chris Fallin that combines aspects of linear scan with SSA-aware splitting. It provides ~20% faster compilation than its predecessor while improving code quality 10–20% on register pressure-heavy benchmarks. The key insight: operate on SSA form directly, split live ranges at block boundaries, and use parallel move resolution.
- **E-graph mid-end**: Cranelift adopted e-graph-based optimization (described in section 4.4), replacing its previous peephole optimizer and solving phase-ordering problems.

Cranelift is used as the backend for Wasmtime (WebAssembly), `rustc_codegen_cranelift` (alternative Rust backend for debug builds — 2–4x faster compilation than LLVM), and several other projects. It targets x86-64, aarch64, s390x, and riscv64.

Source: https://cranelift.dev/ and https://cfallin.org/blog/2022/06/09/cranelift-regalloc2/

### 11.3. TPDE — Adaptable Single-Pass Backend Framework

TPDE (Schwarz, Kamm & Engelke, 2025) is a compiler back-end framework that adapts to existing SSA-form code representations. Instead of requiring IR translation (a significant cost when targeting LLVM), TPDE performs compilation in a single pass — combining instruction selection, register allocation, and instruction encoding — using only an IR-specific adapter.

Target instructions are derived from code written in high-level language through LLVM's Machine IR, easing portability while enabling optimizations during code generation. The authors built a new back-end for LLVM IR from scratch targeting x86-64 and AArch64, showing compilation speeds an order of magnitude faster than LLVM -O0 while producing code with comparable quality.

TPDE represents a new point in the design space: the framework approach means you can bolt fast native codegen onto any existing SSA-based IR without inventing a new backend from scratch. For a language with its own IR (like Mage's potential ZIR-equivalent), this could provide near-instant native compilation without LLVM dependency.

Source: https://arxiv.org/abs/2505.22610

### 11.4. Macroassembler JITs — DynASM

For generating machine code on the fly without the weight of a full compiler backend like LLVM or even Cranelift, macroassemblers provide a highly efficient solution. DynASM, created by Mike Pall for LuaJIT, is a prominent example.

DynASM is a C preprocessor and a tiny runtime. The developer writes assembly instructions directly interspersed with C code. A preprocessor converts these assembly lines into C macros that emit the raw machine code bytes at runtime. It avoids the overhead of intermediate representations, instruction selection passes, or register allocation algorithms, delegating those tasks to the human programmer writing the assembly. This results in JIT compilers that are extraordinarily small, fast, and capable of generating highly optimized, architecture-specific machine code with near-zero compilation latency.

---

## 12. Trace-Based JIT & Speculative Optimization

### 12.1. Trace-Based JIT Compilation — LuaJIT, PyPy

While the document covers tiered compilation (section 4.2), it describes only method-based JITs. Trace-based JITs take a fundamentally different approach: instead of compiling entire functions, they record "traces" — linear sequences of instructions actually executed through hot loops — and compile those.

**LuaJIT** (Mike Pall) is the canonical example. When a loop becomes hot, LuaJIT's interpreter records a trace of the bytecodes executed through one iteration, including inlined function calls. The trace is then compiled to machine code by a sophisticated single-pass assembler. Guards are inserted at every point where the trace could diverge (type checks, branch conditions). If a guard fails, execution falls back to the interpreter ("side exit"), which may start recording a new trace from that point.

LuaJIT achieves remarkable performance — often within 2x of optimized C — with a codebase of ~60,000 lines. The trace compiler's single-pass design means compilation is extremely fast (microseconds per trace).

**PyPy** (Bolz et al.) uses meta-tracing: instead of tracing the user program directly, PyPy traces the execution of the *interpreter* running the user program. When the interpreter enters a hot loop in user code, the meta-tracer records what the interpreter does — which bytecodes it dispatches, what type checks it performs, what allocations it makes. The resulting trace is specialized to the observed types, and optimizations (constant folding, allocation removal, unboxing) are applied.

CF Bolz-Tereick (2025) reflects on tracing JITs: they excel at optimizing across function boundaries and through deeply nested dynamic dispatch (because the trace naturally inlines everything), but struggle with methods that have many divergent paths (trace explosion). Most production JITs today (V8, SpiderMonkey, HotSpot) are method-based, but LuaJIT and PyPy demonstrate that tracing can be highly competitive for the right workloads.

Source: https://pypy.org/posts/2025/01/musings-tracing.html and https://kipp.ly/jits-impls/

### 12.2. Deoptimization & On-Stack Replacement (OSR)

Speculative optimization in JITs creates a fundamental problem: what happens when an assumption is violated while optimized code is running? Two mechanisms address this:

**On-Stack Replacement (OSR)** allows switching execution tiers while a function is mid-execution. OSR-up: when a long-running interpreted loop becomes hot, the JIT compiles it and transfers execution to the compiled version without waiting for the function to return. OSR-down (deoptimization): when a speculative assumption fails in compiled code, execution transfers back to the interpreter at the equivalent program point.

The engineering challenge: at the moment of transfer, all live variables must be mapped between the source and target representations. The interpreter and compiler may use different stack layouts, register assignments, and even different variable representations (an unboxed double in compiled code vs. a tagged value in the interpreter). D'Elia and Demetrescu (2018) formalized OSR as a general framework for transferring execution between related program versions, implemented in LLVM.

**Deoptless** (Flückiger et al., PLDI 2022) proposes replacing traditional deoptimization points with dispatched specialized continuations. Instead of falling back to the interpreter on guard failure, the system dispatches to a separately compiled continuation specialized for the failing case. This provides a more transparent performance model — no mysterious slowdowns from deoptimization storms.

Barrière et al. (POPL 2021) formally verified speculation and deoptimization in a JIT compiler, proving that the observable behavior of speculative execution matches the source semantics — a critical correctness property that is notoriously hard to get right.

Source: https://season-lab.github.io/papers/osr-distilled-pldi18.pdf and https://janvitek.org/pubs/pldi22.pdf

---

## 13. Domain-Specific & AI-Oriented Compilation

### 13.1. Polyhedral Compilation — Loop Nest Optimization

Polyhedral compilation represents loop nests and array accesses as parametric integer polyhedra, enabling mathematically precise reasoning about dependencies, parallelism, and data locality. For a language targeting AI workloads — which are dominated by nested loops over tensors — polyhedral techniques are directly relevant.

The core idea: a loop nest like `for i in 0..N: for j in 0..M: A[i][j] = B[j][i]` is modeled as an iteration domain (the set of (i,j) points), an access relation (mapping iterations to array elements), and a schedule (mapping iterations to execution times). Transformations like tiling, interchange, fusion, skewing, and parallelization are expressed as affine transformations of the schedule — and the polyhedral framework can automatically verify that these transformations preserve program semantics.

**Halide** (Ragan-Kelley et al., 2012) introduced the influential separation of algorithm from schedule. The programmer writes what to compute (the algorithm), and separately specifies how to compute it (the schedule — tiling factors, parallelism, vectorization, storage). This decoupling allows exploring optimization spaces without modifying the algorithm, and enables auto-tuning.

**Tiramisu** extends polyhedral compilation with the full power of the model (including skewing for RNN optimization), achieving 2x speedup over TVM on recurrent architectures.

**Triton** (Tillet et al.) takes a different approach: instead of polyhedral analysis, it provides block-level programming abstractions for GPU kernels, letting the compiler handle memory coalescing and scheduling within blocks. Triton has become the dominant way to write custom GPU kernels for PyTorch.

Source: http://polyhedral.info/ and https://triton-lang.org/main/programming-guide/chapter-2/related-work.html

### 13.2. Automatic Differentiation at the Compiler Level — Enzyme

Automatic Differentiation (AD) computes exact derivatives of programs by applying the chain rule systematically. While frameworks like PyTorch and JAX provide AD through operator overloading or tracing, **Enzyme** (Moses & Churavy, 2020) operates at the LLVM IR level — differentiating compiled code rather than source code.

Enzyme's approach is uniquely powerful:
- **Language-agnostic**: because it operates on LLVM IR, it can differentiate programs written in C, C++, Rust, Julia, Fortran, Swift, or any LLVM-targeting language.
- **Optimization-friendly**: AD happens after LLVM optimization passes, meaning the derivative code benefits from the same optimizations as the original. Conversely, the derivative itself can be further optimized. Moses showed that performing AD after optimization can be orders of magnitude faster than AD before optimization.
- **GPU support**: Enzyme can differentiate CUDA kernels, generating reverse-mode gradients of parallel GPU code — the first fully automatic AD tool to do so.

The alternative approaches:
- **Source-to-source** AD (Tapenade, ADIFOR) rewrites source code to produce gradient functions. Requires all code to be available and analyzable at the source level.
- **Operator overloading** AD (JAX, Adept) provides differentiable versions of language primitives. Requires rewriting code to use non-standard types.
- **DSL-based** AD (TensorFlow, PyTorch) defines computation in a differentiable graph language. Restricted to the operations the DSL supports.

Source: https://enzyme.mit.edu/ and https://c.wsmoses.com/papers/EnzymeGPU.pdf

---

## 14. Advanced Memory Management

### 14.1. Perceus — Garbage-Free Reference Counting with Reuse

Perceus (Reinking, Xie, de Moura & Leijen, PLDI 2021) is a reference counting algorithm for the Koka language that achieves a remarkable property: cycle-free programs are **garbage-free** — only live references are ever retained, and objects are freed at the exact moment they become unreachable. No GC pauses, no scanning, no root sets.

The algorithm works on a functional core with explicit control flow. Perceus inserts precise `dup` (increment) and `drop` (decrement) operations using a linear resource calculus, ensuring that every allocation is matched by exactly one deallocation on every execution path.

The breakthrough is **reuse analysis**: when an object is about to be freed (its last reference is dropped) and a new object of the same size is about to be allocated, Perceus can guarantee in-place mutation — the old memory is reused for the new allocation without going through the allocator at all. This enables a paradigm called **FBIP (Functional But In-Place)**: programmers write purely functional code (no mutation), but the compiler generates in-place updates wherever possible.

Example: a balanced binary tree insertion written in pure functional style compiles to code that mutates tree nodes in-place when no other references exist — matching the performance of hand-written imperative code, with the safety guarantees of immutability.

**Frame-Limited Reuse** (Lorenzen & Leijen, ICFP 2022) extends Perceus with a drop-guided reuse algorithm that is more robust to program transformations and prevents pathological heap growth.

Source: https://www.microsoft.com/en-us/research/wp-content/uploads/2020/11/perceus-tr-v1.pdf

### 14.2. Region-Based Memory Management — MLKit, Cyclone

Region-based memory management groups allocations into regions (arenas with compile-time-determined lifetimes). The compiler's region inference algorithm statically determines which region each allocation belongs to, and all objects in a region are freed together when the region's scope ends.

**MLKit** (Tofte & Talpin, 1994) pioneered region inference for Standard ML. The compiler analyzes the program's data flow and assigns each allocation to a region whose lifetime is provably sufficient. No programmer annotations are needed — the compiler infers everything. MLKit later integrated region inference with generational garbage collection (Elsman & Hallenberg, 2021), showing that the combination is often superior to either approach alone: regions handle the common case (short-lived data freed in bulk), and GC handles the pathological case (long-lived data with complex sharing patterns).

**Cyclone** (Grossman et al., 2002) brought region-based management to a C-like language. Cyclone programmers annotate pointers with region lifetimes (similar to Rust's lifetimes), and the type system statically prevents dangling pointers. Cyclone demonstrated that porting C programs required altering ~8% of code, of which only 6% (of the 8%) were region annotations — a modest annotation burden for complete memory safety.

The connection to Rust: Rust's ownership and borrowing system is directly descended from region-based type systems. The key difference is that Rust uses affine types (values can be used at most once) to enable compile-time deallocation without region inference, while MLKit uses region inference to avoid annotations.

The design spectrum: explicit lifetimes (Rust-style, more control, more annotation), inferred regions (MLKit-style, less annotation, less control), or reference counting (Perceus-style, zero annotation, runtime cost). All three avoid garbage collection pauses.

Source: https://elsman.com/mlkit/ and https://www.cs.umd.edu/projects/cyclone/papers/cyclone-regions.pdf

---

## 15. Runtime Object Model Optimization

### 15.1. Hidden Classes & Inline Caching — V8, SpiderMonkey

For dynamically typed languages, property access on objects (`obj.field`) is potentially an expensive hash table lookup. Hidden classes (called "Shapes" in SpiderMonkey, "Maps" in V8, "Structures" in JavaScriptCore) solve this by imposing static-like structure on dynamic objects.

When an object is created, the engine assigns it a hidden class describing its layout — which properties exist and at what memory offsets. When a property is added, a new hidden class is created (or an existing one is reused via transition chains). Objects with the same hidden class have identical memory layouts, enabling fixed-offset access instead of hash lookups.

**Inline caching (IC)** exploits hidden classes at call sites. The first time `obj.x` is executed, the engine looks up the property, records the hidden class and the offset, and patches the call site with a fast path: "if the object's hidden class is C, load from offset 16." Subsequent executions hit the fast path in O(1). If different hidden classes are seen at the same call site:
- **Monomorphic IC**: one hidden class — fastest, single comparison + direct load.
- **Polymorphic IC**: 2–4 hidden classes — linear search through a small table.
- **Megamorphic IC**: many classes — falls back to generic hash lookup.

The performance cliff is dramatic: monomorphic access can be 60–100x faster than megamorphic. This is why V8 and SpiderMonkey invest heavily in hidden class stability analysis, and why "initialize all properties in the constructor" is critical JavaScript performance advice.

Source: https://v8.dev/docs/hidden-classes and https://mrale.ph/blog/2015/01/11/whats-up-with-monomorphism.html

### 15.2. Deoptimization Guards & Speculative Type Specialization

Beyond inline caching, JIT compilers perform speculative optimization: the compiler observes that a variable is "always" an integer and generates optimized integer arithmetic, guarded by a type check. If the guard fails (the variable is suddenly a string), the engine deoptimizes — throwing away the compiled code and falling back to the interpreter.

The key patterns:
- **Type specialization**: compile arithmetic assuming integer operands; guard and deopt if float/string appears.
- **Shape guards**: compile property access assuming a specific hidden class; deopt if the object's class changes.
- **Bounds check elimination**: prove that array indices are in bounds; remove runtime checks.
- **Allocation sinking/scalar replacement**: if an object doesn't escape a function, replace it with stack-allocated fields ("virtual object"). Guard and deopt if escape is detected.

V8's TurboFan, SpiderMonkey's IonMonkey, and HotSpot's C2 all use these techniques extensively. The challenge is managing the deoptimization cost: if assumptions are wrong too often, the program spends more time deoptimizing than executing. Adaptive recompilation (recompile with fewer assumptions after repeated deopt) is the standard mitigation.

---

## 16. Incremental & Query-Based Compilation

### 16.1. Query-Based Compilation — rustc, Salsa, rust-analyzer

Traditional compilers execute a fixed sequence of passes (parse → type-check → lower → optimize → codegen). Query-based compilers invert this: computation is organized as a set of memoized functions ("queries") that compute results on demand. When a query is invoked, the system checks if a valid cached result exists; if not, it computes the result and caches it, tracking which inputs were read.

**rustc** is transitioning to a query-based architecture. Instead of running type inference as a monolithic pass, the compiler defines queries like `type_of(DefId) → Type`, `mir_built(DefId) → Mir`, `codegen_unit(Symbol) → CodegenUnit`. Each query is computed on demand and memoized. When the user changes a file, the system invalidates only the queries whose inputs changed, recomputing the minimum necessary.

**Salsa** (Matsakis et al.) is the incremental computation framework used by rust-analyzer. It provides:
- **Automatic dependency tracking**: queries automatically track which other queries they read; no manual dependency declarations needed.
- **Shallow verification**: when an input changes, Salsa checks if the query's result actually changed before invalidating dependent queries. This "green" verification prevents cascading recomputation.
- **Cycle detection**: handles mutually recursive queries gracefully.
- **Parallel execution**: Salsa 2.0+ supports parallel query evaluation, enabling parallel autocomplete and diagnostics in rust-analyzer.

The latest Salsa version (used in rust-analyzer since 2024–2025) supports persistent caches, near-instant crate graph updates, and parallel completion. The migration yielded major performance wins: David Barsky and Lukas Wirth reported significant improvements in rust-analyzer responsiveness.

Source: https://rustc-dev-guide.rust-lang.org/query.html and https://github.com/salsa-rs/salsa

---

## 17. Type System & Effect System Implementation

### 17.1. Bidirectional Type Checking

Bidirectional typing (Pierce & Turner, 2000; surveyed by Dunfield & Krishnaswami, 2021) splits the typing judgment into two modes: **synthesis** (infer the type from the expression) and **checking** (verify the expression against a known type). The type information flows "down" in checking mode and "up" in synthesis mode.

The practical benefits:
- **Reduced annotations**: the programmer annotates function signatures, and the checker propagates type information inward. Lambda parameters, match arms, and return expressions can often omit types entirely.
- **Supports undecidable features**: higher-rank polymorphism, dependent types, and GADTs have undecidable type inference, but decidable type checking. Bidirectional typing supports these features without requiring full inference — the user provides annotations where inference fails, and the checker handles the rest.
- **Better error messages**: because the checker knows the "expected" type at every point, error messages can report mismatches precisely: "expected `Int`, got `String`" rather than "failed to unify `?a` with `?b`."
- **Incremental-friendly**: Porter et al. (2025) showed how bidirectional typing can be made incremental via order maintenance data structures, enabling live type-checking that updates in real-time as the programmer edits.

GHC (Haskell), Agda, Idris, and Lean all use bidirectional type checking. Bidirectional typing is the modern standard for balancing type inference convenience with expressive power.

Source: https://dl.acm.org/doi/fullHtml/10.1145/3450952

### 17.2. Algebraic Effects & Effect Handlers — Compilation Techniques

Algebraic effects (Plotkin & Power, 2001) provide a structured way to express side effects (I/O, state, exceptions, async, concurrency) as operations that are "performed" by the program and "handled" by an enclosing handler — analogous to `throw`/`catch` but vastly more general. Effect handlers can resume the computation after handling (unlike exceptions), enabling coroutines, generators, and cooperative concurrency as library-level abstractions.

The compilation challenge: a naive implementation uses delimited continuations, which require capturing the call stack — expensive in time and space. Efficient compilation strategies include:

- **Evidence passing** (Xie & Leijen, ICFP 2021): Koka compiles effect handlers to plain C by threading "evidence" (handler references) as extra function parameters. When an effect operation is performed, the runtime uses the evidence to find the handler and invoke it. This avoids continuation capture for the common case (tail-resumptive handlers).
- **Multi-prompt delimited control** with **yield bubbling**: effect operations yield control up the stack frame by frame, avoiding full continuation capture. Only when a handler needs to suspend and later resume does a real continuation need to be allocated.
- **Multicore OCaml**: implements effect handlers using stack switching with fiber-based continuations, leveraging the system's ability to cheaply allocate and switch between small stacks.

The performance is real: Koka with evidence-passing compilation matches or beats OCaml on many benchmarks, while providing algebraic effects as a first-class feature. Combined with Perceus reference counting (section 14.1), Koka achieves garbage-free, effect-typed, functional programming with C-level performance characteristics.

Source: https://xnning.github.io/papers/multip.pdf and https://koka-lang.github.io/koka/doc/book.html

---

## 18. WebAssembly Compilation Techniques

### 18.1. Streaming & Lazy Compilation

WebAssembly's binary format was designed from the ground up for fast compilation. Several techniques exploit this:

- **Streaming compilation**: V8's `WebAssembly.compileStreaming()` begins compiling WebAssembly functions as bytes arrive over the network, before the entire module is downloaded. The binary format places function bodies after the type and import sections, so the compiler knows all signatures before it encounters any function body. This enables compilation to proceed in parallel with download.
- **Lazy compilation**: V8 does not compile all functions eagerly. Instead, functions are compiled on first call by the baseline compiler (Liftoff). This avoids compiling functions that are never called — common in large modules that export many unused functions.
- **One-pass validation and compilation**: WebAssembly's structured control flow (no arbitrary `goto`) and stack machine design enable single-pass validation. The validator maintains a type stack and checks each instruction in sequence — O(n) time, O(1) state per instruction. Liftoff exploits this by compiling during the same single pass: each WebAssembly instruction is immediately translated to machine code.
- **Lazy validation** (proposed, V8): defer function body validation until the function is first called. Combined with lazy compilation, this means the engine pays no compile-time or validation cost for unused functions.

The design lesson: if a language's binary format is designed for streaming, single-pass compilation, the cold-start latency can be dramatically reduced. WebAssembly's structured control flow is the key enabler — it sacrifices `goto` but gains compilability. For a language targeting both native and Wasm compilation, this trade-off is worth understanding.

Source: https://v8.dev/docs/wasm-compilation-pipeline

---

## 19. Case Studies — Ruff & ty (Astral)

### 19.1. Ruff — Generated Parser to Hand-Written Recursive Descent

Ruff is an extremely fast Python linter and formatter written in Rust. Its parser evolution illustrates a recurring pattern in tooling: starting with a generated parser and migrating to a hand-written one for performance and control.

Ruff initially used the RustPython parser, then a LALRPOP-generated parser. In v0.4.0 (April 2024), it switched to a hand-written recursive descent parser, yielding **>2x faster parsing** and **20–40% overall speedup** for all linting and formatting. On micro-benchmarks, the hand-written parser achieved 2.2–2.4x speedup per file.

The motivations mirror broader trends:
- **Control and flexibility**: Python has syntactic ambiguities (e.g., parenthesized `with` items) that are awkward to encode in a parser generator's grammar DSL but straightforward in hand-written code.
- **Performance**: the generated parser was a black box — hot paths and cold paths couldn't be distinguished, and domain-specific optimizations were impossible. The hand-written parser allows fine-grained control over allocation, lookahead, and branch prediction.
- **Error recovery**: a hand-written parser can implement context-sensitive error recovery (inserting missing colons, recovering from invalid assignment targets) that a generated parser cannot express. Ruff now produces structured error messages like "Expected 'def', 'with' or 'for' to follow 'async', found 'while'" instead of generic "Unexpected token" errors.
- **Error resilience for editors**: since Ruff runs as an editor tool, it must produce useful results on syntactically invalid code. The hand-written parser lays the foundation for continuing analysis past syntax errors — critical for the language server use case.

The broader lesson: parser generators (LALRPOP, yacc, ANTLR) are excellent for bootstrapping but become constraints as a tool matures. Every major production compiler and toolchain (GCC, Clang, Go, Rust, V8, Zig) uses a hand-written parser. The flexibility to implement custom error recovery, incremental reparsing, and performance-critical optimizations outweighs the convenience of grammar-driven generation.

Source: https://astral.sh/blog/ruff-v0.4.0

### 19.2. ty — Salsa-Based Incremental Type Checking

ty (formerly "Red Knot") is Astral's Python type checker and language server, written in Rust. It type-checks the `home-assistant` project in 2.19 seconds — 8.9x faster than mypy (19.6s), 20.8x faster than Pyright (45.7s). In incremental mode (editing a file in the PyTorch repository), ty recomputes diagnostics in 4.7ms — 80x faster than Pyright.

The architecture is built on **Salsa** (the same incremental computation framework used by rust-analyzer, described in section 16.1). Key design decisions:

- **Incremental from the ground up**: the entire type checker is structured as Salsa queries. Parsing, name resolution, type inference, and diagnostic emission are all memoized, demand-driven computations. When a file changes, only the queries that transitively depend on it are recomputed. This is what enables 4.7ms incremental updates on a multi-million-line codebase.
- **First-class intersection types**: unlike mypy and Pyright, ty supports intersection types natively. This enables more precise type narrowing — after `isinstance(x, A)` in a branch where `x: A | B`, ty narrows to exactly `A` rather than approximating. Intersection types also enable more accurate modeling of Python's runtime type system (multiple inheritance, protocol composition).
- **Advanced type narrowing and reachability analysis**: ty performs control-flow-sensitive type narrowing (including `hasattr` narrowing) and uses type information to detect unreachable code — going beyond what traditional type checkers infer.
- **Diagnostic system inspired by rustc**: ty produces multi-file, multi-span diagnostics that explain not just *what* is wrong but *why*, pulling context from declarations in other files. The diagnostic output is designed for both humans and AI agents.
- **Gradual typing with gradual guarantee**: ty avoids false positives on untyped code. Partially typed codebases receive appropriate treatment rather than a flood of errors — critical for adoption in existing projects.

ty demonstrates that combining Salsa-based incrementality with a Rust implementation creates a new performance tier for language tooling: cold-start performance that matches or exceeds cached performance of existing tools, and incremental performance that enables real-time editor feedback on the largest codebases.

Source: https://astral.sh/blog/ty

---

## 20. Summary of Techniques

| Technique | Space Cost | Time Cost | Key Trade-off | Examples |
|---|---|---|---|---|
| Span per AST node (lo+hi) | 8–12 bytes/node | O(1) access | Memory vs convenience | rustc, swc |
| Single position per node | 4 bytes/node | O(1) access, infer end from structure | Half the span data | Go `token.Pos` |
| Bit-packed position | 4 bytes/node | O(1) access | File count/size limits | Cuik |
| Width-only green nodes | 0 bytes/node | O(depth) to resolve | Enables incremental reparse | Roslyn, rowan |
| Token-indexed AST | 4 bytes/node | O(1) via token table | Source can be freed after IR gen | Zig |
| Bytecode-to-source side table | 2–10 bytes/entry | O(log N) lookup | Separate from code, optional | JVM, DWARF, JS Source Maps |
| Delta/RLE line info | ~1 byte/line | O(N) decode, O(log N) search | Compact but sequential decode | DWARF, Lua Compact Debug |
| Pratt parsing | O(1) per operator | Drives parse via binding power | Only for expressions | Most hand-written parsers |
| PEG/packrat | O(input × rules) memo table | O(input) guaranteed | Memory for time | pegen (CPython 3.9+) |
| Incremental LR (tree-sitter) | Full syntax tree | O(edit size) reparse | Always-valid tree, even with errors | Neovim, Helix, Zed |
| SIMD tokenization | Sentinel padding needed | ~2.75x faster than scalar | Architecture-specific | Accelerated-Zig-Parser |
| Copy-and-patch stencils | Stencil library (~MB) | memcpy + patch per instruction | Compilation speed vs code quality | CPython 3.13 JIT |
| Arena allocation | One large region | ~2ns per allocation | No individual free | bumpalo, every compiler |
| String interning | Hash table + buffer | One hash per new string | O(1) equality after intern | rustc Symbol, V8 |
| Hash consing | Hash table + structure | One hash per construction | O(1) structural equality | BDDs, type representations |
| Struct-of-arrays | Parallel arrays | Better cache for columnar access | Worse for per-node access | Zig AST, ECS |
| Qualifiers in pointer bits | 0 extra bytes | Mask on access | Requires aligned allocation | Cuik, Clang |
| Insertion-only error correction | 1 byte per parser state | Per-error-site | Always produces valid output | Röhrich (1980) |
| Typed holes | Language-level feature | Per-hole type inference | Requires language co-design | Hazel |
| Sea of Nodes IR | Graph structure | Enables global optimization | Complex to implement/debug | HotSpot C2, V8 TurboFan |
| E-graph optimization | Equivalence classes | Avoids phase-ordering | Memory for explored rewrites | Cranelift, egg |
| GLL parsing | GSS + SPPF | O(n³) worst, O(n) unambiguous | Handles all CFGs, no restrictions | Iguana |
| Scannerless parsing | Single grammar | Slower (more ambiguity) | Composable grammars, no lexer hack | SGLR, Rascal |
| Futamura projection | Interpreter + PE | JIT compilation cost | Compiler from interpreter for free | Truffle/Graal |
| NaN boxing | 0 extra bytes | Mask/check on access | 48-bit pointer limit | SpiderMonkey, LuaJIT |
| Tagged pointers | 0 extra bytes | Mask on access | Reduced integer range | V8, OCaml, Ruby |
| Graph coloring regalloc | Interference graph | NP-complete (heuristic) | Best code, slow compilation | GCC, LLVM |
| Linear scan regalloc | Live intervals | O(N log N) | 15–68x faster, slightly worse code | V8 Liftoff, HotSpot C1 |

---

| CPS IR | Explicit continuations | All control flow explicit | Syntactic overhead | SML/NJ, Chez Scheme |
| ANF IR | Let-bound intermediates | Simpler than CPS, same power | Less expressive control | GHC Core, OCaml |
| MLIR multi-level IR | Dialect per abstraction | Progressive lowering, composable | Learning curve, framework weight | TensorFlow, Triton, IREE |
| QBE backend | ~14K lines C99 | ~50–70% LLVM perf, instant compile | Limited targets, fewer opts | cproc |
| Cranelift + ISLE | DSL-driven lowering | Fast compile, e-graph mid-end | Less mature than LLVM | Wasmtime, rustc debug |
| TPDE single-pass | Adapter-based framework | 10x faster than LLVM -O0 | No cross-function opts | LLVM IR fast mode |
| Trace-based JIT | Trace recording | Auto-inlining, cross-function opt | Trace explosion on branchy code | LuaJIT, PyPy |
| OSR / Deoptimization | Stack frame mapping | Tier switch mid-execution | Complex frame reconstruction | V8, HotSpot, SpiderMonkey |
| Lezer parser | Compact JS tree | Incremental LR | No Wasm overhead | CodeMirror 6 |
| Nanopass framework | AST boilerplate | Fast single-task passes | Formal ILs per pass | Chez Scheme |
| Surgical monomorphization | Lambda set tags | Compile-time defunctionalization | Avoids code bloat | Roc |
| Macroassembler JIT (DynASM) | Direct code emission | Zero IL overhead | Human handles regalloc | LuaJIT |
| SIMD Structural Parsing | Two-pass index generation | Parses at RAM speed | Memory bandwidth bottleneck | simdjson, simdcsv |
| Parsing with Derivatives | Brzozowski's extension | Parses arbitrary CFGs elegantly | Requires memoization/laziness | functional parsers |
| Dynamic Superinstructions | On-the-fly code stitching | Eliminates dispatch overhead | Basic template copying | GForth |
| Polyhedral compilation | Parametric polyhedra | Optimal loop tiling/fusion | Restricted to affine loops | Halide, Tiramisu, Pluto |
| Enzyme AD | LLVM IR differentiation | Language-agnostic, GPU support | Requires LLVM integration | Julia, C/C++, Rust |
| Perceus ref counting | Linear resource calculus | Garbage-free, in-place reuse | No cycles without extension | Koka |
| Region-based memory | Region inference | Bulk deallocation, no GC pauses | Less control than ownership | MLKit, Cyclone |
| Hidden classes / IC | Shape transition chains | 60–100x faster monomorphic access | Megamorphic cliff | V8, SpiderMonkey, JSC |
| Query-based compilation | Memoized demand-driven | Minimal recompilation | Architectural complexity | rustc, Salsa, rust-analyzer |
| Bidirectional typing | Synth + check modes | Reduced annotations, better errors | Undecidable full inference still | GHC, Agda, Lean |
| Algebraic effects | Evidence-passing handlers | Composable effects, no monads | Requires language co-design | Koka, Multicore OCaml |
| Wasm streaming compile | Structured binary format | Compile during download | No arbitrary goto | V8, SpiderMonkey |
| Hand-written recursive descent | Direct Rust/C code | 2x+ faster than generated, full control | More code to maintain | Ruff, GCC, Clang, Go, Rust |
| Salsa incremental type checking | Memoized query graph | 80x faster incremental vs Pyright | Requires query-based architecture | ty, rust-analyzer |

---


## 20A. Checked Corrections and Caveats

### 20A.1. CPython / pegen date correction

The summary table should say **`pegen (CPython 3.9+)`**, not `3.12+`. PEP 617 shipped the PEG parser in Python 3.9, with the new parser enabled by default in 3.9 and the old LL(1) parser removed in 3.10. This matters because pegen is not a late add-on or experimental side path — it is the parser architecture CPython has already been living with for several releases.

Source: https://peps.python.org/pep-0617/ and https://docs.python.org/3/whatsnew/3.9.html

### 20A.2. Tree-sitter precision note

Calling tree-sitter simply “GLR” is directionally correct, but in practice the generated parser behaves like an LR parser most of the time and invokes GLR exploration when the grammar declares a runtime conflict. That makes it different in feel from always-generalized systems like GLL or Marpa: tree-sitter gets editor-grade speed partly by keeping its generalized machinery selective rather than universal.

Source: https://tree-sitter.github.io/tree-sitter/creating-parsers/2-the-grammar-dsl.html

## 20B. Additional Implementations Worth Adding

### 20B.1. langcc — XLR Parser Generation for Full Front-Ends

Joe Zimmerman’s **langcc** is one of the few serious attempts to make parser generation competitive again for full industrial languages. The interesting claim is not just “another LR generator,” but that it extends canonical LR with several implementation ideas — including grammar transformations, per-symbol attributes, recursive-descent actions, and **XLR**, an extension that adds bounded nondeterministic choice to shift/reduce parsing. It also generates much more than a parser: AST types, traversals, hashing, and pretty-printers.

The really original side is the developer ergonomics around conflicts. Rather than dumping opaque shift/reduce tables, langcc includes a **conflict tracing** story that tries to map LR conflicts back to explicit confusing input pairs. That is exactly the sort of feature parser generators historically needed and mostly failed to provide.

**Pros:** grammar-first workflow, full frontend generation, unusually strong performance claims for an automatic generator, conflict diagnosis taken seriously.  
**Cons:** research-tool ecosystem, much less battle-tested than Menhir/tree-sitter/ANTLR, fewer production case studies.

Source: https://langcc.io/ and https://arxiv.org/abs/2209.08383

### 20B.2. Marpa — Earley/Leo Parsing with “Ruby Slippers” Recovery

**Marpa** is Jeffrey Kegler’s practical general parser in the Earley/Leo family. The marquee property is that it aims to parse any BNF grammar exactly, without forcing arbitrary conflict resolution, while still achieving linear-time behavior on large practical classes of grammars. Unlike PEG, it is not based on ordered choice, so it keeps the exact CFG meaning instead of embedding parsing policy into the grammar.

The implementation twist that deserves a place in this document is **Ruby Slippers parsing**. Instead of only recovering *after* the parser has fallen over, Marpa can expose what it is expecting and let the surrounding application inject or adjust tokens to keep the parse moving. This turns error recovery into a programmable interface rather than a fixed parser-generator afterthought.

**Pros:** exact CFG semantics, left/right/middle recursion all fine, programmable recovery/event model, parse-forest-friendly.  
**Cons:** smaller ecosystem, more unusual mental model than recursive descent or LR, ambiguity management pushes complexity into later phases.

Source: https://jeffreykegler.github.io/Marpa-web-site/ and https://jeffreykegler.github.io/Ocean-of-Awareness-blog/individual/2011/11/marpa-and-the-ruby-slippers.html

### 20B.3. Menhir — Industrial LR(1) with Incremental, Inspection, and Unparsing APIs

**Menhir** is usually introduced as “the modern OCaml yacc,” but that undersells the implementation ideas. In `--table` mode it exposes an **incremental API** where parser states are persistent data structures, so parsing can stop at token boundaries and resume later from saved checkpoints. This is exactly the right substrate for live parsing, IDE integration, and custom recovery.

The more unusual part is that Menhir goes further with an **inspection API** (introspect parser states and stacks) and an **unparsing API** designed to help generate correct text back from ASTs. This is a rarer combination than it should be: most parser generators stop at “produce an AST,” while Menhir explicitly acknowledges editor/tooling round-trips.

**Pros:** strong theory, practical LR(1), persistent incremental states, unusually good tooling hooks, serious error-message infrastructure.  
**Cons:** OCaml-centric, generated tables/code can be larger with advanced APIs enabled, GLR mode currently drops some of these APIs.

Source: https://gallium.inria.fr/~fpottier/menhir/manual.html

### 20B.4. Ungrammar — Generate the Concrete Syntax Tree API, Not the Parser

rust-analyzer’s **ungrammar** is not a parser generator at all, and that is exactly why it is interesting. It specifies the **shape of the concrete syntax tree** as a schema and generates the typed API for navigating that tree. The parser itself can stay hand-written or otherwise independently engineered.

This addresses a real pain point: the grammar shape you need for parsing is often not the tree shape you want to expose to tools. Left-recursion elimination, precedence encoding, and recovery scaffolding distort parse trees. Ungrammar decouples “how do I parse strings?” from “what typed tree API do I want clients to use?”

**Pros:** clean separation of parser mechanics from CST API design, ideal for IDE/tooling stacks, works well with lossless trees.  
**Cons:** not a parser, adds another specification layer, most useful only once you already care about a typed CST API.

Source: https://rust-analyzer.github.io/blog/2020/10/24/introducing-ungrammar.html

### 20B.5. Oil / OSH / YSH — Lossless Syntax Trees via ASDL

The Oil shell project explicitly moved **from AST to Lossless Syntax Tree**. That is a valuable data point because shells are exactly the sort of language where comments, trivia, token boundaries, and syntactic oddities matter for refactoring and tooling. Oil’s argument is that one parser may need to serve both execution and source-to-source tooling, and the representation should preserve enough syntax to support both.

This sits in an interesting middle ground between classic ASTs and green/red trees: not every implementation needs Roslyn-style persistence, but many do need a tree that preserves enough original structure to round-trip and to support precise diagnostics.

**Pros:** practical reminder that “AST” is often too lossy for tools, ASDL gives a compact schema language, especially relevant for shell-like grammars.  
**Cons:** more syntax noise retained, larger trees, more work if the runtime only needs a simplified executable form.

Source: https://www.oilshell.org/blog/2017/02/11.html

### 20B.6. Factor — Self-Hosting Optimizing Compiler for a Stack Language

**Factor** is one of the best counterexamples to the idea that concatenative languages must choose between elegance and serious compilation. The compiler is written in Factor, lowers stack code into SSA-based IR, and then applies a very recognizable optimizing pipeline: type inference, sparse conditional constant propagation, generic-dispatch elimination, escape analysis, scalar replacement, value numbering, representation selection, instruction scheduling, and linear-scan register allocation.

What makes Factor especially worth adding here is that it shows how a compiler can start from a stack-effect language and still end up with a modern register-oriented optimizer without pretending the source language was expression-oriented all along. The **stack checker** is also part of the compiler story: abstract interpretation of stack effects is doing real frontend work, not just linting.

**Pros:** self-hosting, strong optimizer, elegant demonstration that concatenative languages can have industrial compiler architecture.  
**Cons:** niche ecosystem, less relevant if the reader cares only about conventional syntax/languages.

Source: https://factorcode.org/slava/dls.pdf and https://concatenative.org/wiki/view/Factor/Optimizing%20compiler

## 20C. Fast Forth Implementation Spectrum

Forth needs a different comparison frame from most compiler literature. “Fastest Forth” can mean at least three different things:

1. **fastest threaded-code engine** (interpreter/VM core),  
2. **fastest desktop native-code compiler**, or  
3. **fastest tiny embedded native-code system**.

There is no universally accepted public benchmark suite that cleanly settles all three at once, so it is better to describe the design space than to crown a single winner.

### 20C.1. Gforth — The Strong Conservative Pick for Fast Open Threaded Code

If the question is “what is the strongest open, portable, well-documented Forth implementation with a fast traditional engine?”, **Gforth** is the obvious answer. The Gforth manual states that on RISC machines its engine is “very close to optimal” for threaded-code execution, and the system layers in **dynamic superinstructions** and later **stack caching** as the major performance levers.

Gforth matters beyond Forth because it is one of the cleanest long-running laboratories for VM implementation techniques: direct/indirect threading hybrids, superinstruction formation, stack caching, and careful benchmarking.

**Pros:** open source, ANS-oriented, extremely instructive implementation, strongest documented open threaded engine.  
**Cons:** if you want absolute peak native-code performance, good commercial/native-code Forths usually outrun it.

Source: https://gforth.org/manual/Performance.html and https://www.complang.tuwien.ac.at/forth/gforth/Docs-html/Dynamic-Superinstructions.html

### 20C.2. VFX Forth — The Strong Conservative Pick for Peak Desktop Performance

If the question is “which Forth is usually named when people want the *fastest desktop production compiler*?”, the conservative answer is **VFX Forth**. MPE’s own material claims it has long been the fastest Windows Forth, emphasizes **native code generation**, **aggressive stack-traffic optimization**, and **inlining**, and says the result gets within roughly 25% of hand-written assembler on their published examples.

This should be read as a **vendor claim**, not a neutral standards-body ranking, but it lines up with the reputation VFX has in practitioner discussions: if peak speed matters and commercial tooling is acceptable, VFX is usually in the first sentence.

**Pros:** native code, serious optimizer, big-codebase credibility, likely best answer for “fastest serious Forth compiler on desktop.”  
**Cons:** commercial/closed, public benchmark evidence is uneven and often vendor-hosted.

Source: https://www.mpeforth.com/software/pc-systems/vfx-forth-common-features/ and https://vfxforth.com/

### 20C.3. SwiftForth — Subroutine Threading with Direct Code Substitution

**SwiftForth** takes a more transparent path: it is a **subroutine-threaded** system that substitutes direct code where possible and supports inline expansion for words whose headers mark them as inlineable. This gives it a strong reputation as a very fast desktop Forth without requiring the full optimizer mystique of VFX.

The implementation angle is worth calling out because subroutine threading is often dismissed as “just compile calls,” but SwiftForth shows how much mileage you can get once you combine that with direct substitution and selective inlining.

**Pros:** clear execution model, fast in practice, commercially supported, good exemplar of subroutine-threaded design.  
**Cons:** less public detail about deep optimizer internals than VFX/Gforth papers, commercial.

Source: https://www.forth.com/swiftforth/

### 20C.4. Mecrisp / Mecrisp-Stellaris — Tiny Embedded Native Code with Direct-to-Flash Compilation

For microcontrollers, **Mecrisp** and **Mecrisp-Stellaris** sit in a different league from desktop Forths. They compile **directly into flash**, perform **constant folding**, and in the newer RA compiler path add **automatic inlining** and even **register allocation for the data stack**. This is a notably elegant answer to embedded constraints: keep the environment interactive, but compile straight to native code in tiny memory budgets.

The design is especially original because it is not merely “a small Forth.” It combines tiny-system practicality with real compiler behavior instead of falling back to a purely interpreted core.

**Pros:** excellent embedded story, very small footprint, native code, direct-to-flash workflow, unusually strong fit for MCU work.  
**Cons:** architecture-specific, less relevant for large desktop-hosted applications.

Source: https://mecrisp.sourceforge.net/

### 20C.5. zeptoforth — Native/Inlined Cortex-M Forth with an RTOS Mindset

**zeptoforth** is a modern Cortex-M Forth that combines **subroutine threading**, **native code inlining**, and a **preemptively multitasking RTOS**. This is an interesting point in the design space because many tiny Forths stay minimal; zeptoforth instead leans toward “serious embedded application environment” while still keeping the Forth implementation strategy performance-conscious.

It is not the safest universal answer to “fastest Forth,” but it is one of the more original *modern embedded* implementations because it mixes a native/inlining model with a richer systems environment.

**Pros:** modern MCU focus, flash/RAM flexibility, richer runtime model than many tiny Forths, explicit inline control.  
**Cons:** benchmark story is less standardized than Gforth/VFX reputation, narrower hardware domain.

Source: https://hackaday.io/project/170826-zeptoforth and https://github.com/tabemann/zeptoforth/discussions/190

### 20C.6. iForth / tForth — Benchmark Culture and Parallel-Compiler Lineage

Marcel Hendrix’s **iForth** site is valuable less because it settles the winner question, and more because it preserves an unusually rich **benchmark culture** around Forth performance. The site links matrix, LINPACK, FFT, nsieve, and broader benchmark collections, and explicitly says that ideas from the **parallel transputer compiler tForth** were carried into iForth.

That makes it a useful “original sides” entry: a Forth lineage where benchmarking, metacompilation, and parallel/compiler experimentation are treated as first-class implementation topics rather than side notes.

**Pros:** rich historical/performance material, benchmark-oriented, interesting lineage from transputer work.  
**Cons:** not the cleanest answer for a single modern production winner.

Source: https://iforth.nl/

### 20C.7. colorForth — Radical Source Representation and Tiny Compiler Path

**colorForth** is not the best default answer to “fastest modern Forth,” but it is one of the most original implementation lines in the entire ecosystem. Chuck Moore’s presentation describes a source representation where token classes are visually distinguished by color, paired with a tiny compiler that emits **subroutine-threaded** Pentium code and **inlines** several primitive arithmetic operations.

The interesting point is not merely performance; it is that source notation, compiler size, and machine model are treated as one co-designed object. That is a much rarer design stance than in mainstream compiler work.

**Pros:** radically original, tiny and fast compile path, historically important for language/hardware co-design.  
**Cons:** idiosyncratic, not a general recommendation for most users.

Source: https://www.ultratechnology.com/color4th.html and https://www.forth.com/resources/forth-programming-language/

### 20C.8. Practical Bottom Line on “Fastest Forth”

A defensible short ranking is:

- **Fastest open threaded-code implementation:** **Gforth / gforth-fast**.  
- **Fastest desktop/native-code implementation (conservative pick):** **VFX Forth**, with **SwiftForth** in the same discussion.  
- **Most distinctive tiny embedded native-code systems:** **Mecrisp-Stellaris / Mecrisp**; **zeptoforth** if you want a richer MCU environment.

The most important caveat is that desktop Forth performance comparisons are still unusually anecdotal. The reputations are strong, but truly apples-to-apples public benchmarking is thinner than in the C/C++/JVM/JS worlds.

## 20D. Hacker News and Community Marginalia Worth Mining

The following are not primary sources; they are useful because they capture practitioner experience, implementation folklore, and sharp one-paragraph explanations that papers often omit.

### 20D.1. “Implementing a Forth” (Hacker News)

One comment in the HN thread reports a **1.4 million line** Forth codebase moved onto **VFX Forth**, with the VFX native-code version said to run **at least ten times faster** than the earlier threaded-code build. Treat that as anecdote, not benchmark science, but it is exactly the kind of large-system datapoint that is otherwise hard to find in Forth literature.

Source: https://news.ycombinator.com/item?id=44142652

### 20D.2. SwiftForth / Gforth / iForth Buyer’s-Eye Commentary

In HN discussion around the SwiftForth IDE release, commenters describe **Gforth** as the obvious free, well-rounded choice but also say **SwiftForth’s optimized subroutine threading** is materially faster. In another HN discussion, a practitioner mentions choosing **iForth** for 64-bit support and license simplicity while still regarding **SwiftForth** and **VFX** as top-tier commercial systems. Again: anecdotal, but useful as ecosystem temperature.

Source: https://news.ycombinator.com/item?id=47045194 and https://news.ycombinator.com/item?id=22802449

### 20D.3. Tree-sitter Praise and Pushback

The HN thread on tree-sitter is worth reading because it contains both the usual praise — compact trees, explicit `ERROR` nodes, good fit for per-keystroke parsing — and the sharpest criticism from grammar authors who dislike external scanners, generated file bulk, or debugging conflict behavior. This is a better balance than official docs alone.

Source: https://news.ycombinator.com/item?id=26225298 and https://news.ycombinator.com/item?id=39768020

### 20D.4. Arena-Based Parser Layouts

A 2024 HN thread on arena-based parsers includes a useful implementation idea: instead of building a pointer-rich DOM/tree, write a cleaned-up normalized copy of the parsed text into a byte arena and use **offsets** instead of pointers for relations such as siblings and parents. This lines up nicely with the document’s existing discussion of flat/compact ASTs and pointer-free layouts.

Source: https://news.ycombinator.com/item?id=40276112

### 20D.5. Ungrammar in One Sentence

An HN explanation of **ungrammar** gets to the core idea in one paragraph: it is not really about parsing strings, but about generating the **concrete syntax tree node API**. That is a valuable framing to keep around because it helps readers separate grammar engineering from tree-API engineering.

Source: https://news.ycombinator.com/item?id=24878098 and https://news.ycombinator.com/item?id=37119482

### 20D.6. langcc as “More Than a Parser Generator”

The HN discussion around langcc highlights a key implementation point that can be easy to miss from the name alone: langcc is trying to generate a **whole frontend skeleton**, not just a parser. That makes it a closer relative of syntax/IR workbenches than of classic yacc.

Source: https://news.ycombinator.com/item?id=32949019

## 21. References


1. rustc Span design — https://doc.rust-lang.org/nightly/nightly-rustc/rustc_span/struct.Span.html
2. Go token.Pos — https://pkg.go.dev/go/token
3. Roslyn Red-Green Trees — https://ericlippert.com/2012/06/08/red-green-trees/
4. rowan (rust-analyzer) — https://github.com/rust-analyzer/rowan
5. Zig ZIR documentation — https://github.com/ziglang/zig/blob/master/src/Zir.zig
6. DWARF debug format — https://dwarfstd.org/
7. JS Source Maps (ECMA-426) — https://tc39.es/ecma426/
8. CPython PEP 657 — https://peps.python.org/pep-0657/
9. Pratt Parsers: Expression Parsing Made Easy — https://journal.stuffwithstuff.com/2011/03/19/pratt-parsers-expression-parsing-made-easy/
10. PEG parsers (pegen) — https://we-like-parsers.github.io/pegen/peg_parsers.html
11. Squirrel Parser — https://arxiv.org/abs/2601.05012
12. Fast Incremental PEG Parsing (GPeg) — https://doi.org/10.1145/3486608.3486900
13. Tree-sitter — https://tree-sitter.github.io/tree-sitter
14. Accelerated-Zig-Parser — https://github.com/Validark/Accelerated-Zig-Parser
15. Meriyah — https://github.com/nicolo-ribaudo/meriyah
16. TCC (Tiny C Compiler) — https://bellard.org/tcc/
17. Cuik (RealNeGate) — https://github.com/RealNeGate/Cuik
18. Copy-and-Patch Compilation — https://arxiv.org/abs/2011.13127
19. Copy-and-Patch worked example — https://scot.tg/2024/12/22/worked-example-of-copy-and-patch-compilation/
20. Sea of Nodes — Click, "From Quads to Graphs" (1993)
21. Cranelift E-graph RFC — https://github.com/bytecodealliance/rfcs/blob/main/accepted/cranelift-egraph.md
22. egg E-graph Library — https://egraphs-good.github.io/
23. bumpalo Arena Allocator — https://docs.rs/bumpalo
24. bump-scope Allocator — https://docs.rs/bump-scope
25. Hash Consing — https://en.wikipedia.org/wiki/Hash_consing
26. Hazel Typed Holes — https://hazel.org/
27. Röhrich Error Correction (1980) — https://link.springer.com/article/10.1007/BF00263989
28. Jamie Brandon: Implementing Interactive Languages — https://www.scattered-thoughts.net/writing/implementing-interactive-languages/
29. Efficient Incremental Parsing (Wagner thesis) — https://diekmann.co.uk/diekmann_phd.pdf
30. JVM LineNumberTable — https://docs.oracle.com/javase/specs/jvms/se17/html/jvms-4.html
31. LuaJIT Remake (Haoran Xu) — https://sillycross.github.io/2023/05/12/2023-05-12/
32. gperf Perfect Hash Generator — https://www.gnu.org/software/gperf/
33. GLL Parsing (Scott & Johnstone) — https://pure.royalholloway.ac.uk/en/publications/purely-functional-gll-parsing
34. Faster, Practical GLL Parsing — https://link.springer.com/chapter/10.1007/978-3-662-46663-6_5
35. One Parser to Rule Them All (Data-Dependent Grammars) — https://ir.cwi.nl/pub/24027/24027B.pdf
36. Scannerless Parsing — https://en.wikipedia.org/wiki/Scannerless_parsing
37. Faster Scannerless GLR Parsing (SRNGLR) — https://www.researchgate.net/publication/221302808_Faster_Scannerless_GLR_Parsing
38. Futamura Projections — Y. Futamura, "Partial evaluation of computation process" (1971)
39. Supercompilation (Turchin) — https://mazdaywik.github.io/direct-link/The%20Concept%20of%20a%20Supercompiler.pdf
40. NaN Boxing and Tagged Pointers — https://witch.work/en/posts/javascript-trip-of-js-value-tagged-pointer-nan-boxing
41. ExBoxing — https://medium.com/@kannanvijayan/exboxing-bridging-the-divide-between-tag-boxing-and-nan-boxing-07e39840e0ca
42. Linear Scan Register Allocation — https://web.cs.ucla.edu/~palsberg/course/cs132/linearscan.pdf
43. Efficient Global Register Allocation — https://arxiv.org/pdf/2011.05608
44. Storage Strategies for Collections in Dynamically Typed Languages — https://soft-dev.org/pubs/pdf/bolz_diekmann_tratt__storage_strategies_for_collections_in_dynamically_typed_languages.pdf
45. CPS-SSA Correspondence (Kelsey, 1995) — https://bernsteinbear.com/assets/img/kelsey-ssa-cps.pdf
46. Compiling with Continuations (Appel, 1992) — https://www.cs.princeton.edu/~appel/papers/cpcps.pdf
47. MLIR: Multi-Level Intermediate Representation — https://mlir.llvm.org/
48. Composable Code Generation in MLIR — https://arxiv.org/abs/2202.03293
49. Chris Lattner on MLIR and AI Fragmentation — https://www.modular.com/blog/democratizing-ai-compute-part-8-what-about-the-mlir-compiler-infrastructure
50. QBE Compiler Backend — https://c9x.me/compile/
51. Cranelift Progress in 2022 — https://bytecodealliance.org/articles/cranelift-progress-2022
52. Cranelift regalloc2 — https://cfallin.org/blog/2022/06/09/cranelift-regalloc2/
53. ISLE Language Reference — https://github.com/bytecodealliance/wasmtime/blob/main/cranelift/isle/docs/language-reference.md
54. TPDE: A Fast Adaptable Compiler Back-End Framework — https://arxiv.org/abs/2505.22610
55. How JIT Compilers are Implemented and Fast — https://kipp.ly/jits-impls/
56. Musings on Tracing in PyPy (Bolz-Tereick, 2025) — https://pypy.org/posts/2025/01/musings-tracing.html
57. On-Stack Replacement, Distilled (D'Elia & Demetrescu, PLDI 2018) — https://season-lab.github.io/papers/osr-distilled-pldi18.pdf
58. Deoptless: Speculation with Dispatched OSR (Flückiger et al., PLDI 2022) — https://janvitek.org/pubs/pldi22.pdf
59. Formally Verified Speculation and Deoptimization (Barrière et al., POPL 2021) — https://hal.science/hal-03185848v1/document
60. Polyhedral Compilation — http://polyhedral.info/
61. Halide: Decoupling Algorithms from Schedules — https://halide-lang.org/
62. Triton: Related Work on Polyhedral vs Scheduling Languages — https://triton-lang.org/main/programming-guide/chapter-2/related-work.html
63. Tiramisu Compiler — http://tiramisu-compiler.org/
64. Enzyme: High-Performance AD of LLVM — https://enzyme.mit.edu/
65. Enzyme GPU Reverse-Mode AD — https://c.wsmoses.com/papers/EnzymeGPU.pdf
66. Perceus: Garbage Free Reference Counting with Reuse (PLDI 2021) — https://www.microsoft.com/en-us/research/wp-content/uploads/2020/11/perceus-tr-v1.pdf
67. Reference Counting with Frame Limited Reuse (ICFP 2022) — https://www.microsoft.com/en-us/research/wp-content/uploads/2023/07/flreuse.pdf
68. Koka Programming Language — https://koka-lang.github.io/koka/doc/book.html
69. MLKit Region-Based Memory Management — https://elsman.com/mlkit/
70. Region-Based Memory Management in Cyclone (PLDI 2002) — https://www.cs.umd.edu/projects/cyclone/papers/cyclone-regions.pdf
71. Hidden Classes in V8 — https://v8.dev/docs/hidden-classes
72. Bidirectional Typing Survey (Dunfield & Krishnaswami, 2021) — https://dl.acm.org/doi/fullHtml/10.1145/3450952
73. Incremental Bidirectional Typing via Order Maintenance (Porter et al., 2025) — https://arxiv.org/abs/2504.08946
74. Generalized Evidence Passing for Effect Handlers (Xie & Leijen, ICFP 2021) — https://xnning.github.io/papers/multip.pdf
75. Efficient Compilation of Algebraic Effect Handlers (Schrijvers et al., 2021) — https://dl.acm.org/doi/10.1145/3485479
76. Salsa Incremental Computation Framework — https://github.com/salsa-rs/salsa
77. rustc Query System — https://rustc-dev-guide.rust-lang.org/query.html
78. V8 WebAssembly Compilation Pipeline — https://v8.dev/docs/wasm-compilation-pipeline
79. Ruff v0.4.0: Hand-Written Recursive Descent Parser — https://astral.sh/blog/ruff-v0.4.0
80. Ruff Internals (Compiler Alchemy) — https://compileralchemy.substack.com/p/ruff-internals-of-a-rust-backed-python
81. ty: An Extremely Fast Python Type Checker and LSP — https://astral.sh/blog/ty
82. ty GitHub Repository — https://github.com/astral-sh/ty

83. PEP 617 — https://peps.python.org/pep-0617/
84. What’s New in Python 3.9 — https://docs.python.org/3/whatsnew/3.9.html
85. langcc — https://langcc.io/
86. Practical LR Parser Generation — https://arxiv.org/abs/2209.08383
87. Marpa parser site — https://jeffreykegler.github.io/Marpa-web-site/
88. Marpa and the Ruby Slippers — https://jeffreykegler.github.io/Ocean-of-Awareness-blog/individual/2011/11/marpa-and-the-ruby-slippers.html
89. Menhir manual — https://gallium.inria.fr/~fpottier/menhir/manual.html
90. Introducing Ungrammar — https://rust-analyzer.github.io/blog/2020/10/24/introducing-ungrammar.html
91. From AST to Lossless Syntax Tree — https://www.oilshell.org/blog/2017/02/11.html
92. Factor: a dynamic stack-based programming language — https://factorcode.org/slava/dls.pdf
93. Factor optimizing compiler overview — https://concatenative.org/wiki/view/Factor/Optimizing%20compiler
94. Gforth performance — https://gforth.org/manual/Performance.html
95. Gforth dynamic superinstructions — https://www.complang.tuwien.ac.at/forth/gforth/Docs-html/Dynamic-Superinstructions.html
96. A Look at Gforth Performance — https://www.complang.tuwien.ac.at/anton/euroforth/ef09/papers/ertl.pdf
97. VFX Forth common features — https://www.mpeforth.com/software/pc-systems/vfx-forth-common-features/
98. VFX Forth — https://vfxforth.com/
99. SwiftForth — https://www.forth.com/swiftforth/
100. Mecrisp — https://mecrisp.sourceforge.net/
101. iForth home page — https://iforth.nl/
102. colorForth presentation — https://www.ultratechnology.com/color4th.html
103. Forth history / colorForth note — https://www.forth.com/resources/forth-programming-language/
104. zeptoforth project page — https://hackaday.io/project/170826-zeptoforth
105. zeptoforth inline discussion — https://github.com/tabemann/zeptoforth/discussions/190
106. Tree-sitter grammar DSL conflict handling — https://tree-sitter.github.io/tree-sitter/creating-parsers/2-the-grammar-dsl.html
107. HN: Implementing a Forth — https://news.ycombinator.com/item?id=44142652
108. HN: SwiftForth IDE — https://news.ycombinator.com/item?id=47045194
109. HN: I had my third go at Forth this year — https://news.ycombinator.com/item?id=22802449
110. HN: Tree-sitter parsing system — https://news.ycombinator.com/item?id=26225298
111. HN: Tree-sitter is great / hard to use — https://news.ycombinator.com/item?id=39768020
112. HN: Arena-based parsers — https://news.ycombinator.com/item?id=40276112
113. HN: Ungrammar — https://news.ycombinator.com/item?id=24878098
114. HN: AST vs. Bytecode — https://news.ycombinator.com/item?id=37119482
115. HN: langcc — https://news.ycombinator.com/item?id=32949019

