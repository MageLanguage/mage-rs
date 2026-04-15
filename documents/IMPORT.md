# Import and Export

Research on import and export designs across languages and runtimes, with emphasis on namespace boundaries, file evaluation, explicit public API values, runtime loading, static linkage, and interactions with testing and tooling.

---

## 1. Textual Inclusion

### 1.1. C `#include` — Preprocessor Concatenation

C’s `#include` is not a module system. It is textual inclusion: the preprocessor copies the contents of another file into the current translation unit before parsing.

This is extremely simple and historically important, but it comes with predictable costs:

- no namespace boundary by default
- no explicit public API object
- order matters
- macros and textual substitution leak freely
- build systems and include paths become part of semantic behavior

The advantage is raw flexibility. The disadvantage is that inclusion is not evaluation, not import, and not encapsulation. Languages that want explicit runtime values, tracing, or lazy execution usually move away from this model quickly.

The key lesson is not “never include text”, but rather: textual inclusion is a distinct mechanism and should not be confused with module import. Racket’s distinction between `require` and `include` makes this especially clear.

---

## 2. Runtime Module Evaluation Returning a Value

### 2.1. Lua — `require()` Returning a Table

Lua’s most successful module pattern is simple:

- create a local table
- place public functions and values on it
- return that table

Example style:

```mage-rs/documents/IMPORT.md#L1-8
-- math_utils.lua
local M = {}

function M.add(a, b)
    return a + b
end

return M
```

Consumer side:

```mage-rs/documents/IMPORT.md#L1-4
local math_utils = require("math_utils")
print(math_utils.add(2, 3))
```

This pattern is attractive because it makes the public API a first-class value. The module result is inspectable, passable, nestable, and naturally namespaced through member access.

Lua community discussions repeatedly converge on this as the cleanest approach. The older `module(...)` approach and environment-based tricks are more magical and less explicit. The lua-users discussions on alternative module definitions are particularly useful because they compare explicit returned namespace tables against ambient module state and show why explicit returns scale better.

Useful lessons:

- one file can return one explicit namespace value
- private names remain private naturally
- re-exporting is straightforward
- the public API is easy to document
- lazy procedure values fit naturally inside the returned namespace

This is one of the strongest reference designs for value-oriented module systems.

Sources:
- lua-users: Alternative Module Definitions
- lua-users: Namespaces And Modules
- Programming in Lua package/module patterns

### 2.2. CommonJS — Executed File Returning `module.exports`

CommonJS is also runtime-oriented. A module executes and exposes its public surface through `module.exports`. Importing the module returns that exported value.

Module definition:

```mage-rs/documents/IMPORT.md#L1-7
// math.cjs
module.exports = {
  add(a, b) {
    return a + b;
  }
};
```

Consumer side:

```mage-rs/documents/IMPORT.md#L1-4
const math = require("./math.cjs");
console.log(math.add(2, 3));
```

This has many of the same strengths as Lua’s returned table pattern:

- import result is a real value
- exports are inspectable
- runtime loading is straightforward
- object-shaped module APIs are easy to consume

However, CommonJS also shows several pitfalls:

- ambient mutable export object can become messy
- caching is usually implicit and global
- hidden process-wide sharing can surprise users
- top-level side effects happen at load time
- export shape can become accidental instead of intentional

The best takeaway from CommonJS is not its syntax, but its core semantic fact:

> importing a module returns one concrete value

That is often better than named export systems when the language’s execution model is runtime-first.

Useful discussions around CommonJS vs ES modules repeatedly highlight the tension between runtime convenience and static analyzability. Those discussions are useful because they show where module systems become architectural rather than syntactic.

Sources:
- CommonJS practice discussions
- Hacker News threads on CommonJS vs ES modules
- Node module system comparisons

### 2.3. Runtime-Executed Namespace Objects in Scripting Systems

Many scripting and plugin systems converge on a similar shape even without a formal standard:

- execute file
- produce object or environment
- hand object back to caller

This appears in ad hoc plugin systems, some Scheme implementations, some game scripting systems, and many internal DSL environments.

The recurring reason is simple: a returned namespace object is easy to reason about. It avoids the need for a special global registry or hidden export channel.

---

## 3. Static Module Linkage

