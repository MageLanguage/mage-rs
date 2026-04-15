# Bytecode Formats

Research on bytecode file format designs, header structures, section layouts, relocation strategies, serialization techniques, and zero-copy approaches across virtual machines, operating systems, and serialization frameworks.

---

## 1. Native Executable Formats

### 1.1. ELF — Executable and Linkable Format

ELF (System V Release 4, 1990s) is the standard binary format on Unix-like systems. Its design is the reference point for all section-based binary formats. An ELF file has two parallel views of the same data:

**Program headers** (segments) describe how the loader maps the file into memory at runtime. **Section headers** describe the logical structure for linkers and debuggers. An executable needs only program headers; an object file needs only section headers; a shared library needs both.

The ELF header is a fixed-size structure at offset 0:

```
ELF Header (64-bit):
  e_ident[16]     magic (\x7fELF), class, endianness, version, OS/ABI, padding
  e_type          object type (relocatable, executable, shared, core)
  e_machine       target architecture (x86_64, ARM, RISC-V, ...)
  e_version       always 1
  e_entry         virtual address of entry point
  e_phoff         offset to program header table
  e_shoff         offset to section header table
  e_flags         processor-specific flags
  e_ehsize        size of this header (allows future growth)
  e_phentsize     size of one program header entry
  e_phnum         number of program header entries
  e_shentsize     size of one section header entry
  e_shnum         number of section header entries
  e_shstrndx      index of section name string table
```

The crucial design decisions: (1) the header stores the size of its own entries (`e_ehsize`, `e_phentsize`, `e_shentsize`), allowing the format to grow without breaking old parsers; (2) the section header table and program header table are at arbitrary file offsets, not fixed positions; (3) the 16-byte `e_ident` prefix encodes endianness and word size before the rest of the header, so the loader knows how to parse subsequent fields.

**Sections** are identified by 4-byte string names from a dedicated string table (`e_shstrndx` points to it). Each section header records type, flags, address, offset, size, alignment, and entry size (for tables with fixed-size entries). Well-known sections include `.text` (code), `.data` (initialized data), `.bss` (zero-initialized data), `.rodata` (constants), `.symtab` (symbol table), `.strtab` (string table), `.rel.*`/`.rela.*` (relocations), and `.debug_*` (DWARF debug info).

**Relocations** connect symbolic references to definitions. A relocation entry records: the offset within the section to patch, the symbol index, the relocation type (encoding how to compute the value), and an addend. The relocation type is architecture-specific — x86_64 alone defines over 30 types (`R_X86_64_64` for absolute 64-bit, `R_X86_64_PC32` for PC-relative 32-bit, `R_X86_64_PLT32` for PLT calls, etc.). The linker processes relocations by computing `Symbol.value + Addend` and applying the type-specific operation at the target offset.

The separation of concerns is ELF's greatest strength: sections are orthogonal to segments, relocations are separate from code, and the string table indirection means section names are unlimited. The format has survived 30+ years of evolution without breaking changes because every table entry self-describes its size.

Source: System V ABI specification; https://refspecs.linuxfoundation.org/elf/gabi4+/ch4.eheader.html

### 1.2. Mach-O — Apple's Binary Format

Mach-O (Mach Object) is used on macOS, iOS, and all Apple platforms. Where ELF separates program headers from section headers, Mach-O uses a single linear list of **load commands** that serve both purposes.

```
Mach-O Layout:
  Header            magic, CPU type, file type, number of load commands
  Load Command 0    (e.g., LC_SEGMENT_64: maps a segment into memory)
  Load Command 1    (e.g., LC_SYMTAB: symbol table location)
  Load Command 2    (e.g., LC_DYSYMTAB: dynamic symbol table)
  ...
  Segment __TEXT    (contains sections: __text, __stubs, __cstring, ...)
  Segment __DATA    (contains sections: __data, __bss, __got, ...)
  Segment __LINKEDIT (symbol tables, string tables, relocations)
```

The header is minimal: magic number (0xFEEDFACF for 64-bit), CPU type/subtype, file type (executable, dylib, object, etc.), number of load commands, total size of load commands, and flags. Load commands immediately follow the header. Each command starts with a type tag and size, so unknown commands can be skipped — this is the extensibility mechanism.

`LC_SEGMENT_64` is the primary command: it names a segment (up to 16 bytes, e.g., `__TEXT`), gives its virtual address, virtual size, file offset, file size, and protection flags. Sections within a segment are listed inline in the command. Unlike ELF, where sections and segments are in separate tables, Mach-O nests sections inside their parent segment command.

**Universal binaries** (fat binaries) prepend a `fat_header` with an array of `fat_arch` entries, each pointing to a Mach-O for a different architecture. The loader picks the best match. This is elegant for distribution but wasteful on disk — a universal binary for arm64 + x86_64 is roughly double the size.

The load command model is more rigid than ELF's section headers (segment names are limited to 16 characters, sections must belong to a segment) but more self-contained — a single linear scan of load commands tells you everything about the binary. For paging, the header and load commands are considered part of the first segment (`__TEXT`), so they are mapped into memory alongside the code.

Source: Apple Mach-O Reference; https://github.com/aidansteele/osx-abi-macho-file-format-reference

### 1.3. PE/COFF — Windows Portable Executable

The Windows PE format descends from COFF (Common Object File Format). It opens with a DOS stub for backward compatibility — a tiny DOS program that prints "This program cannot be run in DOS mode" — followed by the PE signature (`PE\0\0`) at the offset given by `e_lfanew` in the DOS header.

The COFF header records machine type, number of sections, timestamp, and optional header size. The "optional" header (required for executables) contains the entry point address, image base (preferred load address), section alignment, file alignment, and a **Data Directory** array — 16 fixed-index entries pointing to import table, export table, resource table, relocation table, debug directory, TLS table, etc.

**Base relocations** in PE are organized in blocks by page (4KB). Each block has a page RVA (relative virtual address) and an array of 16-bit entries: 4 bits for relocation type, 12 bits for offset within the page. The most common type is `IMAGE_REL_BASED_DIR64` (absolute 64-bit address fixup). This page-block scheme is compact — most relocations cluster within the same page, so the page base is stored once per block rather than once per relocation.

The fixed-index Data Directory is PE's distinctive design choice. Unlike ELF (where you search section names) or Mach-O (where you scan load commands), PE gives O(1) access to the import table, export table, and relocation table — they are always at known indices in the Data Directory array. The downside is that the array size is fixed (16 entries), limiting extensibility without version bumps.

Source: Microsoft PE Format specification; https://learn.microsoft.com/en-us/windows/win32/debug/pe-format

---

## 2. Virtual Machine Bytecode Formats

### 2.1. JVM .class — The Java Class File

The JVM class file format (1995) is the most thoroughly specified bytecode format in existence. Every `.class` file begins with a 4-byte magic number (`0xCAFEBABE`), followed by minor and major version numbers.

```
ClassFile:
  magic               0xCAFEBABE
  minor_version       u16
  major_version       u16
  constant_pool_count u16
  constant_pool[]     variable-size entries (the "heart" of the format)
  access_flags        u16
  this_class          u16 (index into constant_pool)
  super_class         u16 (index into constant_pool)
  interfaces_count    u16
  interfaces[]        u16[] (indices into constant_pool)
  fields_count        u16
  fields[]            field_info structures
  methods_count       u16
  methods[]           method_info structures (bytecode lives here)
  attributes_count    u16
  attributes[]        extensible attribute structures
```

