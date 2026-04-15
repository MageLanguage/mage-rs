# Type Systems

Research on practical type system designs, inference algorithms, runtime enforcement strategies, and implementation tradeoffs across statically typed, gradually typed, and dynamically typed languages.

---

## 1. Framing the Design Space

A language type system is not one thing. It is a bundle of choices about:

- what values carry type information
- when types are checked
- how much is inferred
- how much is explicit
- whether types exist only statically or also at runtime
- whether untyped and typed code may interoperate
- whether type information drives optimization, safety, tooling, or all three

For languages with runtime-visible semantics, deferred execution, or execution-driven validation, architectural constraints may matter more than abstract elegance.

Such languages should not simply copy a classic whole-program static type checker. They need a type architecture that works with:

- runtime-visible type values
- declaration-time resolution
- deferred executable materialization
- execution-driven semantic discovery
- VM-friendly representation

The most important distinction is not “static vs dynamic”, but:

- **where type information lives**
- **when it becomes binding**
- **how much global knowledge the checker assumes**
- **how expensive the model is to implement and run**

---

## 2. Conservative, Strong, and Marginal Type System Families

There are several families of type systems worth studying. Some are strong conservative picks. Others are “marginal but efficient”: limited or partial systems that are much easier to build and can still provide substantial value.

---

## 3. Hindley–Milner — The Classic Global Inference Baseline

### 3.1. What it is

Hindley–Milner (HM) is the classic polymorphic type inference system of ML-family languages. It provides:

- principal types
- type inference without many annotations
- let-polymorphism
- unification-based inference

The canonical implementation family is:

- generate type variables
- unify constraints
- generalize at let-bindings
- instantiate at use sites

This is the most famous “small powerful type system” because it gives a lot of usability for relatively modest machinery.

### 3.2. Why it is attractive

HM is attractive because it offers:

- compact core theory
- pleasant ergonomics
- principal types
- efficient implementations in practice
- strong fit for expression-oriented functional languages

It is the baseline many language designers compare against, even when their language is not a perfect fit.

### 3.3. Why it is a poor direct fit for runtime-first architectures

HM assumes several things that do not align cleanly with runtime-first architectures:

- declarations are usually processed in a relatively global static context
- semantic validity is largely a property of the source program before execution
- polymorphism is usually built around static let-generalization
- type inference is not designed around declaration-time runtime resolution
- types are usually not first-class runtime values in the same sense Mage wants

Most importantly, HM is strongest when:

- expression typing is mostly static
- inference sees enough of the surrounding program
- execution order does not control declaration existence

Some runtime-first architectures say the opposite in several key places.

### 3.4. Useful pieces to borrow

Even if Mage does not adopt HM wholesale, it should still borrow:

- unification for monotypes
- principal-shape style reasoning where possible
- let-bound generalization ideas for explicit or implicit polymorphism later
- level-based generalization techniques for efficient implementations

### 3.5. Practical verdict

**HM is a useful implementation ingredient, but not a good top-level semantic model for Mage.**

Languages with runtime-first semantics may still use HM-like mechanisms inside a more local or bidirectional framework, especially for:

- procedure bodies
- local monomorphic inference
- future generic or polymorphic features

---

## 4. Bidirectional Typing — The Strong Conservative Practical Choice

### 4.1. What it is

Bidirectional typing splits typing into two judgments:

- **synthesis**: expression produces a type
- **checking**: expression is checked against an expected type

This structure has become one of the strongest practical designs for new languages because it scales well to richer type systems while preserving implementability and error locality.

### 4.2. Why it is so popular

Bidirectional typing is popular because it offers:

- strong error locality
- relatively straightforward implementation
- explicit control over where annotations are required
- extensibility to many advanced features
- a good balance between ergonomics and predictability

It works especially well when:

- introduction forms check
- elimination forms synthesize
- annotations are used where inference would otherwise become expensive or ambiguous

### 4.3. Why it fits runtime-first architectures very well

Bidirectional typing is one of the best matches for runtime-first architectures.

Reasons:

- it tolerates partial local information better than HM-style global inference
- it naturally supports explicit annotations in languages that already prefer them
- it can be layered over runtime-first semantics
- it gives much better control over error timing and locality
- it scales from minimal checking to richer future type features

Many explicit procedure declaration styles are already annotation-rich:

```mage
add : (procedure {x : U64; y : U64}, U64) {
    return x + y;
}
```

That means such languages are naturally positioned for a bidirectional discipline:

- declarations provide expected types
- bodies can be checked against them
- expressions synthesize where obvious
- annotations remain meaningful documentation

### 4.4. A likely adaptation for runtime-first languages

A strong practical direction for such languages would be:

- declarations bind names to explicit type values
- expressions synthesize types where local
- block bodies and returns are checked against known expected types
- runtime values also carry type identity
- unreachable code is not deeply checked until entered

This is not textbook bidirectional typing, but it is a bidirectional architecture adapted to a runtime-first model.

### 4.5. Practical verdict

**Bidirectional typing is one of the strongest conservative implementation choices for runtime-first languages.**

If such a language wants a real, maintainable, extensible type checker, it should almost certainly be organized bidirectionally.

---

## 5. Local Type Inference — The Best Companion to Bidirectional Typing

### 5.1. What it is

Local type inference, in the sense of Pierce and Turner, is a family of techniques for recovering omitted type information from nearby context instead of requiring whole-program inference.

It is especially useful when full inference is:

- undecidable
- too complex
- too slow
- too difficult to explain in error messages

### 5.2. Why it matters

Many practical languages want:

- richer types than plain HM
- some omitted annotations
- predictable errors
- limited inference burden

Local inference is the sweet spot.

### 5.3. Why it fits runtime-first architectures

Such languages often want:

- explicit declaration types
- low annotation burden in common expressions
- no global speculative type analysis
- entered-code-only semantic commitment

That makes local inference much more attractive than global inference.

Possible uses in Mage:

- inferring anonymous or nested local variable types from initializer
- inferring omitted procedure call type arguments in a future generic system
- checking block results against expected types
- inferring lambda or procedure parameter details from expected callable type if such syntax appears later

### 5.4. Practical verdict

**Local type inference should likely be part of Mage from early on, but only as a servant of bidirectional typing, not as the main design identity.**

---

## 6. Monomorphic First-Assignment Typing — The Strong “Marginal but Efficient” Option

### 6.1. What it is

A variable gets its type from its first assigned value and that type cannot later change.

This is a common choice in languages that want stable runtime-visible variable identity.

This is not sophisticated polymorphic inference. It is a runtime-visible, declaration-sensitive monomorphic typing discipline.

### 6.2. Why it is powerful despite being simple

This model is much simpler than HM or advanced bidirectional polymorphism, but it gives a lot:

- variables always have stable type identity
- assignment can be checked cheaply
- runtime representation can be compact
- declaration-time semantics remain central
- error messages can be clear

It also matches the architecture directly.

### 6.3. Why it may be the right first working type system

Before implementing full richer type semantics, a language can get very far with:

- builtin type values
- value-to-type association
- variable binding fixes type at creation
- later assignments must match
- procedure values have explicit callable type
- `Source` and `Void` are real first-class types

This is a “marginal but efficient” system because it does not solve everything, but it is:

- cheap to implement
- easy to explain
- strongly aligned with Mage architecture

### 6.4. Practical verdict

**This is often a strong candidate for a language’s first implemented type system layer.**

Not the final one, but the first real one.

---

## 7. Nominal vs Structural Typing

### 7.1. Nominal typing

Nominal systems say types are equal or compatible because of declaration identity.

Examples:
- Java
- C#
- Rust structs and enums
- many VM-friendly languages

Benefits:
- cheap runtime checks
- stable identity
- simple metadata
- good fit for explicit declarations

Costs:
- more ceremony
- less flexible record/object interop
- weaker anonymous composition

### 7.2. Structural typing

Structural systems say types are compatible because their shapes match.

Examples:
- TypeScript object types
- OCaml object and polymorphic variant traditions
- row-polymorphism-based systems
- many record-oriented research languages

Benefits:
- flexible composition
- ergonomic records and interfaces
- works naturally with open object-like values

Costs:
- more expensive checking
- trickier runtime reflection
- harder compatibility and coherence questions
- more complex inference in rich systems

### 7.3. What fits constructor-heavy runtime-visible systems

Some architectures lean toward explicit type values and constructors like:

- `Class`
- `Interface`
- `Enumeration`

That suggests a likely long-term nominal core, possibly with structural-like views layered on top.