### 3.1. ECMAScript Modules — Linked Before Evaluation

ECMAScript modules are the strongest modern example of a static module system:

- imports and exports are part of syntax
- the graph is linked before evaluation
- imported bindings are live bindings
- namespace imports expose module namespace objects
- tooling can statically analyze dependency structure well

Module definition:

```mage-rs/documents/IMPORT.md#L1-6
// math.js
export function add(a, b) {
  return a + b;
}
```

Consumer side with namespace import:

```mage-rs/documents/IMPORT.md#L1-4
import * as math from "./math.js";
console.log(math.add(2, 3));
```

This model is excellent for:

- tree shaking
- static bundling
- browser-native dependency graphs
- explicit export contracts

However, it assumes a semantic split between:

- module linking
- then evaluation

That makes it less suitable for languages where the meaning of declarations depends on runtime state or execution order. ES module design discussions, especially old es-discuss threads on module naming and declarations, are useful because they expose how often external file identity, internal namespace naming, and declaration mechanisms get conflated.

The main lesson to keep from ESM is:

- exports are a contract
- explicit public API matters
- namespace import objects are useful

But the static linking model itself is not always the right fit for runtime-oriented languages.

Sources:
- ECMAScript specification
- MDN module docs
- es-discuss threads on module import and naming

### 3.2. TypeScript — Syntax on Top of Host Module Systems

TypeScript is useful not because it introduces a radically new module system, but because it clarifies how language-level `import`/`export` syntax interacts with different runtime/module backends.

Module definition:

```mage-rs/documents/IMPORT.md#L1-6
// math.ts
export function add(a: number, b: number): number {
  return a + b;
}
```

Consumer side:

```mage-rs/documents/IMPORT.md#L1-4
import { add } from "./math";
console.log(add(2, 3));
```

The strongest lesson from TypeScript’s module documentation is that modules are API boundaries first. Syntax is only one part; emitted behavior depends on the host system and build configuration.

That is a useful reminder that module system design cannot be divorced from artifact generation and runtime policy.

Sources:
- TypeScript handbook on modules
- TypeScript module reference

---

## 4. Namespace-Oriented Module Systems

### 4.1. Racket — `require` and `provide`

Racket is an extremely valuable reference because it distinguishes several mechanisms that many languages blur together:

- `require` — import module bindings
- `provide` — export selected bindings
- `include` — textual inclusion
- `load` / `dynamic-require` — runtime loading variants
- namespaces as first-class runtime structures

Module definition:

```mage-rs/documents/IMPORT.md#L1-6
#lang racket
(provide add)

(define (add a b)
  (+ a b))
```

Consumer side:

```mage-rs/documents/IMPORT.md#L1-4
#lang racket
(require "math.rkt")
(displayln (add 2 3))
```

This separation is powerful. It clarifies that:

- import is not include
- module loading is not the same as namespace injection
- export should be explicit
- dynamic loading has different semantics than ordinary module imports

Racket’s module system also strongly emphasizes scope isolation. Modules are separate units with explicit exported surfaces. At the same time, Racket’s namespace manipulation APIs show that runtime-controlled loading and isolated execution contexts are possible and important.

Two especially relevant lessons:

1. `require` + `provide` is preferable to textual inclusion for reusable libraries.
2. if a language supports runtime-controlled execution contexts, namespace and module instance identity must be handled carefully.

Discussions around `require` vs `load` vs `include` are especially useful because they expose the costs of conflating these concepts.

Sources:
- Racket Guide: Modules, require, provide
- Racket Reference: require/provide
- Racket namespace manipulation docs
- discussions on dynamic-require and namespace instantiation

### 4.2. Scheme Family Surveys — Modules Mean Many Different Things

Scheme discussions are useful because they openly acknowledge that “module system” can mean wildly different things:

- first-class environments
- syntactic modules
- meta-modules
- library declarations
- load-order control
- visibility control

Scheme module surveys and philosophy essays repeatedly emphasize two facts:

- visibility control and evaluation order are different concerns
- module systems should not accidentally conflate packaging, syntax extension, and runtime loading

This is useful because it prevents shallow thinking. “Import/export” is not a single solved pattern; it is a cluster of design choices.