The **constant pool** is the central lookup table. It stores strings, class names, field references, method references, integers, longs, floats, doubles, and various descriptor types. All other structures reference constant pool entries by 1-based index. This is conceptually similar to ELF's string tables but much richer — it stores typed values, not just strings. The constant pool serves as both a string intern table and a symbol table.

Each `method_info` contains a `Code` attribute with the actual bytecode, plus a `LineNumberTable` (bytecode offset → source line), `LocalVariableTable` (variable names and types), and `StackMapTable` (verification hints added in Java 6). The `Code` attribute also declares `max_stack` and `max_locals` — the JVM uses these to pre-allocate the operand stack and local variable array for each frame, avoiding runtime growth checks.

The **attribute** system is the class file's extensibility mechanism. Each attribute is a `(name_index: u16, length: u32, data: u8[length])` triple. Unknown attributes are silently ignored. This allows the format to carry debug info, annotations, module declarations, and future features without version bumps. It is the same principle as ELF's section headers but at a finer granularity — attributes can appear on classes, fields, methods, and even within code.

The class file has no explicit relocation table because the JVM performs all linking symbolically at runtime. Method calls are encoded as constant pool indices pointing to `CONSTANT_Methodref_info` entries, which in turn reference class names and method descriptors as UTF-8 strings. The first time a method is called, the JVM resolves the symbolic reference and patches the constant pool entry (or an internal cache) with a direct pointer. This lazy symbolic resolution is why Java has no static linker.

Source: JVM Specification Chapter 4; https://docs.oracle.com/javase/specs/jvms/se19/html/jvms-4.html

### 2.2. WebAssembly — Section-Based Streaming Module

WebAssembly's binary format (2017) was designed with two unusual constraints: streaming compilation (begin compiling before the entire file is downloaded) and parallel compilation (compile multiple functions simultaneously). These constraints directly shaped the section layout.

```
Wasm Module:
  magic           \0asm (4 bytes)
  version         1 (4 bytes, little-endian u32)
  Section 1       Type Section (function signatures)
  Section 2       Import Section
  Section 3       Function Section (signature indices only — no bodies)
  Section 4       Table Section
  Section 5       Memory Section
  Section 6       Global Section
  Section 7       Export Section
  Section 8       Start Section
  Section 9       Element Section
  Section 10      Code Section (function bodies)
  Section 11      Data Section
  Section 12      Data Count Section
  Custom Section  (name, debug info, etc. — can appear anywhere)
```

Each section is `(section_id: u8, size: u32, contents: u8[size])`. The `size` field allows skipping unknown sections and enables streaming — the decoder knows exactly how many bytes to read for each section. Integers throughout use LEB128 encoding (variable-length, borrowed from DWARF) for compactness.

The critical split: the **Function Section** (section 3) declares only the type signature index for each function, while the **Code Section** (section 10) contains the actual bodies. Because signatures come first, a streaming compiler can begin allocating call stubs and performing type checking before any function body arrives. Furthermore, since each function body in the Code Section is length-prefixed, multiple functions can be compiled in parallel on separate threads.

**Custom sections** have section_id 0 and carry a UTF-8 name string followed by arbitrary payload. The `name` custom section provides human-readable names for debugging. DWARF debug info is embedded in custom sections named `.debug_info`, `.debug_line`, etc. Custom sections can appear anywhere between known sections and are silently ignored by runtimes that don't understand them.

WebAssembly has no relocation table in its standard format. All references are by index into the module's index spaces (functions, globals, tables, memories). These are resolved during instantiation by the embedder providing imports. However, the `wasm-ld` linker uses a custom `linking` section with its own relocation entries for object files that haven't been linked yet — demonstrating how custom sections can extend the format for toolchain-internal use without changing the spec.

Source: WebAssembly Specification, Binary Format; https://webassembly.github.io/spec/core/binary/modules.html

### 2.3. Android DEX — Dalvik Executable

The DEX format (2008) was designed for memory-constrained mobile devices. Where JVM class files store one class per file with per-class constant pools, DEX packs all classes in an application into a single file with shared pools. This eliminates redundancy across classes — a string like `"java/lang/Object"` is stored once, not once per class that references it.

```
DEX Layout:
  header          magic ("dex\n038\0"), checksum, SHA-1 hash, file size,
                  header size, endian tag, and offsets/sizes of all pools
  string_ids[]    offsets to string data
  type_ids[]      string indices for type descriptors
  proto_ids[]     method prototypes (return type + parameter types)
  field_ids[]     (class, type, name) triples
  method_ids[]    (class, proto, name) triples
  class_defs[]    class definitions with offsets to class data
  call_site_ids[] (Android 8.0+)
  method_handles[](Android 8.0+)
  data            string data, code items, debug info, annotations, etc.
  link_data       (reserved)
```

The header is 112 bytes and contains the offset and count of every pool. This means a parser can jump directly to any pool without scanning — O(1) access to all tables. Pool entries use 32-bit indices and offsets, with LEB128 encoding within data items for compactness.

Each `code_item` (function body) declares `registers_size`, `ins_size`, `outs_size`, and `insns_size` — the total register count, parameter count, outgoing argument count, and instruction count. This is the same principle as the JVM's `max_stack`/`max_locals` but for a register-based VM. The try-catch handlers, debug info, and bytecode instructions follow within the code item.

The DEX format uses a **map section** (`map_list`) at the end of the file that describes every item's type, count, and offset. This is a table of contents for the entire file — it enables tools to enumerate all items without understanding every structure. It also enables verification: the map must account for every byte in the file.

The shared pool design pays off dramatically. Google reported that DEX files are roughly 50% smaller than the equivalent set of class files. The downside is that DEX cannot be incrementally loaded per-class — the entire file must be available. Android addressed this with `dex2oat` (ahead-of-time compilation to native code at install time) and later with ART's profile-guided compilation.

Source: Android DEX format specification; https://source.android.com/docs/core/runtime/dex-format

### 2.4. Erlang BEAM — IFF-Style Chunk Format