A practical early implementation strategy would be:

- start with **nominal type identity**
- consider structural features later where clearly beneficial
- avoid making the initial runtime depend on structural subtyping or expensive record compatibility

### 7.4. Practical verdict

**Nominal typing is a strong conservative pick for a first real runtime type layer.**

Structural typing may later be added for selected constructs, but should not be the first implementation burden.

---

## 8. Row Polymorphism — The Best Structural Record Technique Worth Studying

### 8.1. What it is

Row polymorphism supports records and variants with extensible fields while keeping type reasoning manageable.

Instead of saying a function takes exactly `{x: U64}`, a row-polymorphic system can express:

- record has field `x`
- may also have other fields

This is the best-known principled way to support flexible record typing without going all the way into unrestricted structural subtyping.

### 8.2. Why it matters

If a language wants exported values and object-like or class-like records to be ergonomic, row polymorphism is one of the most interesting future directions.

It helps with:
- exports
- modules-as-values
- extensible records
- object capability style APIs

### 8.3. Why it is not a first implementation feature

Although elegant, row polymorphism adds:
- type representation complexity
- unification complexity
- error message complexity
- runtime/typechecker interaction complexity

This is not where a language should start if its runtime-first architecture is not built yet.

### 8.4. Practical verdict

**Excellent future research direction, not a first implementation target.**

---

## 9. Occurrence Typing — A Dynamic-First Typed System Worth Mining

### 9.1. What it is

Occurrence typing, associated especially with Typed Racket, refines the type of a value depending on control-flow facts.

For example:
- after checking `x is String`, treat `x` as `String` in that branch

### 9.2. Why it is interesting for runtime-first languages

Such languages may want:
- entered-code-only semantics
- control-flow-driven meaning
- test-driven semantic discovery

Occurrence typing is relevant because it is one of the few type-system families that takes dynamic control flow seriously rather than treating typing as purely prior static truth.

### 9.3. Why it is probably not phase-one material

Occurrence typing usually requires:
- branch-sensitive refinement machinery
- proposition environments
- control-flow-sensitive type narrowing
- richer term predicates

That is very valuable later, especially if Mage gains:
- `is` checks
- tag tests
- variant/object inspection

But it is too much for the first working type system.

### 9.4. Practical verdict

**High-value future feature if Mage embraces dynamic-first typed reasoning, but not an initial target.**

---

## 10. Gradual Typing — Attractive but Dangerous

### 10.1. What it is

Gradual typing combines typed and untyped regions through a dynamic or unknown type, often called:

- `Dynamic`
- `?`
- `Any`

It promises migration between dynamic and static code.

### 10.2. Why people want it

Benefits:
- incremental adoption
- typed and untyped interoperability
- lower migration barrier
- useful for dynamic-first languages evolving static support

### 10.3. Why it is difficult

Gradual typing is not one thing. Different enforcement strategies make very different tradeoffs:

- erasure / optional typing
- transient checks
- natural / proxy-based sound enforcement
- concrete/tag-based enforcement

These differ in:
- soundness
- overhead
- interoperability
- blame behavior
- runtime complexity

### 10.4. Why runtime-first languages should be cautious

Such architectures may already want:
- every variable has a type
- types are values
- runtime-visible semantics matter
- execution-driven diagnostics matter

A naive gradual system can undermine that by introducing too much ambiguity around:
- what type guarantees actually mean
- whether runtime values are checked deeply or shallowly
- where failures appear
- how efficient the VM remains

### 10.5. Possible narrow use

Mage may still want a limited “unknown” or “deferred” type internally during migration or incomplete code handling, especially in tooling.

But that should not be mistaken for a full gradual typing commitment.

### 10.6. Practical verdict

**Do not make full gradual typing a phase-one goal.**
A limited internal unknown type may be useful, but a full gradual semantics would likely distract from the core runtime-first migration.

---

## 11. Refinement Types — Powerful but Expensive

### 11.1. What they are

Refinement types augment ordinary types with logical predicates.

Examples:
- integer greater than zero
- vector of specific length
- sorted list
- non-null pointer

### 11.2. Benefits

They can express:
- strong invariants
- domain-level guarantees
- safer APIs
- rich diagnostics

### 11.3. Costs

They require:
- theorem proving or SMT integration
- predicate checking or simplification
- more complicated error explanations
- more design complexity