Sources:
- R7RS module system discussions
- Scheme module system surveys
- essays on philosophy of Scheme modules
- SISC modules and libraries documentation

### 4.3. Julia — Explicit Exports in Separate Namespaces

Julia’s module system is namespace-first:

- each module creates a new global scope
- exports are explicit
- files and modules are related but not identical
- `include` and module declarations are distinct mechanisms

Module definition:

```mage-rs/documents/IMPORT.md#L1-7
module MathUtils

export add

add(a, b) = a + b

end
```

Consumer side:

```mage-rs/documents/IMPORT.md#L1-4
using .MathUtils

println(add(2, 3))
```

The key lesson is the value of separating:

- file layout
- namespace construction
- public API export

Julia is particularly useful as a reminder that files and modules do not have to be the same thing. That is powerful, but also adds complexity. For languages that already treat files as executable units, this separation may be unnecessary overhead.

Still, Julia strongly reinforces the point that:

- explicit public surface is valuable
- namespace discipline matters
- inclusion and module import should remain distinct concepts

Source:
- Julia manual on modules

---

## 5. Selective, Renamed, and Scoped Imports

### 5.1. D — Basic, Selective, Renamed, Static, and Public Imports

D provides one of the most instructive import designs among C-like languages. Its module system includes:

- basic imports
- selective imports
- renamed imports
- static imports
- scoped imports
- public imports

Module definition:

```mage-rs/documents/IMPORT.md#L1-5
module math;

int add(int a, int b) {
    return a + b;
}
```

Consumer side with basic import:

```mage-rs/documents/IMPORT.md#L1-4
import math;
import std.stdio;

void main() {
    writeln(add(2, 3));
}
```

Consumer side with static import:

```mage-rs/documents/IMPORT.md#L1-4
static import math;
import std.stdio;

void main() {
    writeln(math.add(2, 3));
}
```

This is useful because it shows several tensions clearly:

- convenience vs namespace pollution
- local clarity vs verbosity
- direct symbol import vs qualified access
- re-exporting dependencies vs explicit repeated imports

The D specification and community discussions make it clear that modules are not textual inclusion. A file corresponds to a module namespace, symbols are imported into symbol tables, and there are deliberate choices for how much namespace folding occurs.

Particularly useful lessons:

- `static import` is a good tool for avoiding namespace pollution
- selective imports make dependencies explicit
- public imports can reduce user verbosity but may hide dependency structure
- renamed imports improve ergonomics but need restraint

D is a good reference when thinking about import forms beyond the all-or-nothing namespace object pattern.

Sources:
- D language module spec
- D forum explanations of import semantics
- discussions on namespace pollution and static import

### 5.2. Nim — Import, Include, and Export Forwarding

Nim distinguishes:

- `import` — module import
- `include` — textual inclusion
- `export` — forwarding imported modules or symbols

Module definition:

```mage-rs/documents/IMPORT.md#L1-5
# mathutils.nim
proc add*(a, b: int): int =
  a + b
```

Consumer side:

```mage-rs/documents/IMPORT.md#L1-4
import mathutils

echo add(2, 3)
```

Nim is interesting because it combines explicit export marks on declarations with module forwarding. Style guidance in Nim also warns about namespace pollution and recommends careful imports and re-exports where public types depend on external modules.

Useful lessons:

- forwarding or re-exporting can be a first-class part of the design
- include and import should remain distinct
- naming collisions and order sensitivity become real in global/shared namespaces

Sources:
- Nim manual on modules
- Nim style guide import/export section

### 5.3. Elixir — Alias, Import, Require, and Use

Elixir is very useful because it deliberately separates several operations that many languages merge:

- `alias` — shorten a module name
- `import` — bring functions/macros into local scope
- `require` — ensure macros are available
- `use` — invoke extension-point behavior and code injection

Module definition:

```mage-rs/documents/IMPORT.md#L1-6
defmodule MathUtils do
  def add(a, b) do
    a + b
  end
end
```

Consumer side with alias:

```mage-rs/documents/IMPORT.md#L1-4
defmodule Demo do
  alias MathUtils, as: Math

  def run do
    Math.add(2, 3)
  end
end
```

Consumer side with import:

```mage-rs/documents/IMPORT.md#L1-4
defmodule Demo do
  import MathUtils, only: [add: 2]

  def run do
    add(2, 3)
  end
end
```

This separation is valuable because it shows that “import” need not mean “load module”. Often what programmers really want is one of:

- aliasing
- namespace folding
- macro availability
- extension injection

Elixir’s documentation and community style guidance repeatedly suggest preferring `alias` over `import` for ordinary code, because it preserves origin clarity and avoids namespace confusion.

Useful lessons:

- namespace folding is a separate concern from loading
- module aliasing improves readability with less ambiguity
- extension-point injection should not be confused with import
- import forms should be explicit about what they affect

Sources:
- Elixir docs: alias, require, import, use
- Elixir community discussions and style guidance

---

## 6. Namespaces, Naming, and File Identity

### 6.1. External File Identity vs Internal Namespace Identity

Several design discussions, especially around JavaScript modules and Scheme systems, emphasize a subtle but important point:

- the external thing you locate on disk or through a package manager
- and the internal namespace or name by which code refers to it

are not the same concern.

When these are conflated, systems become brittle:

- naming becomes awkward
- package renaming becomes hard
- vendoring multiple versions becomes painful
- import syntax starts doing too many jobs

This shows up in:
- es-discuss debates on module naming
- Rust-like discussions on crates vs modules
- C++ modules and build-system integration issues
- surveys on module system terminology

The lesson is:

> import resolution policy and internal namespace exposure should be separable concepts

### 6.2. Build Systems and Modules Are Deeply Entangled

C++ module discussions, Oberon-family notes, and other design blogs repeatedly show that module systems are not just parser features. They are deeply connected to:

- file lookup
- package layout
- compilation units
- artifact generation
- dependency resolution
- caching policy

Any serious import/export design must account for build and artifact behavior early.

Sources:
- C++ modules discussions
- Oberon import design commentary
- module-system design notes from language blogs

---

## 7. Public API Shape

### 7.1. Returning One Explicit Namespace Object

Across many ecosystems, the most robust pattern is still:

- build one namespace-like object
- place intended public fields on it
- return it

Advantages:

- explicit public API boundary
- private names remain local
- natural member access
- stable debugger representation
- easy re-exporting
- good fit for lazy executable values
- good fit for artifact generation from chosen entry values

This is the strongest generally applicable recommendation in this survey.

### 7.2. Auto-Export of All Top-Level Names

Some systems implicitly expose many or all top-level declarations. This is convenient in the short term but causes long-term problems:

- accidental API leakage
- hard refactoring
- weak documentation of public contract
- hard-to-see dependency propagation

The broad lesson from many ecosystems is:

> explicit export beats implicit full-surface export

### 7.3. Named Exports vs Single Namespace Value

Named export systems can be excellent when:
- static tooling matters most
- module linkage happens before evaluation
- file/module identity is stable and explicit

Single namespace return values are better when:
- runtime evaluation is primary
- import result should be a first-class value
- lazy unresolved values should sit naturally inside exported APIs
- testing and tracing should follow real execution

---

## 8. Import Caching and Isolation

### 8.1. Caching Is a Policy, Not a Universal Truth

Lua and CommonJS popularized the idea that loading a module once and caching it is convenient. That is true in many applications, but it is not universally correct.

Caching interacts with:

- side effects
- module initialization
- mutable module state
- testing isolation
- replay/debugging reproducibility

Therefore, import caching should be treated as a runtime policy, not an inherent semantic law.

### 8.2. Tooling and Tests Need Fresh Worlds

For testing, tracing, and debugging, the recurring best practice is:

- isolate runs
- reload dependencies fresh for each isolated run
- do not let one execution leak hidden state into another

This shows up in many systems indirectly, even where it is not the default runtime behavior. It is particularly important for:

- execution-driven diagnostics
- time-travel traces
- deterministic replay
- reliable imports in tool-driven environments

---

## 9. Lazy Compilation Compatibility

### 9.1. Imported Procedures as Unresolved Values

A runtime-oriented import model pairs naturally with lazy compilation if imported module APIs can hold unresolved executable values.

A good pattern:

- file executes
- builds namespace object
- namespace contains procedures
- each procedure can carry source reference
- compiled representation is created later on first execution