The BEAM file format (Erlang's VM bytecode) uses a variant of EA IFF 1985 (Interchange File Format), the same container format used by Amiga IFF, AIFF audio, and RIFF/WAV. The file starts with `FOR1`, followed by the total size, then the form type `BEAM`.

```
BEAM File:
  "FOR1"        4-byte tag
  size          u32 big-endian (total size of remaining data)
  "BEAM"        4-byte form type
  Chunk "AtU8"  atom table (UTF-8 atom names)
  Chunk "Code"  bytecode instructions
  Chunk "StrT"  string table (binary literals)
  Chunk "ImpT"  import table (module, function, arity triples)
  Chunk "ExpT"  export table (function, arity, label triples)
  Chunk "LocT"  local function table
  Chunk "FunT"  lambda/fun table
  Chunk "LitT"  literals table (compressed Erlang terms)
  Chunk "Attr"  module attributes (compiled metadata)
  Chunk "CInf"  compilation info (compiler options, source path)
  Chunk "Dbgi"  debug info (abstract syntax tree, if present)
  Chunk "Docs"  documentation chunk
  Chunk "Line"  line number table
  ...
```

Each chunk is `(tag: [u8; 4], size: u32, data: u8[size], padding: 0..3 bytes to 4-byte alignment)`. The 4-byte tag is a human-readable ASCII identifier. Unknown chunks are silently skipped. This is one of the simplest extensible container formats — no header table, no offsets table, just a linear sequence of self-describing chunks.

The **atom table** (`AtU8`) is conceptually similar to ELF's string table or the JVM constant pool for strings — it stores all atom names used in the module, referenced by index from other chunks. The **import table** references atoms by index into this table: `(module_atom_index, function_atom_index, arity)`.

The **literals table** (`LitT`) is zlib-compressed. It contains Erlang terms serialized in the External Term Format (ETF). The compression is worthwhile because literal terms can be large (e.g., embedded data tables). The `Code` chunk references literals by index into this table.

The BEAM format has no relocation table because Erlang resolves all inter-module references at load time via the atom table and code server. When a module is loaded, the loader patches import references to point to the target module's export table entries. If the target module is later hot-swapped, the references are transparently updated.

Source: Erlang `beam_lib` documentation; https://www.erlang.org/doc/apps/stdlib/beam_lib.html

---

## 3. Scripting Language Bytecode

### 3.1. Lua 5.x — Recursive Prototype Dump

Lua's bytecode format is deliberately platform-specific. The header encodes the exact sizes of the host machine's types, and a mismatch causes the loader to reject the file. This is a pragmatic design — Lua targets embedded systems where cross-compilation is rare, and avoiding any portability abstractions keeps the format minimal.

```
Lua 5.4 Header:
  signature       "\x1bLua" (4 bytes)
  version         0x54 (major * 16 + minor)
  format          0 (official format)
  LUAC_DATA       "\x19\x93\r\n\x1a\n" (6 bytes — corruption detector)
  instruction_size sizeof(Instruction) — typically 4
  lua_Integer_size sizeof(lua_Integer) — typically 8
  lua_Number_size  sizeof(lua_Number) — typically 8
  LUAC_INT        0x5678 (integer byte-order check)
  LUAC_NUM        370.5 (float byte-order check)
```

The `LUAC_DATA` bytes are a corruption detector: `\r\n` catches text-mode FTP mangling CR→CRLF, `\x1a` stops DOS `TYPE` from dumping the rest of the file. This is the same trick as the JVM's `0xCAFEBABE` (obvious corruption) and Python's magic number `\r\n` suffix (text-mode detection), but more thorough — it checks multiple corruption modes in 6 bytes.

After the header, the format is a recursive dump of function prototypes. Each prototype contains:

```
Prototype:
  source           string (filename, only in the top-level prototype)
  line_defined     integer
  last_line_defined integer
  num_params       u8
  is_vararg        u8
  max_stack_size   u8
  code[]           instruction array (4 bytes per instruction)
  constants[]      typed constant pool (nil, boolean, integer, float, string)
  upvalues[]       upvalue descriptors (in_stack: u8, index: u8, kind: u8)
  protos[]         nested function prototypes (recursive)
  line_info[]      one line number per instruction (debug, strippable)
  local_vars[]     (name, start_pc, end_pc) triples (debug, strippable)
  upvalue_names[]  strings (debug, strippable)
```

Nested functions are stored inline — `protos[]` is an array of complete prototypes. There is no global table of functions, no offsets table, no index. The loader deserializes recursively, building the tree in memory. This is simple but means you cannot skip to a specific function without deserializing everything before it.

Debug information (line_info, local_vars, upvalue_names) can be stripped by passing `strip=1` to `luaU_dump`. The stripped format is identical except these fields are empty. This is a clean separation — the bytecode is valid and executable without debug info.

Lua has no relocation table. All references within a prototype are by index (constants by index, upvalues by index, nested protos by index). The instruction set uses register indices (8-bit fields within the 32-bit instruction word). There are no absolute addresses to relocate.

Source: Lua 5.4 source code (`lundump.c`, `ldump.c`); https://www.lua.org/source/5.4/lundump.c.html

### 3.2. LuaJIT — Streaming Bottom-Up Prototypes

LuaJIT's bytecode dump format (Mike Pall) reverses Lua's prototype order: child prototypes are serialized before their parents. This enables single-pass loading — when the loader encounters a prototype that references a child, the child has already been deserialized and is available. No backpatching or second pass is needed.

```
LuaJIT Bytecode Dump:
  header:
    magic           "\x1bLJ" (3 bytes)
    version         u8 (bytecode version, e.g., 1 or 2)
    flags           uleb128 (strip debug, big-endian, FFI, etc.)
    if !stripped:
      source_name   length-prefixed string
  proto 0           (innermost function — leaf)
  proto 1           (may reference proto 0)
  ...
  proto N           (top-level chunk — outermost)
  trailer:
    0x00            end-of-file marker
```

Each prototype is length-prefixed (total byte size as ULEB128), enabling skipping without parsing internals. Within a prototype: flags, parameter count, frame size, upvalue count, complex constant count, numeric constant count, instruction count, then the data arrays in order: instructions, upvalue references, complex constants (strings, nested proto references, tables), numeric constants, and optionally debug line info and variable names.

The ULEB128 encoding throughout (instead of fixed u32) makes the format more compact. The bottom-up ordering is the key innovation — it enables streaming deserialization without a table of contents or forward references. Contrast with Lua 5.x's top-down recursive format, which requires the deserializer to recurse and backtrack.

Source: https://github.com/RedHolms/LuaJIT-BytecodeDumpFormat; LuaJIT source `lj_bcdump.h`, `lj_bcwrite.c`, `lj_bcread.c`

### 3.3. Luau — Version-Tagged Format with Type Annotations

Luau (Roblox's Lua fork) extends the bytecode format with a version byte that enables backward compatibility. The format starts with a version tag, then a string table, followed by function prototypes, and ends with a main prototype index.

```
Luau Bytecode:
  version         u8 (currently 3–6)
  types_version   u8 (version 4+, type annotation version)
  string_count    varint
  string_table[]  length-prefixed strings
  proto_count     varint
  proto_table[]   function prototypes (reference strings by index)
  main_proto      varint (index of entry-point prototype)
```

The **string table** is a global shared pool, unlike Lua 5.x where each prototype has its own constants array. This is the DEX strategy applied to Lua — shared pools eliminate redundancy across functions. Each prototype references strings by global index.

Each prototype contains: max stack size, parameter count, upvalue count, `is_vararg` flag, flags byte, type information (version 4+), instruction array, constant array (referencing string table for strings, containing inline values for numbers/booleans), closure reference array, and optionally line info (with a small/large line encoding scheme) and debug info.

Luau's bytecode version field enables the runtime to reject incompatible bytecode with a clear error rather than crashing on unexpected encoding. Versions are bumped when the instruction set changes (new opcodes, changed operand encoding). This is simple but effective — it avoids the need for the format to be self-describing at the instruction level.

Source: Luau source `Compiler/src/BytecodeBuilder.cpp`, `VM/src/lvmload.cpp`; https://luau.org/

### 3.4. CPython .pyc — Minimal Header, Marshaled Object Graph

Python's `.pyc` format is one of the simplest bytecode formats in wide use. It is a thin wrapper around Python's `marshal` serialization of a code object tree.

```
.pyc File (Python 3.7+):
  magic_number     u16 (changes with each bytecode version) + \r\n
  bit_field        u32 (flags, e.g., hash-based validation)
  timestamp        u32 (source modification time, or 0 if hash-based)
  source_size      u32 (source file size for validation)
  code_object      marshal'd code object (recursive, rest of file)
```

The magic number changes with every Python minor release (and sometimes patch releases) that modifies the bytecode instruction set. The `\r\n` suffix after the magic number detects text-mode file corruption — the same technique as Lua. The `bit_field` (PEP 552, Python 3.7) enables hash-based validation instead of timestamp-based, which is important for reproducible builds.

The code object is serialized via Python's `marshal` module. Each code object contains: argument count, keyword-only argument count, number of locals, stack size, flags, raw bytecode bytes, constants tuple (recursive — may contain nested code objects), names tuple, variable names, free variables, cell variables, filename, name, first line number, and a line number table.

The line number table format changed significantly in Python 3.11 (PEP 657): it now encodes `(start_line, end_line, start_col, end_col)` per instruction, enabling the `^^^` caret indicators in tracebacks. This increased `.pyc` size by ~22% but was deemed worthwhile for developer experience. The encoding uses adaptive compression — entries with small deltas use fewer bytes.

Python has no relocation table. All references within bytecode are by index into the per-code-object name and constant tables. The `LOAD_CONST` instruction takes an index into `co_consts`, `LOAD_NAME` takes an index into `co_names`, etc. Nested code objects (for nested functions) are stored as entries in the constants tuple, similar to Lua's inline nested prototypes.

Source: Ned Batchelder, "The structure of .pyc files" (2008); PEP 657; https://nedbatchelder.com/blog/200804/the_structure_of_pyc_files.html

---

## 4. Relocation and Patching Strategies

### 4.1. ELF Relocations — Symbol + Addend + Type

ELF relocations are the most general relocation scheme. Each relocation entry specifies: the offset within the section to patch, the symbol table index, the relocation type (architecture-specific), and an addend (either inline in the instruction or explicit in the relocation entry).

```
Elf64_Rela:
  r_offset    u64   offset in section where fixup is applied
  r_info      u64   symbol index (high 32) | relocation type (low 32)
  r_addend    s64   constant to add to symbol value
```

The relocation type encodes both the operation (absolute, PC-relative, GOT-relative, PLT-relative, etc.) and the format (8-bit, 16-bit, 32-bit, 64-bit, with or without sign extension). This is maximally flexible — any addressing mode the architecture supports can be expressed as a relocation type.

There are two flavors: `Elf64_Rel` (addend is implicit, stored in the instruction being patched) and `Elf64_Rela` (addend is explicit in the relocation entry). `Rela` is preferred on 64-bit because it avoids having to decode the existing instruction to extract the implicit addend. x86_64 ELF uses `Rela` exclusively.

The static linker resolves most relocations at link time. Those that remain (in shared libraries) are handled by the dynamic linker at load time, stored in `.rela.dyn` (data relocations) and `.rela.plt` (procedure linkage table stubs for lazy binding).

Source: ELF specification, Chapter 6 "Relocation"; https://gabi.xinuos.com/elf/06-reloc.html

### 4.2. PE Base Relocations — Page-Grouped Fixups

Windows PE relocations are simpler than ELF's. They exist for a single purpose: when a DLL or executable is loaded at a different base address than its preferred `ImageBase`, all absolute addresses embedded in the code must be adjusted by the delta.

The relocation table (`.reloc` section) is organized as a sequence of **blocks**, one per 4KB page:

```
Block:
  page_rva      u32   base RVA of the page
  block_size    u32   total size of this block (including header)
  entries[]     u16   array of (type:4, offset:12) pairs
```

Each 16-bit entry encodes a 4-bit relocation type and a 12-bit offset within the page. The most common type is `IMAGE_REL_BASED_DIR64` — add the base delta to the 64-bit value at the given offset. The page-block organization exploits spatial locality: absolute addresses tend to cluster within the same code page, so the page base is amortized across many entries.

The Windows loader applies all relocations at image load time (not lazily). If the image loads at its preferred base, the `.reloc` section is not processed at all — this is the common case for executables (which typically get their preferred address) and an optimization that avoids touching those pages.

Source: PE Format specification, "Base Relocation Table"; https://learn.microsoft.com/en-us/windows/win32/debug/pe-format

### 4.3. Threaded Code Patching — Gforth Image Relocation

Gforth (a Forth implementation) faces a unique relocation problem: its image files contain threaded code where each cell is either a code-field address (pointing to a primitive's machine code) or a data address (pointing within the dictionary). Both kinds of addresses are absolute and must be relocated when the image loads at a different address than where it was saved.

Gforth offers three strategies:

**Non-relocatable images:** Raw memory dumps. Load address must match save address exactly. Fast to save and load, but fragile — on systems with ASLR, they almost never work. Created with `savesystem`.

**Data-relocatable images:** Absolute data addresses are present, but code addresses are replaced with **tokens** (small indices into a table of primitives). At load time, tokens are replaced with actual code addresses for the current engine binary, and data addresses are adjusted by the base delta. This makes images portable across different builds of the same Gforth version but disables dynamic native code generation.

**Fully relocatable images:** Both data addresses and code addresses are tokenized. These images work across different Gforth builds, different machines with the same data format (endianness, cell size), and with dynamic native code generation. Created with `gforthmi`.

The relocation data is stored as a **bitmap** — one bit per cell in the image, indicating whether that cell contains an address that needs relocation. This is extremely compact: for a 1MB image, the bitmap is only 16KB (at 8 bytes per cell, ~128K cells, ~16K bytes of bitmap). The loader scans the bitmap and adjusts each flagged cell.

Win32Forth and Mitch Bradley's `cforth` take a different approach: they relocate at **runtime** rather than load time. Every memory access adds a base offset. This avoids any relocation data in the image but costs one addition per memory access — a nontrivial overhead for an inner-loop interpreter.

Source: Gforth Manual, "Image Files"; https://gforth.org/manual/Image-File-Background.html

### 4.4. Text Relocations — The Performance Problem

Ulrich Drepper's influential "Text Relocations" document explains why relocations in executable code (`.text` section) are problematic for shared libraries:

1. **Copy-on-write breakage:** If code pages contain absolute addresses that must be patched at load time, those pages become dirty (modified) and can no longer be shared across processes. Each process gets its own private copy. For a widely-used shared library, this multiplies memory usage.

2. **Security:** Writable code pages conflict with W^X (write XOR execute) policies. The loader must temporarily make the code writable, apply patches, then make it executable again. This window is a potential attack vector.

3. **Performance:** Patching code at load time means touching (and faulting in) every page that contains a relocation. Large libraries with many text relocations add measurable startup latency.

The solution is **position-independent code (PIC):** use PC-relative addressing for code within the same shared library (the relative distance between two points in the same binary is constant regardless of load address) and the Global Offset Table (GOT) for external references. The GOT is in a writable data section, so only data pages are dirtied.

For virtual machines with threaded code (like Gforth or Mage's current design), the bytecode stream is analogous to a `.text` section full of absolute addresses (handler pointers). The same trade-offs apply: patching at load time dirties all pages, and position-independent bytecode (using indices or offsets instead of pointers) avoids this at the cost of one indirection per dispatch.

Source: Ulrich Drepper, "Text Relocations" (2006); https://akkadia.org/drepper/textrelocs.html

---

## 5. Inline-Threading and Preparation Sequences

### 5.1. SableVM — Preparation Sequences for Java Inline Threading

Gagnon and Hendren (2003) introduced "preparation sequences" to solve a problem specific to Java inline-threaded interpreters. In a directly-threaded interpreter, each bytecode is replaced by the address of its C handler. In an inline-threaded interpreter, the handler bodies are copied inline into a "superinstruction" block, eliminating dispatch overhead within basic blocks.

The problem: Java's dynamic class loading, lazy initialization, and multi-threading mean that some bytecodes cannot be fully resolved at first encounter. For example, `getfield` requires resolving the field offset, which requires loading the target class, which may trigger class initialization with side effects.

The solution: replace unresolved bytecodes with **preparation sequences** — small inline code snippets that, when first executed, perform the resolution, patch themselves with the resolved handler, and then jump to it. Subsequent executions hit the resolved handler directly. This is self-modifying code at the interpreter level — the preparation sequence is a one-shot trampoline that replaces itself.

This is conceptually identical to ELF's PLT (Procedure Linkage Table) lazy binding: the first call goes through a resolver stub that patches the GOT entry, and subsequent calls go directly to the target. The innovation is applying this to bytecode interpretation rather than native code linking.

Performance results: inline-threaded SableVM with preparation sequences achieved 1.2x–2.4x speedup over switch-based interpretation and 1.15x–2.14x over direct-threaded interpretation on Java benchmarks.

Source: Gagnon & Hendren, "Effective Inline-Threaded Interpretation of Java Bytecode Using Preparation Sequences" (CC 2003)

### 5.2. Piumarta & Riccardi — Selective Inlining for Threaded Code

Piumarta and Riccardi (PLDI 1998) demonstrated that copying handler bodies inline — selecting which handlers to inline based on basic block boundaries — can achieve up to 70% of optimized C performance for numerical computations in a portable interpreter. Their technique:

1. At compile time, extract the native code body of each handler from the interpreter binary (using labels-as-values in GCC).
2. At runtime, when a bytecode sequence is first encountered, concatenate the handler bodies for a basic block into a new native code buffer.
3. Replace the basic block's entry with a jump to the concatenated sequence.

The key insight: the concatenated sequence eliminates all dispatch overhead (indirect branches) within a basic block. Only basic block boundaries require dispatch. This is the same principle as superinstructions but carried to its logical extreme — every basic block becomes a single superinstruction.

The technique requires that handler bodies are position-independent (no absolute addresses to internal labels). The paper shows that with GCC's `-fpic` flag and careful handler authoring, this is achievable. Handlers that use absolute addressing require relocation when copied — the same problem as code relocation in shared libraries.

Source: Piumarta & Riccardi, "Optimizing direct threaded code by selective inlining" (PLDI 1998); https://www.piumarta.com/papers/pldi98-opt.pdf

---

## 6. Snapshot and Image Formats

### 6.1. V8 Snapshots — Serialized Heap with External References

V8's startup snapshot serializes the entire JavaScript heap (built-in objects, compiled bytecode, internal data structures) into a binary blob. Deserializing a snapshot creates a ready-to-use V8 context in 2ms instead of 40ms from scratch.

The snapshot contains serialized objects with internal pointers. Since heap addresses differ between the process that created the snapshot and the process loading it, V8 uses an **external reference table** — an array of known addresses (C++ function pointers, embedded builtins, etc.) that is built identically in every V8 process. Pointers to external references are serialized as indices into this table, then resolved to actual addresses during deserialization. This is functionally a relocation table but limited to a known set of targets.

V8's code cache (`ScriptData`) serializes compiled `SharedFunctionInfo` objects so that repeated loads of the same script skip parsing and compilation. The code cache includes a version hash (source hash + V8 version + flags) — if any of these differ, the cache is rejected. This is stricter than Lua's version check because V8's internal representations change frequently.

The snapshot blob includes a checksum for integrity verification. Making snapshots reproducible required fixing multiple sources of non-determinism: random seeds, timestamps captured during initialization, and pointer-dependent ordering (objects with seeded hashes). Joyee Cheung's detailed blog series on this is the best available documentation of the challenges.

Source: V8 blog, "Lazy deserialization"; Joyee Cheung, "Reproducible Node.js built-in snapshots" (2024); https://v8.dev/blog/lazy-deserialization; https://joyeecheung.github.io/blog/2024/09/28/reproducible-nodejs-builtin-snapshots-1/

### 6.2. Squeak/Smalltalk — Object Memory Snapshot

The Squeak Smalltalk image format is a direct memory snapshot: every object in the system (including the compiler, debugger, IDE, and all user code) is serialized into a single `.image` file. The header records the image format version, header size, image data size, old base address, special objects array OOP (object-oriented pointer), and other VM state.

```
Squeak Image Header:
  image_format     u32 (encodes version, closure support, float order, Spur, 32/64-bit)
  header_size      u32
  image_data_size  u32
  old_base_address u32 (address where image was saved)
  special_objects  u32 (OOP of the special objects array)
  last_hash        u32 (for object identity hashing)
  screen_size      u32 (saved window dimensions)
  flags            u32
  extra_vm_memory  u32
```

Object headers use three formats selected by a 2-bit tag: short (1 word, class index in upper bits), class (2 words, first is class OOP), and size-and-class (3 words, for large objects). The header also contains format bits, GC bits, and an identity hash.

The Spur object format (2014) is a major revision: each object has a 64-bit header with class index, format, identity hash, and GC metadata. Objects are allocated in fixed-size segments. The Spur format eliminates the old base address relocation by using object table indices (like handles) for inter-object references, rather than raw pointers. This makes images more portable across different load addresses.

The Smalltalk image approach is the most extreme "snapshot" design possible — the format preserves the entire running state, including call stacks, scheduled processes, and open windows. Load time is essentially a `memcpy` plus relocation, achieving near-instant startup for a complete IDE environment.

Source: Squeak wiki, ".image file" and "ImageFormat"; https://wiki.squeak.org/squeak/2213; https://wiki.squeak.org/squeak/6290

### 6.3. Forth Image Files — The Simplest Possible Snapshot

A Forth image is a memory dump of the dictionary — the data structure containing all compiled words. The simplest form (non-relocatable) is literally `fwrite(dictionary, dict_size, 1, file)` with a small header. The header records: stack sizes, entry point (the `QUIT` or `COLD` word), image base address, and Forth version.

Loading is equally simple: `fread` the image into memory, set the dictionary pointer, and start executing. If the load address matches the save address, every pointer in the image is already correct. This is the fastest possible load — no parsing, no relocation, pure memcpy.

The limitation is obvious: the image is tied to its save address. The progression from non-relocatable to data-relocatable to fully relocatable (as described in section 4.3) is a textbook example of the trade-off between load simplicity and portability. Each level of relocatability adds load-time processing and format complexity.

The Forth approach is notable for what it omits: no section headers, no string tables, no symbol tables, no relocation entries (in the non-relocatable case). The image is its own documentation — the dictionary structure within the image is the metadata. This is feasible because Forth has a single global namespace with a simple linked-list dictionary. Languages with more complex module systems require more metadata.

Source: Gforth Manual, "Image Files"; various Forth implementation documentation

---

## 7. Zero-Copy and Memory-Mapped Formats

### 7.1. FlatBuffers — Access Without Unpacking

FlatBuffers (Google, 2014) is a serialization format designed so that the serialized form IS the in-memory representation — no deserialization step is required. Data is accessed directly from the serialized buffer via offset arithmetic.

```
FlatBuffer Layout:
  root_table_offset    u32 (offset to root table from start of buffer)
  [file_identifier]    4 bytes (optional, like a magic number)
  ...vtables...        variable-length virtual tables
  ...objects...        tables, structs, strings, vectors
```

Tables (variable-schema objects) use **vtables** — small arrays of `u16` offsets that map field indices to byte offsets within the table. Each table starts with a `s32` offset to its vtable. To read field N: look up `vtable[N]`, if non-zero, add it to the table's position to get the field's address. If zero, the field is absent (use default).

Structs (fixed-schema objects) are stored inline with no vtable — they are plain C structs with known layout. This enables direct memory access with zero overhead, but structs cannot evolve (no optional fields, no versioning).

The key trade-off: FlatBuffers achieves zero-copy deserialization at the cost of pointer-based access (every field access involves offset arithmetic) and less compact encoding (data is aligned and padded like native structs). For read-heavy, write-rare workloads (e.g., game configuration files, ML model metadata), this trade-off is strongly favorable. For small messages sent frequently (e.g., RPC), the padding overhead can outweigh the deserialization savings.

FlatBuffers supports schema evolution for tables (new fields can be added at the end, old fields can be deprecated) but not for structs. This is enforced by the vtable mechanism — a reader with a newer schema sees extra vtable entries for the new fields; a reader with an older schema simply doesn't look them up.

Source: Google FlatBuffers documentation; https://github.com/google/flatbuffers

### 7.2. Cap'n Proto — Wire Format IS Memory Format

Cap'n Proto (Kenton Varda, 2013) takes zero-copy further than FlatBuffers. Its encoding is defined such that the on-the-wire byte layout IS a valid in-memory representation. There is literally no encoding or decoding step — `write(fd, buffer, size)` sends a message, `read(fd, buffer, size)` receives one ready to use.

```
Cap'n Proto Message:
  segment_count     u32 (number of segments minus 1)
  segment_sizes[]   u32[] (size of each segment in 8-byte words)
  [padding]         0 or 4 bytes to align to 8
  segment_0         root object lives here
  segment_1         overflow data
  ...
```

All data is aligned to 8-byte words. Pointers are relative (offset-based), not absolute, making messages position-independent — they can be placed anywhere in memory without relocation. This is a deliberate rejection of the Squeak/Forth approach (absolute pointers that need relocation) in favor of the ELF PIC approach (relative references that are position-independent).

Structs have a fixed "data section" (inline scalar fields) and a "pointer section" (references to variable-size data). The struct layout is computed from the schema at compile time. Adding new fields extends the data or pointer section; old readers simply see a shorter section and ignore the extra words.

Pointers encode the offset to the target (in words), plus the target's size/type. Three pointer types exist: struct pointers, list pointers, and far pointers (cross-segment references). The offset encoding means that the entire message can be `mmap()`'d from a file and accessed without any processing.

Cap'n Proto's encoding is less compact than Protobuf (no variable-length integers, alignment padding wastes space) but supports an optional **packing** layer that compresses zero bytes — since padding and unused fields are typically zero, packing recovers most of the size difference while maintaining random access within the packed stream.

Source: Cap'n Proto encoding specification; https://capnproto.org/encoding.html

### 7.3. mmap-Friendly Bytecode — Position-Independent Strategies

For a bytecode format that will be memory-mapped, the key constraint is: no absolute pointers in the mapped region. Every reference must be expressible as an index or a relative offset. This enables the OS to share mapped pages across processes and avoids dirtying pages with relocations.

Strategies observed across implementations:

**Index-based references** (JVM, Wasm, DEX, Lua): all references are indices into tables (constant pool, string table, function table). The tables themselves may contain absolute pointers after loading, but the bytecode stream is purely index-based. This is the simplest approach and the most common.

**Relative offset references** (Cap'n Proto, ELF PIC): references encode the distance from the reference site to the target. No base address is needed. This enables `mmap()` with zero processing. The downside is that computing the target requires an addition at each access site.

**Offset table + raw bytecode** (current Mage design): the bytecode stream contains opcode indices and operands that are pure data (no pointers). A separate offset table (the patch map) records which positions need to be patched with handler pointers and which need base-address relocation. The patching is done once at load time, after which the bytecode is ready for execution. This is ELF's relocation model applied to bytecode — the separation of relocation metadata from code is clean, but the patched bytecode becomes process-specific (cannot be shared via mmap across processes).

The optimal choice depends on the execution model. A switch-dispatch interpreter (JVM, Wasm) can use indices throughout because the dispatch loop translates indices to actions. A direct-threaded interpreter (Gforth, Mage) needs pointer-valued opcodes for performance, which means either (a) patching at load time (fast execution, dirty pages) or (b) indirect threading through a table (clean pages, one extra indirection per dispatch).

---

## 8. Compact Encoding Techniques

### 8.1. LEB128 — Variable-Length Integers

LEB128 (Little-Endian Base 128), borrowed from DWARF, encodes integers in 1–5 bytes. Each byte uses 7 bits for data and 1 bit (MSB) as a continuation flag. Values 0–127 fit in 1 byte, 128–16383 in 2 bytes, etc.

Used by: WebAssembly (all integers in the binary format), Android DEX (within data items), DWARF (debug info), LuaJIT (prototype sizes, counts). Not used by: JVM class files (fixed-width u16/u32), Lua 5.x (platform-native sizes), ELF headers (fixed-width).

LEB128 is ideal for formats where most values are small but some can be large (function indices, string lengths, line numbers). The savings over fixed u32 are significant in practice — WebAssembly modules are typically 20-30% smaller with LEB128 than they would be with fixed-width encoding.

The downside: variable-length encoding makes random access impossible without a pre-built index. You cannot jump to "the 50th LEB128 in this stream" without reading the preceding 49. This is why formats that use LEB128 for compactness also use fixed-width fields for table headers and section sizes.

Source: DWARF specification, Section 7.6; WebAssembly specification, Section 5.2.2

### 8.2. Delta Encoding — Exploiting Locality

Delta encoding stores the difference between consecutive values rather than absolute values. Used in:

**DWARF `.debug_line`:** The line number state machine encodes line increments and address increments. Special opcodes pack both into a single byte: `opcode = (line_delta - line_base) + (line_range * address_delta) + opcode_base`. Most statements advance by one line and a few bytes of code, so most line table entries are a single byte.

**JS Source Maps (ECMA-426):** VLQ (Variable-Length Quantity) delta encoding. Each mapping segment encodes deltas for generated column, source file index, original line, original column, and optional name index. Semicolons reset the generated column delta per line. Reduced source map sizes by 50% versus absolute encoding.

**DEX debug info:** Line number advances are delta-encoded. The `DBG_ADVANCE_LINE` opcode takes a signed LEB128 line delta. The `DBG_ADVANCE_PC` opcode takes an unsigned LEB128 address delta. Special opcodes encode both simultaneously (like DWARF).

Delta encoding is most effective when consecutive entries are spatially and numerically close — which is almost always the case for source maps, line tables, and relocation tables. A relocation table sorted by offset will have small positive deltas between consecutive entries.

### 8.3. Bit-Packed Tables — ELF and Gforth Approaches

**ELF symbol table:** Each `Elf64_Sym` entry packs the symbol type (4 bits) and binding (4 bits) into a single `st_info` byte. The symbol visibility (2 bits) occupies the low bits of `st_other`. This bit-packing reduces per-symbol overhead without sacrificing random access — entries are still fixed-size.

**Gforth relocation bitmap:** One bit per dictionary cell, indicating whether it contains a relocatable address. This is the most compact possible relocation representation — 1 bit per potential relocation site versus ELF's 24 bytes per actual relocation entry. The trade-off: the bitmap must cover ALL cells, whether they need relocation or not. For dense relocations (like a threaded-code dictionary), this is more compact; for sparse relocations, a list of offsets wins.

**PE relocation blocks:** 16-bit entries with 4-bit type and 12-bit page offset. The page-relative encoding limits the offset to 4096 bytes, which is exactly one x86 page. This type+offset packing is a middle ground — more compact than ELF's full entries, more flexible than Gforth's bitmap.

---

## 9. Version Compatibility and Format Evolution

### 9.1. Magic Numbers and Version Fields

Every bytecode format starts with identification bytes:

| Format | Magic | Size | Purpose |
|--------|-------|------|---------|
| ELF | `\x7fELF` | 4 | Format identification |
| Mach-O | `0xFEEDFACF` | 4 | Format + 64-bit flag |
| JVM | `0xCAFEBABE` | 4 | Format identification |
| Wasm | `\0asm` + version u32 | 8 | Format + version |
| Lua 5.4 | `\x1bLua` + version byte | 5 | Format + version + corruption check |
| LuaJIT | `\x1bLJ` + version byte | 4 | Format + version |
| Python | 2 bytes + `\r\n` | 4 | Format + version + corruption check |
| DEX | `dex\n038\0` | 8 | Format + version string |
| BEAM | `FOR1` + size + `BEAM` | 12 | IFF container + form type |
| PE | `MZ` (DOS) + `PE\0\0` | 4+4 | Legacy compat + format identification |

The universal pattern: 4+ bytes of magic for identification, followed immediately by a version indicator. Formats that are transmitted over networks (Python, Lua) include corruption-detection bytes (`\r\n`, `\x1a`). Formats designed for embedded use (LuaJIT, Luau) keep the magic minimal.

### 9.2. Extensibility Mechanisms

How formats handle unknown data determines their long-term viability:

**Tagged length-prefixed sections** (Wasm, BEAM, JVM attributes): Each section/chunk/attribute is `(tag, length, data)`. Unknown tags are skipped by reading `length` bytes. This is the most robust extensibility mechanism — old readers can always skip new sections. Wasm and BEAM use this for their entire structure; JVM uses it for the attribute system within the class file.

**Fixed-index arrays** (PE Data Directory): The directory has 16 slots at fixed indices. New data types require allocating a slot, and the total is capped. Simple and fast (O(1) lookup) but inflexible. Works when the set of possible sections is small and stable.

**Self-describing entry sizes** (ELF `e_shentsize`, `e_phentsize`): The header records how large each table entry is. If a future version adds fields to section headers, old readers still know how many bytes to skip per entry. This is more subtle than length-prefixed sections — it allows the table to grow while maintaining array indexing.

**Version-gated fields** (Luau, DEX): Certain fields exist only if the version number is above a threshold. The decoder checks the version before reading those fields. Simple but requires every decoder to know every version's layout.

### 9.3. Platform Sensitivity vs. Portability

Bytecode formats fall on a spectrum:

**Fully platform-specific** (Lua 5.x, Squeak images): Header encodes exact type sizes, byte order, and sometimes the save address. Loader rejects mismatches. Maximum simplicity, zero portability.

**Platform-independent with fixed encoding** (JVM, Wasm, DEX, Cap'n Proto): All integers are big-endian (JVM) or little-endian (Wasm, DEX, Cap'n Proto). Type sizes are fixed by specification. The same file works on any platform. Slightly more complex encoders/decoders, but the portability payoff is massive.

**Platform-independent with self-describing encoding** (ELF): The header records endianness and word size. The loader adapts. This is a middle ground — more portable than Lua (any platform can load any ELF if it understands the target), but the loader must handle both endianness options.

For a language VM, the JVM/Wasm approach (pick one encoding, specify it, done) is almost always correct. The only exception is extreme embedded scenarios where even byte-swapping is too expensive — and modern CPUs have native byte-swap instructions, so this exception barely exists.

---

## 10. Superinstruction Encoding in Bytecode

### 10.1. Static Superinstructions — Compile-Time Fusion

Several VMs pre-fuse common instruction sequences at compile time:

**OCaml bytecode:** The OCaml bytecode compiler identifies common pairs (e.g., `PUSH; ACC0` → `PUSHACC0`) and emits a single superinstruction. The instruction set includes ~30 superinstructions alongside ~120 base instructions. The superinstruction opcode is just another entry in the opcode table — no special encoding.

**CPython 3.12+ specializing adaptive interpreter:** Python's `LOAD_FAST; LOAD_FAST` is fused into `LOAD_FAST__LOAD_FAST` at runtime by the adaptive specialization machinery. The fused instruction occupies the same bytecode slots as the original pair but dispatches in one step instead of two.

The encoding challenge: superinstructions occupy the same opcode space as base instructions. If the base instruction set has 128 opcodes and you add 128 superinstructions, you need 256 opcode values — which may overflow a `u8` opcode field. Solutions include using a wider opcode field (u16), a prefix byte (like x86 instruction prefixes), or reserving opcode space from the start.

### 10.2. Dynamic Superinstructions — Runtime Patching

Some systems patch bytecode at runtime to create superinstructions:

**Gforth dynamic superinstructions:** Gforth copies handler bodies into a buffer at runtime, creating native-code superinstructions from sequences of primitives. The bytecode is replaced with a pointer to the new native sequence. This is the Piumarta & Riccardi technique (section 5.2) applied to Forth.

**Current Mage design:** The compiler emits base instructions, and the patch map records which adjacent pairs can be fused into superinstructions. At load time, the patcher replaces the first instruction's handler pointer with the fused handler and marks the second instruction's operands as part of the fused sequence. The key constraint: fused instructions must occupy the exact same byte footprint as the original pair, because the patching is in-place.

The in-place constraint is important. If a superinstruction were shorter than the pair it replaces, the saved space would create a gap. If longer, it would overwrite the next instruction. Keeping the footprint identical means the original layout is preserved — unfused fallback is always possible, and tools that process bytecode (debuggers, disassemblers) can ignore superinstructions and process the original pair.

---

## 11. Debug and Metadata Sections

### 11.1. Source Maps in Bytecode

How various formats associate bytecode positions with source positions:

**JVM `LineNumberTable`:** Array of `(start_pc: u16, line_number: u16)` — 4 bytes per entry. Only line granularity, no column info. Always present (even without `-g`). Stored as a method attribute, so it is per-method.

**DEX debug info:** A state-machine bytecode (like DWARF) within the debug info section. Opcodes advance position and line simultaneously. More compact than JVM's table for methods with many statements.

**Wasm DWARF:** Full DWARF debug info in custom sections (`.debug_info`, `.debug_line`, `.debug_abbrev`, etc.). This is the most expressive option — DWARF can represent inlined functions, optimized-out variables, and complex scope structures. The cost is complexity and size.

**Source maps as a separate section:** The JVM, CPython, and Lua approach — source maps are stored alongside bytecode in the same file but in clearly delineated sections/attributes. This allows stripping debug info without modifying the bytecode.

### 11.2. String Tables and Symbol Pools

Almost every bytecode format includes a string table or constant pool:

| Format | Name | Scope | Encoding |
|--------|------|-------|----------|
| ELF | `.strtab` / `.shstrtab` | per-file | null-terminated, deduplicated |
| JVM | Constant Pool | per-class | tagged entries with back-references |
| Wasm | (none — names in custom section) | per-module | LEB128 length-prefixed UTF-8 |
| DEX | `string_ids` + `string_data` | per-file (shared) | MUTF-8, LEB128 length-prefixed |
| Lua | per-prototype constants | per-function | type-tagged, length-prefixed |
| BEAM | `AtU8` chunk | per-module | count + length-prefixed atoms |
| Luau | global string table | per-file (shared) | varint length-prefixed |

The DEX and Luau approach (global shared pool) is most space-efficient when strings are reused across functions. The Lua 5.x approach (per-function) is simplest but duplicates strings used in multiple functions. The JVM constant pool is the richest (stores typed references, not just strings) but the most complex to parse.

---

## 12. Summary of Techniques

| Technique | Used By | Key Trade-off |
|-----------|---------|---------------|
| Fixed header + section table | ELF, PE, DEX | O(1) section access; requires header update for new sections |
| Linear load commands | Mach-O | Simple sequential processing; O(n) to find a specific command |
| IFF-style tagged chunks | BEAM, IFF, RIFF | Maximally extensible; O(n) scan but trivial to skip unknowns |
| Ordered known sections | Wasm | Streaming compilation; rigid ordering required |
| Recursive prototype dump | Lua 5.x, CPython | Simple; cannot skip to specific function without full parse |
| Bottom-up prototype dump | LuaJIT | Single-pass loading; children before parents |
| Global shared string pool | DEX, Luau | Space-efficient; requires pool to be loaded first |
| Per-unit constant pool | JVM, Lua 5.x | Self-contained per class/function; may duplicate strings |
| Explicit relocation table | ELF, PE | Clean separation; load-time patching cost |
| Relocation bitmap | Gforth | Ultra-compact for dense relocations; wasteful for sparse |
| No relocations (index-based) | JVM, Wasm, DEX | Zero load-time patching; requires dispatch indirection |
| Zero-copy / mmap-friendly | FlatBuffers, Cap'n Proto | Instant access; padding overhead, relative pointer cost |
| Self-modifying preparation | SableVM, Mage | Fast steady-state; complex first-execution path |
| LEB128 variable-length | Wasm, DEX, DWARF, LuaJIT | Compact; no random access without index |
| Delta encoding | DWARF, Source Maps | Exploits locality; requires sequential processing |
| Length-prefixed sections | Wasm, LuaJIT protos | Enables skipping; costs one size field per section |
| Version-gated fields | Luau, DEX | Simple evolution; decoder must know all versions |
| Attribute extensibility | JVM, ELF | Graceful unknown handling; extra per-attribute overhead |

---

## 13. References

- System V ABI: ELF Object File Format specification. https://gforth.org/manual/Image-File-Background.html
- ELF Header: https://refspecs.linuxfoundation.org/elf/gabi4+/ch4.eheader.html
- ELF Sections: https://refspecs.linuxfoundation.org/elf/gabi4+/ch4.sheader.html
- ELF Relocations: https://gabi.xinuos.com/elf/06-reloc.html
- OSDev Wiki, "ELF": https://wiki.osdev.org/ELF
- Microsoft PE Format: https://learn.microsoft.com/en-us/windows/win32/debug/pe-format
- 0xRick, "PE Base Relocations": https://0xrick.github.io/win-internals/pe7/
- Apple Mach-O Reference (mirror): https://github.com/aidansteele/osx-abi-macho-file-format-reference
- William Woodruff, "Mach-O Internals" (2016): https://yossarian.net/res/pub/macho-internals/macho-internals.pdf
- JVM Specification Chapter 4, "The class File Format": https://docs.oracle.com/javase/specs/jvms/se19/html/jvms-4.html
- WebAssembly Binary Format: https://webassembly.github.io/spec/core/binary/modules.html
- WebAssembly Custom Sections: https://webassembly.github.io/spec/core/appendix/custom.html
- Android DEX Format: https://source.android.com/docs/core/runtime/dex-format
- Mandiant, "Delving into Dalvik: A Look Into DEX Files" (2024): https://cloud.google.com/blog/topics/threat-intelligence/dalvik-look-into-dex-files
- Jonathan Levin, "Dalvik and ART": https://newandroidbook.com/files/Andevcon-DEX.pdf
- Erlang `beam_lib`: https://www.erlang.org/doc/apps/stdlib/beam_lib.html
- Lua 5.4 source, `lundump.c`: https://www.lua.org/source/5.4/lundump.c.html
- Matthew G., "Lua Bytecode 5.1 Specification" (2024): https://matthewg-rev.github.io/2024/09/17/lua-bytecode_51_specification_part_1.html
- LuaJIT Bytecode Dump Format: https://github.com/RedHolms/LuaJIT-BytecodeDumpFormat
- spxnso, "Understanding Luau Bytecode Structure" (2026): https://spxnso.dev/blog/understanding-luau-bytecode-structure-part-1
- Ned Batchelder, "The structure of .pyc files" (2008): https://nedbatchelder.com/blog/200804/the_structure_of_pyc_files.html
- PEP 657, "Fine-grained error locations in tracebacks": https://peps.python.org/pep-0657/
- PEP 552, "Deterministic pycs": https://peps.python.org/pep-0552/
- Ulrich Drepper, "Text Relocations" (2006): https://akkadia.org/drepper/textrelocs.html
- Gagnon & Hendren, "Effective Inline-Threaded Interpretation of Java Bytecode Using Preparation Sequences" (CC 2003)
- Piumarta & Riccardi, "Optimizing direct threaded code by selective inlining" (PLDI 1998): https://www.piumarta.com/papers/pldi98-opt.pdf
- Gforth Manual, "Image Files": https://gforth.org/manual/Image-File-Background.html
- V8 blog, "Lazy deserialization": https://v8.dev/blog/lazy-deserialization
- hashseed (Yang Guo), "Improving V8's performance using the serializer/deserializer" (2015): https://hashseed.blogspot.com/2015/03/improving-v8s-performance-using.html
- Joyee Cheung, "Reproducible Node.js built-in snapshots" (2024): https://joyeecheung.github.io/blog/2024/09/28/reproducible-nodejs-builtin-snapshots-1/
- Kvakil, "Node.js Startup: Speeding up Snapshot Deserialization" (2023): https://www.kvakil.me/posts/2023-05-11-nodejs-startup-series-externalizing-startup-strings.html
- Squeak Wiki, ".image file": https://wiki.squeak.org/squeak/2213
- Squeak Wiki, "ImageFormat": https://wiki.squeak.org/squeak/6290
- Google FlatBuffers: https://flatbuffers.dev/
- Cap'n Proto Encoding: https://capnproto.org/encoding.html
- Cap'n Proto FAQ: https://capnproto.org/faq.html
- Wikipedia, "Threaded code": https://en.wikipedia.org/wiki/Threaded_code
- Mickaël Walter, "Reverse engineering LuaJIT" (2020): https://www.mickaelwalter.fr/reverse-engineering-luajit/
- Clément Béra & Eliot Miranda, "A bytecode set for adaptive optimizations" (IWST 2014)
- Columbia University, "Dynamic Reconstruction of Relocation Information for Stripped Binaries" (2014): http://nsl.cs.columbia.edu/projects/minestrone/papers/reloc.pdf