### 11.4. Relevance to early implementation

Refinement types are academically attractive, but many languages are not at the point where they should shape the core design.

What is more useful from the refinement world is the lesson that:
- type systems should expose useful invariants incrementally
- tooling and runtime evidence matter
- branch-sensitive information is valuable

### 11.5. Practical verdict

**Far future only. Not appropriate for initial Mage implementation.**

---

## 12. Set-Theoretic, Union, and Intersection Types

### 12.1. What they are

These systems enrich the type language with:
- unions
- intersections
- negation
- richer logical composition

They can model:
- overloaded APIs
- tagged unions
- flexible dynamic interfaces
- precise data-flow properties

### 12.2. Benefits

Very expressive. Particularly useful in dynamic language typing and advanced API typing.

### 12.3. Costs

Subtyping, normalization, and inference become much more complex.
Error explanations also become significantly harder.

### 12.4. Practical verdict

**Interesting long-term if a language wants rich dynamic interface typing.**
Definitely not a first system.

---

## 13. Higher-Rank Polymorphism — Powerful but Annotation-Hungry

### 13.1. What it is

Higher-rank polymorphism allows polymorphic function types to appear inside argument positions and other nested places, not just at top-level let-generalized positions.

### 13.2. Why it matters

It enables:
- generic higher-order APIs
- stronger abstractions
- expressive library design

### 13.3. Why bidirectional typing matters here

Complete global inference for higher-rank types is not practical. The most successful implementations rely on:

- bidirectional typing
- local inference
- explicit annotations where needed

### 13.4. Relevance to constructor-heavy or generic callable systems

If a language eventually wants rich generic procedure values or type constructors, higher-rank polymorphism is worth studying. But only after:

- runtime type values exist
- procedure types are explicit
- local/bidirectional checking exists

### 13.5. Practical verdict

**Future advanced feature; not phase one.**
But if added later, bidirectional typing is the right implementation foundation.

---

## 14. Subtyping — Use Sparingly

### 14.1. What it is

Subtyping introduces compatibility relationships beyond exact type equality.

### 14.2. Why it is attractive

It can model:
- interface compatibility
- inheritance
- richer API flexibility

### 14.3. Why it often hurts practicality

Subtyping tends to make:
- inference harder
- error messages worse
- implementation more brittle
- type-directed execution more subtle

Especially when combined with:
- polymorphism
- overloads
- global inference
- structural typing

### 14.4. Relevance to early implementations

A language should likely avoid broad, implicit subtyping in its first substantial type implementation.

A much safer approach is:
- explicit nominal types
- explicit callable signatures
- explicit conversions or constructors
- possible interface conformance later

### 14.5. Practical verdict

**Avoid broad implicit subtyping early.**
Add only where there is a very clear semantic need.

---

## 15. Constraint-Based Global Solvers — Powerful but a UX Risk

### 15.1. What they are

Many sophisticated type checkers gather constraints globally and solve them after the fact.

### 15.2. Benefits

They can support:
- rich inference
- overload selection
- typeclass-like mechanisms
- flexible constraint propagation

### 15.3. Costs

They often suffer from:
- poor error locality
- performance cliffs
- complexity explosion when features compose
- difficult debugging for compiler implementors

### 15.4. Practical verdict

**Mage should bias against a large global constraint solver early.**
A local, explicit, bidirectional system is more aligned with the architecture.

---

## 16. Runtime Type Representation Strategies

When a language wants types to be values, implementation is not only about static checking. Runtime representation matters too.

---

## 16.1. Erased types

Type info exists only during checking and disappears at runtime.

Benefits:
- cheap runtime
- simpler VM

Costs:
- impossible to treat types fully as runtime values
- weak support for runtime reflection or export/import type identity
- not aligned with runtime-visible type-value architectures

### Verdict
**Not suitable as the sole Mage strategy.**

---

## 16.2. Tagged runtime type identity

Each runtime value carries or can cheaply recover a type tag or type pointer.

Benefits:
- runtime type queries possible
- assignment checks possible
- callable/type value identity is natural
- import/export/runtime sharing easier

Costs:
- some runtime overhead
- value representation pressure
- VM integration required

### Verdict
**Best likely starting point for Mage.**

---

## 16.3. Separate binding type metadata