This is much more natural with a returned namespace object than with static named export linkage.

### 9.2. Do Not Eagerly Compile Nested Executable Units on Import

Importing a file should not require compiling every nested executable block inside it. The strongest compatible model is:

- file-level code executes as needed
- exported procedures may remain unresolved
- nested branches/blocks inside those procedures remain unresolved
- first actual entry triggers compilation of the relevant executable unit

This preserves lazy semantics and keeps import from becoming accidental whole-program compilation.

---

## 10. Good Practices

### 10.1. Keep Private Scope Separate from Public API
Do not rely on all top-level declarations being public.

### 10.2. Return One Explicit Public Value
Prefer a namespace-like object value.

### 10.3. Re-export Deliberately
If a module wants to expose dependencies, do so intentionally on the returned namespace object.

### 10.4. Keep Export Surface Small
Exports are a contract. Smaller contracts are easier to evolve.

### 10.5. Distinguish Include from Import
Textual inclusion and module import are different mechanisms.

### 10.6. Distinguish Alias from Import
Shortening a module name is a different concern from importing its contents.

### 10.7. Distinguish Extension Injection from Import
Macro or code-injection facilities should not be confused with ordinary module import.

### 10.8. Do Not Hide Module Policy in Tooling
Import behavior, reload behavior, and isolation behavior should be explicit enough that tooling and runtime agree.

---

## 11. Example Patterns

### 11.1. File Returning Explicit Namespace

```mage-rs/documents/IMPORT.md#L1-10
add : (procedure {a : U64; b : U64}, U64) {
    return a + b;
};

subtract : (procedure {a : U64; b : U64}, U64) {
    return a - b;
};

export Class {
    add      : add;
    subtract : subtract;
};
```

### 11.2. Re-exporting Another File

```mage-rs/documents/IMPORT.md#L1-8
math : import "math.hex";

double : (procedure {x : U64}, U64) {
    return math.add x, x;
};

export Class {
    math   : math;
    double : double;
};
```

### 11.3. Imported Namespace Used in a Test

```mage-rs/documents/IMPORT.md#L1-4
example : import "example.hex";

assert (example.add 2, 2), 4;
```

### 11.4. Artifact Generation from Explicit Entry Value

```mage-rs/documents/IMPORT.md#L1-9
core : import "core.hex";

main : (procedure Void, U8) {
    return 0d0;
};

bytecode "example", main;
```

---

## 12. Interesting References

### 12.1. Lua Community Module Patterns
- lua-users: Alternative Module Definitions
- lua-users: Namespaces And Modules
- Lua package/module discussions and style patterns

Why useful:
- explicit returned namespace objects
- private locals vs public API
- alternate namespace execution ideas

### 12.2. Racket Module and Namespace Docs
- Racket Guide: Modules
- Racket Guide: Module Basics
- Racket Reference: `require` / `provide`
- Racket namespace manipulation docs
- discussions of `require` vs `load` vs `include`

Why useful:
- sharp distinction between import, load, include, and namespace manipulation
- explicit export discipline
- module instance and namespace isolation

### 12.3. D Module System
- D language spec: Modules
- D community explanations of `import`, `static import`, selective import, public import

Why useful:
- selective imports
- renamed imports
- qualified access vs namespace folding
- re-export mechanisms

### 12.4. Nim Module System
- Nim manual on modules
- Nim style guidance on import/export/include

Why useful:
- distinction between import and include
- forwarding exports
- practical guidance on public dependency surfaces

### 12.5. Elixir Module Directives
- official Elixir docs on `alias`, `require`, `import`, and `use`
- community discussions about preferring alias over import

Why useful:
- separates namespace shortening, loading, macro enablement, and code injection
- prevents “import does everything” confusion

### 12.6. JavaScript and es-discuss Material
- ECMAScript specification and MDN module docs
- es-discuss threads on module naming and declarations
- community discussions comparing ESM and CommonJS

Why useful:
- export as contract
- namespace import objects
- external identity vs internal naming
- static vs runtime trade-offs

### 12.7. Broader Module-System Surveys
- Scheme module-system surveys
- “A Philosophy on Scheme Modules”
- language-design notes on module systems
- historical discussions on module terminology

Why useful:
- reminds us there is no single “module system” concept
- separates visibility, evaluation order, packaging, and environments

---

## 13. Summary by Mechanism Family

### 13.1. Textual Inclusion

Representative systems:
- C `#include`
- language-level `include` forms
- file concatenation approaches

Core property:
- another file’s text becomes part of the current compilation unit

Best use:
- splitting one logical unit into multiple source files

Main risks:
- no real module boundary
- accidental leakage
- order sensitivity
- poor debugger and tooling boundaries

### 13.2. Runtime Module Value Systems

Representative systems:
- Lua `require()` returning a table
- CommonJS returning `module.exports`
- plugin and scripting systems that execute files and return objects

Core property:
- importing executes code and returns a value

Best use:
- runtime-oriented languages
- systems where file evaluation matters
- languages with lazy executable values

Main strengths:
- import result is a first-class value
- explicit namespace object works naturally
- easy fit for lazy compilation

Main risks:
- hidden caching
- top-level side effects
- accidental global state if runtime policy is unclear

### 13.3. Static Linkage Module Systems

Representative systems:
- ECMAScript modules
- ML-family module linkage styles
- package-level static dependency systems

Core property:
- import/export graph is established before evaluation

Best use:
- ahead-of-time linking
- strong static tooling
- explicit dependency graphs

Main strengths:
- analyzable imports
- explicit contract surface
- good fit for static optimization and bundling

Main risks:
- awkward in runtime-first languages
- may fight declaration-time semantics
- can over-separate linking and execution

### 13.4. Namespace-Centric Module Systems

Representative systems:
- Racket `require` / `provide`
- Julia modules
- many Scheme-family systems

Core property:
- modules create explicit namespaces with controlled visibility

Best use:
- languages that want strong visibility control
- environments with multiple loading and namespace operations

Main strengths:
- precise public/private boundaries
- separate namespace manipulation from textual inclusion
- strong conceptual clarity when well designed

Main risks:
- more concepts to learn
- files and modules may drift apart
- can be heavier than needed if files already behave as executable units

### 13.5. Import Refinement Systems

Representative systems:
- D selective / renamed / static / public imports
- Nim import vs export forwarding
- Elixir alias / import / require / use split

Core property:
- import is not one thing; namespace folding, qualification, macro availability, and code injection are separate operations

Best use:
- large codebases
- ecosystems where name conflicts and readability both matter

Main strengths:
- better control of namespace pollution
- more expressive dependency surfaces
- clearer separation of concerns

Main risks:
- more surface complexity
- misuse if distinctions are unclear
- can become stylistically inconsistent across projects

### 13.6. Strongest Common Lessons

Across these families, the recurring lessons are:

1. textual inclusion and module import should stay distinct
2. explicit public API beats implicit auto-export
3. returning one namespace-like value is one of the simplest robust runtime designs
4. import caching should be treated as policy, not hidden law
5. import resolution, namespace exposure, and artifact generation should not be carelessly conflated
6. tooling behavior should follow real execution semantics when diagnostics depend on execution

## 14. Open Questions

These are often policy questions rather than one-size-fits-all truths:

1. Should normal runtime imports be memoized within a single runtime?
2. How should path resolution work across local files, installed packages, and standard libraries?
3. Should the language expose multiple import-like mechanisms explicitly, or one import plus auxiliary alias/include mechanisms?
4. Is the exported namespace object always a single returned value, or can richer wrappers exist later?
5. How much of import/reload policy should be language-level versus runtime-level?

---

## 15. Final Recommendation

The strongest general recommendation from this survey is:

- treat files as executable units
- treat import as runtime evaluation that returns one value
- treat export as construction of one explicit public namespace value
- avoid auto-export of all top-level names
- keep private execution scope separate from returned public API
- support lazy executable values inside exported namespace objects
- treat import caching and test isolation as explicit policies, not hidden magic
- keep include, alias, import, and extension-injection as distinct concepts

In compact form:

1. a file executes
2. it returns one exported value
3. import evaluates the file and obtains that value
4. that value is usually a namespace-like object
5. exported executable values can remain unresolved until first use
6. tooling should exercise this through real isolated execution when diagnostics depend on execution

That is the model this document recommends.