Instead of every value carrying full rich type info, bindings carry stable type identity, while immediate values use compact tags.

Benefits:
- cheaper than full per-value descriptive metadata
- fits first-assignment typing well
- aligns with “variable has type” rule

Costs:
- some operations still need value-level type tests
- type identity for compound values still needs representation

### Verdict
**Very promising for Mage’s first implementation.**

---

## 17. A Practical Type System Ladder for Mage

A practical design should probably evolve in layers rather than jump to the final system.

---

## 17.1. Layer 1 — Minimal runtime type substrate

Implement:

- builtin `Type` values
- builtin primitive types
- `Void`
- `Source`
- `String`
- `Number`
- procedure type values
- binding stores type after first assignment
- assignment checks type identity
- runtime values carry enough information to recover type identity

This gives Mage a real working type system consistent with the architecture.

---

## 17.2. Layer 2 — Bidirectional body checking

Implement:

- declarations introduce expected types
- expressions synthesize local types
- returns check against declared return shapes
- arguments check against callable parameter types
- body blocks are checked against expected signatures when entered/materialized

This is where the real type checker becomes useful.

---

## 17.3. Layer 3 — Local inference conveniences

Add:

- infer local variable type from initializer where annotation omitted
- infer expected block body type from surrounding context
- infer simple call argument conversions or omitted details if desired

This should remain local and predictable.

---

## 17.4. Layer 4 — Richer nominal constructors

Add:

- `Class`
- `Interface`
- `Enumeration`
- conformance rules
- object/class runtime type identity

Only after runtime-first semantics and executable materialization are already real in the implementation.

---

## 17.5. Layer 5 — Optional advanced features

Possible future additions:
- row polymorphism for records
- occurrence typing
- higher-rank polymorphism
- selected structural views
- limited gradual or unknown type for tooling
- effect or capability typing

---

## 18. Strong Conservative Picks

If the goal is practical implementation with low regret, these are some of the strongest conservative choices for runtime-first languages with explicit declarations.

### 18.1. Bidirectional typing
Best overall foundation for the checker.

### 18.2. Nominal runtime type identity
Best initial representation strategy for type values and executable values.

### 18.3. First-assignment monomorphic variable typing
Directly matches Mage architecture and is cheap to implement.

### 18.4. Local inference, not global inference
Preserves usability without fighting the runtime-first model.

### 18.5. Explicit callable types for procedures
Essential for executable values and body checking.

---

## 19. Marginal but Efficient Picks

These are not maximal systems, but they may be the highest-value early implementations in many language projects.

### 19.1. Binding-fixed monomorphic typing
The simplest real type discipline that still gives meaningful guarantees.

### 19.2. Explicit procedure type plus checked bodies
High value, low conceptual overhead.

### 19.3. Nominal builtin types only
Avoids early complexity while supporting runtime-first semantics.

### 19.4. Unknown or deferred type only for tooling and migration
Can help incomplete code and editor workflows without committing to full gradual typing.

### 19.5. Runtime type values without full type-level computation
Lets types be values before implementing the final constructor system.

---

## 20. Type Systems That Look Attractive but Are Probably Too Early

Avoid making these early project goals:

- full HM with rich polymorphism as the main semantic model
- broad structural typing
- unrestricted subtyping
- full gradual typing semantics
- refinement types
- set-theoretic types
- broad global constraint solving
- higher-rank polymorphism before basic runtime type/value model exists

These may become useful later, but they are too expensive or misaligned for the current migration stage.

---

## 20A. Community Notes and Forum Marginalia Worth Mining

Academic papers are essential, but community discussions often expose the practical fault lines earlier and more bluntly than formal publications. The forum and community material around type systems repeatedly surfaces the same implementation truths:

- global inference becomes a usability problem when feature interactions pile up
- local, annotation-guided systems are usually easier to maintain
- error locality matters almost as much as formal expressiveness
- structural flexibility is attractive, but nominal identity is much cheaper to make predictable
- dynamic-first retrofits often pay for flexibility with runtime or semantic complexity
- many “beautiful” type systems are poor first implementation targets

The notes below are not normative by themselves, but they are useful pressure tests against over-engineering.

### 20A.1. HM vs Bidirectional Is Often the Wrong First Question

A recurring practical observation in blog and forum discussion is that “HM or bidirectional?” is often the wrong framing. The deeper question is usually:

- does the language need polymorphic inference broadly?
- or does it mainly need predictable local checking with occasional inference?

The practical takeaway aligns with Pierce and Turner and with later bidirectional literature:

- if the language wants explicit signatures and predictable checking, bidirectional typing is the better backbone
- if the language later wants polymorphism, unification can still be added inside a bidirectional framework
- therefore bidirectional typing is not the “opposite” of HM so much as a broader implementation discipline

This is especially relevant to Mage because the language already wants explicit procedure signatures and runtime-visible type values.

### 20A.2. Global Constraint Solvers Often Have a Reputation for Bad UX

Community discussion around modern industrial type systems repeatedly warns about large global constraint solvers:

- they can become slow in pathological cases
- they often produce poor or distant error messages
- they make feature composition fragile
- they are hard for compiler authors to debug

This does not mean constraints are bad. It means:
- use them locally where they buy something
- do not make the whole language depend on a giant opaque solve step if better locality is possible

For runtime-first languages, this strongly favors:
- bidirectional checking
- local inference
- explicit procedure signatures
- explicit declaration-time type values

### 20A.3. Row Polymorphism Has a Strong Reputation as the “Least Bad” Structural Record Story

When structural extensibility comes up, experienced implementors frequently point toward row polymorphism as the cleanest principled record technique.

The practical consensus is roughly:

- unrestricted structural subtyping is often messy
- row-polymorphic records are a more disciplined alternative
- they are still more complex than nominal records
- they are usually worth it only when open record APIs are central to the language

This suggests:
- keep row polymorphism on the research horizon
- do not make it part of the first implementation burden
- only consider it once modules/exports/classes/interfaces clearly need flexible structural records

### 20A.4. Gradual Typing Splits the Community for Good Reasons

Discussion around optional and gradual typing repeatedly distinguishes multiple very different systems that are often casually lumped together:

- erasure-style optional typing
- transient runtime checks
- proxy or contract-based “natural” semantics
- concrete/tag-based enforcement

The practical lesson from both papers and forums is:

- “gradual typing” is not one implementation decision
- performance, soundness, and interoperability depend heavily on the enforcement strategy
- many industrial systems deliberately give up strong guarantees for speed and compatibility
- strong guarantees often cost more than language designers first expect

The important conclusion is:
- a limited unknown or deferred type may be useful internally
- a full gradual typing commitment should not happen accidentally
- if Mage ever goes in that direction, it must choose semantics explicitly rather than borrowing the label loosely

### 20A.5. Runtime Type Identity Is Repeatedly Valued for Simplicity

Even outside traditional PL papers, implementation discussions often converge on a simple practical preference:

- if runtime checks matter, stable runtime type identity is extremely valuable

This may be:
- inline type tags
- type pointers
- nominal IDs
- binding-level type metadata plus cheap value tagging

The common reason is not philosophical purity; it is implementation tractability:
- assignment checks are simpler
- casts are simpler
- debugging is simpler
- imported/exported values are easier to reason about
- VM/runtime integration is easier to explain

This strongly reinforces the recommendation to start with:
- nominal runtime type identity
- explicit builtin type values
- binding-fixed variable typing

### 20A.6. Dependent and Proof-Oriented Systems Are Inspiring but Usually Too Early

Community reaction to proof-oriented and dependently typed material is often the same:

- conceptually enlightening
- practically expensive
- easy to underestimate
- best learned from, not copied into an early implementation

The useful lesson is not “implement dependent types”. The useful lesson is:
- types can carry meaningful guarantees
- richer type systems reward explicit structure
- language ergonomics matter as much as expressive power

That supports a staged path:
- first implement simple explicit type guarantees
- later consider richer invariant-carrying systems only if the language truly needs them

### 20A.7. Dynamic-First Typed Systems Are Most Useful When They Capture Real Idioms

Discussion around Typed Lua, Typed Racket, and related systems repeatedly emphasizes that dynamic-first type systems succeed when they model the actual idioms programmers already use.

The lesson is:
- do not design a type system only from theory
- design it to match the language’s real execution and data patterns

That means the type system should be designed around the language’s real execution and data model, such as:
- executable values
- declaration-time binding
- source blocks as values
- file imports as calls
- test-driven semantic discovery

A type system that ignores these and assumes static whole-program certainty will fight the language instead of helping it.

### 20A.8. A Practical Community Bottom Line

The strongest recurring practical recommendations from community and forum material are:

- prefer bidirectional typing over a heroic global inference story
- keep inference local and annotation-guided
- start with nominal runtime identity before experimenting with structural flexibility
- treat gradual typing as a family of tradeoffs, not a single feature
- avoid unrestricted subtyping early
- build the simplest type system that matches the runtime model, then grow it

That is very close to the recommendation already reached in this document.

---

## 21. Implementation Implications

Given a runtime-first or execution-driven architecture, a technically grounded recommendation is:

### 21.1. Do first
- implement minimal runtime type values
- implement binding-fixed variable typing
- represent procedure types explicitly
- make `Void` and `Source` real type values
- build a bidirectional checker skeleton around declarations and bodies

### 21.2. Do next
- connect body checking to executable materialization
- check only entered code deeply
- preserve shallow structural diagnostics for unreached code
- make imported/exported values carry clear type identity

### 21.3. Do later
- richer nominal type constructors
- interface/class semantics
- structural extensions
- advanced inference
- branch-sensitive or occurrence-based refinements

---

## 22. Summary by Strategy

### 22.1. Best practical foundation
- **Bidirectional typing**
- **local inference**
- **nominal runtime type identity**
- **first-assignment variable typing**
- **explicit procedure type checking**

### 22.2. Best “marginal but efficient” first implementation
- builtin type values only
- explicit procedure types
- monomorphic bindings fixed at first assignment
- runtime type checks on assignment and call boundaries
- no full generic or structural machinery yet

### 22.3. Best future advanced directions
- row polymorphism
- occurrence typing
- selected higher-rank polymorphism via bidirectional typing
- richer interface/class runtime semantics

### 22.4. Systems to avoid early
- broad global inference
- unrestricted subtyping
- full gradual typing commitment
- refinement-heavy designs
- highly expressive set-theoretic systems

---

## 23. Bottom Line

If a language wants a type system that matches its architecture rather than fighting it, the likely best path is:

1. **minimal runtime type substrate**
2. **first-assignment monomorphic binding typing**
3. **explicit procedure type representation**
4. **bidirectional checking of entered code**
5. **local inference where cheap and obvious**
6. **richer nominal type constructors later**
7. **advanced type features only after runtime-first execution is stable**

In one sentence:

> The best practical type system is often not a maximal one; it is a runtime-aware, nominal, bidirectional system with simple local inference and strong executable/type value identity.

---

## 24. References

- Hindley, J. R. “The Principal Type-Scheme of an Object in Combinatory Logic.” 1969.
- Damas, L., Milner, R. “Principal type-schemes for functional programs.” 1982.
- Pierce, B. C., Turner, D. N. “Local Type Inference.” TOPLAS 2000.
- Dunfield, J., Krishnaswami, N. R. “Bidirectional Typing.” ACM Computing Surveys 2021.
- Dunfield, J., Krishnaswami, N. R. “Complete and Easy Bidirectional Typechecking for Higher-Rank Polymorphism.” ICFP 2013.
- Peyton Jones, S. et al. “Practical type inference for arbitrary-rank types.” JFP 2007.
- Tobin-Hochstadt, S., Felleisen, M. work on occurrence typing and Typed Scheme / Typed Racket.
- Siek, J. and collaborators on gradual typing design and efficient implementations.
- Kuhlenschmidt, A. et al. “Toward Efficient Gradual Typing for Structural Types via Coercions.” PLDI 2019.
- Greenman, B., Felleisen, M., and others on typed–untyped interactions and comparative gradual typing semantics.
- Leijen, D. on extensible records and row-polymorphic directions.
- Cardelli, L. “Typeful Programming.” 1991 / 1993 circulation and republication lineage.
- Community discussions on Hacker News, Lobsters, and other programming language forums around:
  - algebraic subtyping
  - Typed Lua and optional typing
  - bidirectional typing
  - gradual typing tradeoffs
  - row polymorphism and record systems
  - proof-oriented and dependent type ergonomics

These community discussions are not primary formal sources, but they are useful marginalia for identifying:
- recurring implementation pain points
- error-message and UX concerns
- feature combinations that tend to go pathological
- which “beautiful” systems working implementors consistently warn against adopting too early