[English](coding.md) | [简体中文](coding_CN.md)

<a id="english"></a>

# Multilingual Coding and File Organization Standard

> Version: 1.0.0. Git revisions/file digests identify content. Reuse with the [development standard](development.md); possessing these documents does not establish adoption or qualification.

This standard owns code expression, modules, and files; `development.md` owns admission, authorization, verification, evidence, gates, and release. Keep long-term AI-maintained implementation consistent, direct, and verifiable while respecting each language's semantics/ecosystem.

## 1. Scope and Rule Ownership

Requirement levels and authority follow the development standard. Cover handwritten production, tests, build tools, migrations, templates, and executable examples. Manage generated/third-party files by provenance, without direct stylistic rewriting.

| Source | Sole responsibility |
| --- | --- |
| Development and maintenance standard | Workflow, design admission, contract verification, evidence, and gates |
| General sections and Section 8 of this document | Cross-language principles, language-specific conventions, modules, file organization, and enforcement |
| Committed tool configuration | Encoding these formatting and diagnostic requirements as tool parameters, without a duplicate prose standard |
| Project design and configuration | Actual components, dependency boundaries, directory mappings, product interfaces, tool entry points and parameters, and explicitly registered coding differences |
| Project authorization rules | Which actions are authorized and which require a user decision |

Section 8 owns language rules; Section 1.1 registers confirmed project differences. Project configuration owns versions, tool parameters, and compatibility. Resolve missing public-interface/compatibility conventions first. Private details follow effective rules and nearby code, which cannot itself authorize overrides.

Maintain one source per fact. AI entries link rules/configuration rather than duplicate them or collect session copies. On reuse, update companion links; keep project paths/commands in project documentation.

<a id="overrides"></a>

### 1.1 Project Overrides

Register project differences in a fixed section of an existing guide (such as `CONTRIBUTING.md`), linked from `AGENTS.md` and required at task start. Add a rules file only without a suitable home; never fork this baseline. Subdirectory rules must be discoverable from the entry, not hidden or introduced by transient prompts.

Precedence: current explicit user instruction > confirmed path-applicable project difference > language rule > general rule. Subdirectory differences identify replaced parent provisions/scopes. Proximity, recency, or stronger wording grants no precedence. Resolve unspecified overlaps explicitly, never by choosing the weaker rule.

Record at least the following; unaffected provisions continue to inherit the baseline:

| Field | Requirement |
| --- | --- |
| Provision | Standard version and section/anchor; identify the requirement being replaced |
| Scope | Repository-relative paths, language, and production/test/tool file categories |
| Replacement | A complete executable rule, not merely “follow project conventions” |
| Reason and authority | Compatibility, ecosystem, or product reason, plus user confirmation or an effective existing project agreement |
| Enforcement | Formatter/check configuration; explicitly state when automatic enforcement is unavailable |

For example, a maintenance guide may register a confirmed formatting difference:

```markdown
## Coding Differences

- Provision: coding.md 1.0.0, Section 8, default line width for languages other than C++.
- Scope: handwritten Python files under tools/**/*.py, including tests.
- Replacement: Black line-length is 88 instead of the general 160-column target.
- Reason and authority: the project has confirmed its existing Python tool configuration; link the actual confirmation when registering this difference.
- Enforcement: [tool.black] in pyproject.toml.
```

The example grants no 88-column override. Tools implement confirmed rules, not authorize them; reconcile prose/tool conflicts. Project differences may change naming, comment language/density, formatting, or paths, but cannot weaken correctness, compatibility, evidence, or acceptance, or expand test/download/commit/release authority. Use the governing decision/exception process for such changes.

Reuse legal configuration fields/entries; add no unsupported `development.json` fields. On standard upgrades, recheck each override against its intended provision instead of silently applying it to changed rules.

## 2. Naming and Scope

- Across all languages, prefer one precise complete word for types, functions, variables, fields, modules, and owned files; one concept keeps one name. No truncated words, invented initialisms, or vague short names.
- Use only established abbreviations (ID, UUID, TCP, TLS, RPC, TTL), with language-appropriate capitalization. Local preferences are not consensus; comments cannot repair ambiguous or incorrect names.
- Let modules, types, and lexical scope supply context instead of repeating their names. Do not add empty classes or nesting merely to shorten identifiers.
- Choose the precise word first, then language/symbol capitalization. Use compounds only when one word plus existing scope cannot distinguish necessary meanings or units; join them by language convention. Compound syntax alone is no reason for multiword names.
- Preserve language/framework/external prefixes, suffixes, interfaces, and serialized names. Internal style or personal preference cannot justify changing them, overturning established names, or unrelated bulk renames.
- Name actions, queries, and conversions by observable behavior. Reads performing I/O, mutation, iterator consumption, or ownership transfer must declare those effects in the contract.
- Distinguish easily confused units, identities, counts, offsets, and clock domains in types or names. Do not use comments to excuse avoidable confusion between same-typed arguments.
- Place definitions in the narrowest valid scope that owns their responsibility, private by default. Expose types and interfaces only for real usage contracts, never merely to simplify tests.
- `utils`, `common`, and `manager` cannot substitute for responsibilities. Keep established names with clear responsibilities; assign owners rather than dumping unrelated code into shared directories.

## 3. Implementation and Complexity

### 3.1 Functions, State, and Data

Organize functions around one operation/invariant, with clear validation, preparation, commit, notification, and cleanup. Keep variables near use/lifetime. Early returns/helpers are allowed; arbitrary line limits cannot justify scattering a commit boundary across jumps.

Express constraints with types, encapsulation, explicit results, and native resources. Distinguish absence, emptiness, zero, error, and unknown outcomes; avoid shared sentinels/success defaults. Prefer constrained state representations over flags admitting invalid combinations.

Make shared mutation, borrowing, implicit conversions, and lazy effects visible. Do not depend on unspecified traversal/evaluation order or environment defaults. Development Sections 3–4 own concurrency, atomicity, time, numeric, and error contracts.

### 3.2 Abstraction and Efficiency

- Trace needs/affected flow before adding code. Check existing project capabilities, cross-platform standard libraries, native platforms, then approved dependencies. Implement remaining gaps with the simplest contract/resource/performance-compliant option; availability proves neither suitability nor admission.
- Prefer language and standard-library facilities supported by the actual toolchain. Choose syntax for expression, correctness, and cost, not its age.
- Prefer cross-platform standard libraries when semantics, correctness, resources, and performance are preserved. Otherwise isolate necessary platform APIs/alternatives behind explicit adapters. Standard-library membership alone proves no equivalence.
- Abstractions serve current invariants, boundaries, or consumers. Testability, platform, and security interfaces are valid; speculative frameworks, plugins, and inheritance are not.
- Merge similar code only when semantics, failure guarantees, lifecycle, and evolution agree. Visual similarity cannot unite distinct state machines; identical business rules need reasons to remain duplicated.
- Simplify repeated knowledge/understanding cost, preserving necessary checks, comments, error context, and tests. Dense expressions, nested ternaries, hidden effects, and excessive metaprogramming are not simplification.
- For a sufficient simple implementation with known ceilings, document assumptions, limits, and reevaluation triggers (load, measured contention, platform changes) in existing design and necessary English comments. Do not prebuild replacements or parallel debt records. Unmet current contracts are not future upgrades; simplification retains verification duties.
- Always consider complexity, allocation, copying, locality, and locks. Added optimization complexity needs evidence; distinguish static inference from measurements, never equating fewer lines/objects or asymptotic bounds with measured gains.

## 4. Module and Dependency Boundaries

Divide modules by responsibility, data ownership, and reasons to change. Keep public interfaces small/stable and internals restricted by language/build boundaries. Controlled internal tests must not turn internal symbols into maintained APIs.

Separate core rules/I/O under development Section 3.2: explicit data/events/boundaries in the core, concrete assembly at startup. Do not require an interface per type or hide dependencies in containers/global service locators.

Register dependency directions/public entries; no private-path, reflection, or test bypass. Remove accidental cycles; declare inseparable mutual dependencies as jointly maintained units. Directory renaming alone establishes no layering.

Public protocols/cross-language structures share one semantic source for fields, units, errors, and compatibility. Bindings use native expression; internal shapes need not match. Assign owners to generators, shared vectors, and handwritten adapters.

Contain third-party APIs, errors, lifetimes, and versions at useful boundaries, not mechanical wrappers. When the product publicly exposes an external protocol, declare its dependency and compatibility responsibility.

## 5. Files and Paths

Keep files cohesive and private helpers near callers. Split for visibility, ownership, dependencies, or independent changes, not line limits; avoid unrelated features in one file. Respect language-required type/file, module-entry, and export structures.

Use stable responsibility names, not `new`, `final`, `v2-final`, dates, or authors. Preserve semantically meaningful protocol versions, migration numbers, and historical compatibility fixture names.

Paths/configuration distinguish public entries, internals, test support, and generated output without one universal directory tree. Idiomatic colocated unit tests and centralized system/cross-component tests are both allowed and must be discovered.

New owned paths must work on target platforms: avoid case-only distinctions, reserved names, and needless special characters; verify Git tracking on case-sensitive/insensitive renames. Preserve public paths unless needed and retain external resources' meaningful names.

Version encoding, line endings, and executable permissions; new text defaults to UTF-8. Resolve tool paths from explicit cwd or tool location, never personal directories, implicit search paths, or accidental cwd. Portable source cannot require local absolute paths.

## 6. Repository Contents and Directory Responsibilities

Names below illustrate responsibilities. Create only needed directories under language/build conventions; adoption requires neither empty trees nor automatic repository migration.

| Content | Ownership and boundary |
| --- | --- |
| Production code | Its component and native source layout, such as `src`, `internal`, or `lib`; idiomatic root packages are allowed, with clear entry-point responsibilities |
| Public API | Explicit exports, headers, or packages; distinguish internals from public commitments |
| Tests and support | Beside components or under `tests`; test dependencies must not enter production delivery |
| Fixtures, vectors, counterexamples | Owned by tests or shared contracts; declare provenance and discovery, version protected counterexamples |
| Performance and long-running scenarios | Separate entry points, budgets, and authorization within the test system, optionally under `bench`; ordinary tests must not trigger them accidentally |
| Build, generation, maintenance tools | `tools` or ecosystem-defined locations, with stable inputs, outputs, entry points, and failure semantics; temporary scripts must not become hidden dependencies |
| Protocols, schemas, migrations | One source location per interface/component; preserve migration sequences and compatibility samples by semantics |
| Generated code | Mark its input/generator relationship; tracking depends on consumption and build needs, not a blanket exclusion |
| Examples | Consume real public APIs; label complete examples, snippets, pseudocode, and expected failures; discover runnable examples in verification |
| Configuration and deployment | By environment and purpose; commit shareable configuration and safe templates, reference secrets externally |
| Documentation | One entry point each for navigation, standards, design, usage, and latest validation; version control holds history |
| Third-party source and licenses | Separate or register provenance; retain required licenses and NOTICE files regardless of the root license |
| Build output, caches, temporary data | Declared reproducible output or authorized caches; excluded from source commits by default; release only explicitly listed, evidence-bound artifacts, never an entire output directory |
| Raw execution evidence | Seal in declared local/controlled storage under the development standard; retain failures, and do not treat evidence as disposable merely because it is under build output |

Organize components by independent build, delivery, and ownership boundaries. Root tools coordinate; components declare inputs/dependencies. Independently distributed components list every build/runtime input, without undeclared parent-repository dependencies.

Production builds cannot rely on incidental personal-cache files. Manage required tools, generated code, fixtures, and scripts as maintained, verified inputs with provenance/dependencies.

## 7. Comments, Formatting, and Readability

Comments explain current responsibility, contracts, reasons, and hidden constraints. Public APIs cover arguments/results/errors/ownership/concurrency; implementations cover important state/commit/cleanup stages. Explain applicable units, ranges, defaults/initial state, absence, and special values without invented guarantees.

Write handwritten-code comments in standard English, preserving identifiers, protocol fields, and citations; product UI, test inputs, and Markdown need not be English. Document file/type/function/variable purposes, including internals, tests, and templates. Adapt syntax/placement/detail by language, without ratios or statement narration. Register language/coverage differences under 1.1.

| Object | Baseline |
| --- | --- |
| File | Brief native header/module docstring for handwritten source, tests, and scripts: role and, for tests, verified subject. No filename repetition, authors, dates, or change logs |
| Class and other types | Responsibility, represented concept, invariants, and applicable ownership/thread constraints for classes, structs, interfaces, and major types; not a rephrased name |
| Enums and variants | Enum state/category plus each item's meaning/conditions. Named union variants, fields, and state constants need individual semantics too |
| Functions and methods | At declaration/definition, purpose and applicable arguments/results/failures/effects, including private functions. Keep one full contract; accessors/implementations may explicitly inherit property/interface docs but must add new semantics |
| Variables and constants | Purpose, units, ranges, and special values near members/globals/constants/configuration. Function docs may cover parameters. Locals require purpose; related simple temporaries, loop variables, structured bindings, and captures may share adjacent block comments |

Place headers according to shebang/encoding/build/license/module ordering and merge existing module docs. Use schemas/owning docs for commentless formats; preserve generated/third-party provenance. Adjacent/grouped comments must identify objects/purposes and retain lifecycle, locks, units, and special states; vague “process data” is insufficient.

| Language/format | Comment form and detail |
| --- | --- |
| C / C++ | Consistent declaration documentation, using Doxygen when adopted; adjacent `//` may describe private declarations/variables, while implementation explains ownership, locking, and commit boundaries |
| Go | Adjacent exported-declaration docs begin with the symbol name; maintain package docs centrally and describe file roles/internals in adjacent comments |
| Rust | `///` for public items, `//!` for modules; ordinary or doc comments for private items; explain `unsafe` preconditions |
| Python | Module/class/function docstrings with consistent parameter style; `#` for variables and implementation, without repeating type hints |
| JavaScript / TypeScript | Project JSDoc/TSDoc for public APIs; avoid duplicating readable TypeScript types; explain effects, async work, and lifecycle internally |
| HTML / CSS / templates | Native syntax describing accessibility, compatibility, and complex styling constraints; script blocks follow their language |
| Java / Kotlin / C# | Javadoc, KDoc, or XML documentation for public APIs; ordinary comments for internals/variables, simple accessors may inherit property docs |
| Lua | `--` or existing project documentation format; public modules describe arguments/results/host resource duties, internals according to complexity |
| SQL / Shell / PowerShell | Migration effects, transaction/retry boundaries, preconditions, and external effects; suitable help comments for public commands, not line-by-line command narration |
| Protocol / configuration / template | Supported comments or schema descriptions for units, defaults, absence, and compatibility; never insert comments into formats that forbid them |

Use existing documentation tags/layout tooling; add no generator for uniform appearance. Keep full contracts at authoritative declarations and reasons/invariants in implementation.

Comment each enum item adjacently in native syntax, even with obvious names; explain applicable protocol values, indices, flags, defaults, unknowns, and reservations. Preserve values/order. Comments must match current implementation/contracts; durable TODOs need gaps/owners. No commented-out history, session narrative, or author signatures. Language/density changes retain valid contracts without unrelated bulk translation.

Committed configuration/existing formatters own indentation, braces, wrapping, imports, and width; do not duplicate settings. Format only authorized handwritten scope. Explain configuration changes separately; never disable rules for an inconvenient line.

Unavailable tools leave an unchecked scope, not automatic installation or a claim of equivalent visual inspection. Formatting proves no behavior. Static checks still require authorization for compilation, execution, or downloads they trigger.

## 8. Language-Specific Conventions

Keep language conventions here; projects register languages, versions, tools, paths, and Section 1.1 differences. Read general rules and every applicable language section together, including mixed stacks. Defaults grant no API change, toolchain upgrade, or dependency rewrite.

Section 2's one-complete-word preference applies everywhere. `camelCase`, `PascalCase`, `snake_case`, and `UPPER_SNAKE_CASE` govern case and necessary compounds, not word count: `entry`/`Entry`/`ENTRY`, or `readTimeout`/`ReadTimeout`/`read_timeout` only to distinguish read/write deadlines.

| Language/format | Section |
| --- | --- |
| C++ / C | [C++](#cpp) / [C](#c) |
| Go / Rust / Python | [Go](#go) / [Rust](#rust) / [Python](#python) |
| JavaScript / TypeScript / frontend | [JS/TS](#typescript) / [HTML, CSS, components](#web) |
| Java / Kotlin / C# / Lua | [Java/Kotlin](#jvm) / [C#](#csharp) / [Lua](#lua) |
| SQL / Shell / PowerShell | [SQL](#sql) / [Scripts](#shell) |
| Protocol / configuration / templates | [Protocols and configuration](#protocol) |

Unlisted languages inherit general rules; first adoption adds naming, visibility, modules, errors, resources, and formatting here, with tools/support registered at project level. Outside C++, 160 columns is a configurable target subject to existing tools and literal/generation/external-format semantics. C++ follows its own long-line rules.

<a id="cpp"></a>

### 8.1 C++

These provisions define C++ scope, implementation, comments, long lines, and logical-block spacing. They include handwritten tests and templates. Projects still declare the actual language standard and compatibility floor; listing a feature here does not authorize an upgrade.

#### 8.1.1 Scope

- All handwritten C++ must comply, including production, tests, templates, headers, and implementation files.
- Generated and third-party files are outside stylistic cleanup. Do not rename, comment, or reformat them to apply these rules.
- Do not mechanically apply C++ conventions to other languages or resume frozen component development on this basis.

#### 8.1.2 Naming

- Minimal names, established abbreviations, and conditions for compound names follow Section 2; they are not C++-only rules.
- Use meaningful class scope, such as `Store::Entry`, without repeating ownership or creating empty wrapper classes. Types use `PascalCase`; functions and variables begin lowercase. When multiple words are unavoidable, use `EntryState` and `readTimeout`, not underscore-joined identifiers.
- Private data members have a trailing underscore, such as `timeout_`; parameters and locals do not, such as `timeout`. Do not add meaningless prefixes merely to distinguish a member from a parameter.
- Property accessors use same-name overloads, such as `timeout()` and `timeout(value)`. Provide only required read/write operations, not automatic setters for every field. Name non-property operations by behavior.
- Enum items use `PascalCase`, such as `State::Ready`, without repeating the enum name. Unavoidable compounds use upper-initial camel case. Preserve fixed external protocol/generated symbols.
- Owned constants follow variable naming, such as `limit` or necessary `readLimit`, without `k` prefixes or uppercase solely because they are constant. Do not rewrite macros or fixed external symbols on this basis.
- Prefer complete lowercase namespace words and meaningful nesting, avoiding compounds. If responsibility cannot be split and multiple words are necessary, use lower-initial camel case. Do not manufacture empty nesting for short names.
- Owned headers and implementations use `.hpp` and `.cpp`. Filenames use complete lowercase words; unavoidable compounds use underscores, such as `read_timeout.hpp`. Paths and symbols have separate conventions: an uppercase type name does not require an uppercase filename.

#### 8.1.3 Narrow Scope

- Place definitions in the narrowest scope matching their responsibility. Prefer class-local types, enums, constants, and helpers when only that class uses them.
- Class definitions are private by default and become public only for an actual external contract. Nesting does not imply public visibility; do not expand production APIs for tests.
- Order class access sections `public`, `protected`, `private`, omitting empty sections. Within each, group related types, operations, and data by responsibility instead of mechanically separating symbol categories. Declaration order must not alter appropriate visibility.
- Use `struct` for public data records and `class` for encapsulated invariants, resources, and behavior. Do not decide solely by member count or presence of methods; records may have semantically related helpers.
- For a clearly class-owned operation that needs no instance, prefer a static member over a public namespace function.
- Do not create empty classes merely to collect miscellaneous helpers. Shared concepts across independent components, and interfaces that must be namespace-level, retain their appropriate ownership.
- Prefer anonymous namespaces for translation-unit-private implementation details in `.cpp`; this restricts linkage and visibility, not necessarily symbol-table contents.
- Do not mechanically use anonymous namespaces in headers and create distinct types/state in every translation unit. Prefer private class definitions and arrangements appropriate to header semantics.
- Where supported, use compact nested namespaces such as `namespace astra::detail {}`. Compact syntax does not change ownership or scope.

#### 8.1.4 Performance, Caches, and Layout

- Correctness, lifecycle, and concurrency safety come first. Do not sacrifice invariants, object validity, or synchronization for speed or brevity.
- Aim for very high efficiency: consider hot-path allocation, copying, traversal, indirection, locks, and working sets. Simplification must not introduce hidden copies, repeated scans, or extra allocations.
- Arrange data, hot/cold fields, and ownership for actual access patterns and locality. Consider batches and concurrent access, not just one object's size.
- Do not optimize layout solely for minimum `sizeof`. Alignment, padding, cache-line sharing, and false sharing matter; saving bytes must not introduce unsuitable unaligned access or contention.
- Added performance complexity requires an identified hotspot and benefit rationale. Distinguish static inference from measured results; theoretical gains are not measurements.
- Performance verification still requires current-turn authorization. Benchmarks and optimization work do not bypass project permission rules.

#### 8.1.5 Modern Syntax and Simplicity

- Prefer C++26, C++23, and C++20 features and standard-library capabilities actually supported by the target toolchain.
- Prefer modern syntax when it makes implementation more efficient or simpler without compromising correctness or performance.
- Express ownership, constraints, resource management, and compile-time work directly through the language and library. Avoid duplicate mechanisms and layers with no benefit.
- New syntax is not an achievement by itself. Do not conceal simple logic behind complex templates, macros, or abstractions.
- With equal performance and code size, prefer direct, natural, consistent expression in which important invariants and lifetimes are visible.
- Simplification removes real redundancy and complexity, not necessary validation, comments, or tests.
- Use `auto` when initialization clearly expresses the type, or with iterators/complex templates. Write the type when it carries important meaning not evident from initialization. Apply references and `const` semantically; deduction must not hide copies or borrowing.
- Locals that are not reassigned default to `const`, except when mutation or moving is needed. `const` does not prove deep immutability or thread safety, and must not introduce copies for uniformity.
- Put `const` before the qualified type, such as `const Entry&` and `const auto`. A non-reassignable pointer remains `Entry* const`; preserve which object is qualified.
- Prefer `=` for ordinary value initialization and `{}` for aggregates/zero initialization. Choose `()` or `{}` for construction by semantics. Respect overload selection, narrowing, and `auto` deduction rather than replacing syntax mechanically.
- Ordinary functions use leading return types, such as `Result read()`. Use trailing return types, such as `auto read(...) -> Result`, when dependencies on parameters or similar reasons make them appropriate; do not convert every function for style.
- Prefer explicit lambda captures so borrowing, copying, and ownership transfer remain visible. Captured lifetimes must cover execution, especially escape/async cases; explicit capture alone proves no safety.
- Constructors that could convert implicitly default to `explicit`. Permit implicit conversion only with a clear, justified semantic contract, not merely to save a construction expression.

#### 8.1.6 Comments and Information

- Follow Section 7's English comments, object coverage, and C++ documentation form. Headers describe file responsibility; classes, functions, and variables have corresponding explanations. Keep public contracts at declarations and reasons/invariants at implementations without duplicating full documentation.
- Use preceding `///` uniformly for type, function, and member declarations, including enum items and private declarations. Use `//` for local variables and implementation logic; do not alternate declaration block-comment styles by visibility.
- Document units, legal ranges, results, failures, ownership, lifetime, and threading constraints not apparent from signatures. Explain non-obvious semantics of special members, template parameters, and constraints.
- Distinguish defaults from initial values, and do not invent inapplicable ranges or guarantees. Simple locals, loop variables, structured bindings, and captures may share adjacent block descriptions, provided purpose and required borrowing/lifetime/special-value constraints are clear.
- Internal comments focus on resource handoff, lock boundaries, commit order, rollback, and cleanup that statements do not make obvious. Do not mechanically comment every ordinary control-flow stage. Describe enums and each item separately under Section 7.
- Comments may wrap; the single-line expression rule does not apply to them. Preserve valid contract information; reducing duplication is not permission to remove guarantees.

#### 8.1.7 Long Lines and Wrapping

- Indent with four spaces. Attach pointer/reference symbols to the type, as in `Entry* entry` and `Entry& entry`. Formatting must not obscure pointer versus pointee `const`.
- Keep braces around `if`, `else`, `for`, `while`, and `do` bodies, including single statements.
- Long lines are allowed; do not force wrapping to a fixed width.
- Keep function declaration/definition headers on one line, including return type, name, arguments, and qualifiers. Put the opening definition brace on that line.
- Do not wrap ordinary statements, calls, argument lists, or expressions solely for width. Function/lambda/class bodies retain normal structural lines; do not compress entire functions into one line.
- The first `enum`/`enum class` item starts on the line after `{`, and `}` has its own line, even for a one-item enum.
- Keep chained calls on one line unless exceptionally long chains obscure their stages; then wrap at chain boundaries. Exceeding an old width limit alone is not a reason.
- Complex or deeply nested conditions may wrap by logical groups while preserving parentheses, precedence, and evaluation. Simple conditions stay on one line.
- Do not measure complexity solely by line count or character count, or compress excessively to avoid readability requirements.

#### 8.1.8 Logical Blocks and Blank Lines

- Logical blocks are stages such as input validation, preparation, execution, branching, commit, notification, and cleanup. They may span several lines and are not counted merely by braces.
- Keep declarations, comments, and statements belonging to one stage together. A variable or statement on its own line is not automatically a separate block.
- Separate logical blocks with a blank line, normally one.
- If a function contains more than one logical block, its first line after the opening brace must be blank, before the first stage's comments or statements.
- A very simple single-block function needs no initial blank line. Short length does not exempt a multi-stage function.
- Divide branches and internal stages by actual logic, not mechanically at every brace pair. Keep comments beside their block; place the blank separator before its comment.

#### 8.1.9 Headers and Accessor Results

- Headers default to `#pragma once`; use macro guards when unsupported and register the compatibility difference. Do not use both for the same purpose.
- A `.cpp` includes its own header first when present, then project, third-party, and standard headers in separate groups, sorting within groups by path. Headers directly include their declaration dependencies, rather than relying on caller order or incidental transitive includes. Respect external headers with real ordering requirements.
- Forward declarations reduce unnecessary dependencies only when robust and no complete type is required. Include definitions when needed; never invent declarations for standard-library or third-party types. Prefer implementation-only includes in `.cpp`.
- Organize templates and caller-visible `constexpr` definitions as the language requires. Otherwise simple getters, setters, and single-step forwarding may be inline in `.hpp`; complex validation, synchronization, and state transitions belong in `.cpp` even if short.
- Non-template definitions in headers must satisfy the one-definition rule, using appropriate inline semantics. A header definition does not guarantee compiler inlining or a speed improvement.
- Return scalars and small values by value by default. Larger members may use const references/views to avoid unnecessary copies. State borrow validity and invalidation conditions; read-only access does not imply immutable storage or thread safety.
- References/views must satisfy caller lifetime and synchronization contracts. Never return references to temporaries or expose data valid only under an internal lock as safe after unlocking. Use independent values or controlled snapshots when a valid borrow cannot be guaranteed.

#### 8.1.10 Errors and Return-Value Checks

- Where supported, prefer `std::expected` for recoverable failures, with stable error categories and context. Do not implicitly upgrade tools or add a dependency for it; compatibility targets retain their established result representation.
- Exceptions cover paths that cannot return normal results and third-party boundaries. Specify what may propagate, where conversion/termination occurs, and failure-state guarantees. `std::expected` does not imply non-throwing behavior or justify `noexcept`. Do not disguise broken internal invariants by catching everything and returning a business error.
- Mark error results, resource acquisition, and other results whose omission risks misuse `[[nodiscard]]`, at an appropriate type or function. Decide ordinary queries by actual misuse risk rather than annotating every non-`void` operation mechanically.
- When ignoring a result is justified, express it explicitly and retain the reason. Blanket discards, meaningless casts, and diagnostic suppression must not bypass error handling or resource duties.

<a id="c"></a>

### 8.2 C

- Functions, variables, and owned files default to `snake_case`; macros use `UPPER_SNAKE_CASE`. Public symbols have necessary module prefixes. Do not invent reserved identifiers or copy C++ class-local short names into global C scope.
- Owned type names use lowercase words, joining unavoidable compounds with underscores. Retain explicit structure tags and use forms such as `struct entry*`; do not default to `typedef` that hides tags or pointer ownership.
- Public headers contain only caller-required declarations; opaque internal structures are allowed. Restrict private functions/state to file-local linkage. Fix guards, includes, and C/C++ interoperability according to supported use.
- Headers default to `#pragma once`; use macro guards on unsupported tools and register the compatibility difference.
- Document pointers, lengths, capacities, encoding, and ownership together: allocation, release, and output-parameter state on failure. Avoid unbounded string operations. Casts do not prove alignment, aliasing, or representation validity.
- Separate errors from valid results. Pair acquisition with release on every exit, including partial initialization. A clear shared cleanup exit is acceptable.
- Verify signedness and bounds in bit operations, integer conversions, and buffer arithmetic. `volatile` is not synchronization. Macros must avoid repeated argument evaluation and hidden control flow; prefer functions/constants when suitable.
- Use the project's existing formatter. C++ single-line function-header and initial-blank-line conventions do not automatically apply to C.

<a id="go"></a>

### 8.3 Go

- Package names are short, lowercase responsibility names; exported identifiers use `PascalCase`, others `camelCase`. Preserve conventional initialisms such as `ID`, `URL`, and `HTTP`: `userID`, not `userId`. Do not repeat context with `Get`, package prefixes, or empty receiver types.
- Receivers use accurate complete words, such as `(store *Store)`, consistently for the type; do not replace responsibility names with single-letter abbreviations.
- Filenames prefer lowercase words; concatenate unavoidable compounds, such as `readtimeout.go`. Preserve Go-required platform, build, and test suffixes, including their underscores.
- Use `New` when the package's primary type needs a constructor, and names such as `NewEntry` for other types where needed. Do not force constructors onto usable zero values merely for naming consistency.
- Format with `gofmt`, without manual column alignment overriding it. Organize files by package responsibility and tests by language discovery conventions; make build constraints and platform ownership explicit.
- Group imports as standard library, third party, and project packages, separated by blank lines and sorted within groups by existing tools. Do not install tools just to reorder imports.
- Prefer external test packages for public behavior and same-package tests for internal invariants. Do not expose production internals to accommodate external tests or use package selection to narrow verification.
- Keep interfaces minimal and consumer-oriented, rather than mirroring every concrete type. Choose value/pointer receivers by copying cost, mutability, and semantics; do not copy lock-bearing or otherwise noncopyable state casually.
- Express recoverable failures with `error`; preserve identifiable causes when adding context. Do not classify by error-string comparisons or hide external-input/ordinary failures with panic/recover.
- Explicitly accept and propagate Context for cancellable operations; do not replace request context with an unrelated background context. Assign goroutine, channel, timer, and connection shutdown/release duties. Extra goroutines must not conceal blocking or errors.
- Document mutability and sharing of exposed slices, maps, pointers, and interface values. Returned collections are not automatically independent copies. An interface containing a typed nil pointer is not necessarily a nil interface.
- Data packages must not initiate networking, spawn tasks, or alter process environment during initialization. Document exported behavior and failure constraints, and complex internal stages as required by Section 7.

<a id="rust"></a>

### 8.4 Rust

- Modules, functions, variables, and files default to `snake_case`; types/traits use `PascalCase`; constants/statics use `UPPER_SNAKE_CASE`. Use `rustfmt` and the project's configured diagnostic entry points.
- Prefer `entry.rs` without submodules and `entry/mod.rs` as the parent entry with submodules. Do not declare conflicting forms together.
- Ordinary constructors may consistently use `new`; `Self` versus `Result<Self, Error>` expresses fallibility. A `try_` prefix is not mandatory.
- Simple getters use property names such as `timeout()`. Name mutations for their behavior, without a universal setter pattern or automatic mutators for every field.
- Group imports as standard library, third party, and current crate, separated by blank lines and sorted with existing tools.
- Default to private, widening to local/crate visibility only for actual use. Traits, generics, and macros serve real constraints/reuse; do not hide ordinary control flow or build frameworks with one forwarding implementation.
- Use `Result` for recoverable errors and `Option` for absence. Do not use `unwrap/expect` on external inputs or ordinary I/O failure. Internal invariant failures follow the development standard's isolation rules.
- Libraries/domains use explicit error types; application edges may aggregate errors while preserving causes. This choice does not authorize an error-handling dependency.
- Prefer borrowing and explicit ownership. Unjustified cloning, leaks, and indefinite sharing must not silence compiler errors. `Arc` expresses shared ownership, not synchronization.
- Confine `unsafe` to a minimal boundary and explain caller/implementation safety obligations. Raw pointers, FFI, aliasing, and release duties require concrete constraints, not merely “for performance.”
- Explain async waits while locked, future cancellation, and destruction duties. Dropping a task handle does not prove draining; destruction must not omit required asynchronous external cleanup.

<a id="python"></a>

### 8.5 Python

- Modules, functions, and variables use `snake_case`; classes use `PascalCase`; constants use `UPPER_SNAKE_CASE`. Organize imports by package boundaries; do not hide dependencies with wildcard imports or global search-path changes.
- Enum members and module constants use uppercase words, such as `READY` and `LIMIT`, joining unavoidable compounds with underscores (`READ_TIMEOUT`). Enum types remain `PascalCase`.
- Use Google-style docstrings with `Args`, `Returns`, and `Raises` only as needed. Do not pad simple descriptions with empty sections, repeat type annotations, or invent error guarantees.
- Default to absolute imports within packages; use explicit relative imports only for a small number of justified cases. Paths must match actual package boundaries.
- New file-path handling defaults to `pathlib.Path`, converting at external boundaries as needed. Do not rewrite unrelated existing code merely to adopt it.
- Prefer `dataclass` for simple data records; choose immutability by semantics. Types encapsulating complex behavior/invariants follow their responsibilities; do not convert all classes mechanically.
- Prefer keyword-only configuration, boolean switches, and easily confused same-type arguments. Preserve published positional-call compatibility.
- Use the existing Black configuration; format immediately after edits and check syntax statically. Annotate functions/public data boundaries, but validate dynamic external input at runtime: annotations are not execution guards.
- Do not use mutable defaults for cross-call state. Distinguish `None`, empty containers, and false values. Specify serialization, path, and subprocess input types/boundaries rather than removing ambiguity through string conversion.
- Release resources with context managers or explicit `try/finally`. Catch only exceptions that can be handled or translated, preserving causes. Bare `except`, default success, or swallowed cancellation must not hide failure.
- Assign ownership to async tasks, threads, and subprocesses; await completion/cleanup after cancellation. Background tasks and synchronous blocking must not bypass lifecycle constraints.
- Imports must not run maintenance, connect to networks, or execute tests. Separate executable entry points from reusable computation; preserve shell-argument/protocol-byte semantics and propagate failures, skips, and cleanup errors.

<a id="typescript"></a>

### 8.6 JavaScript / TypeScript

- Functions, variables, and fields use `camelCase`; types/classes use `PascalCase`. Do not add meaningless interface prefixes. Constants also use `camelCase`, including fixed module values.
- Ordinary module filenames use lowercase words, with `camelCase` for unavoidable compounds, such as `readTimeout.ts`. Component files follow component naming.
- Prefer `interface` for TypeScript object contracts and `type` for unions, mapped/composed types. Do not rewrite external types for uniformity or hide interface changes through declaration merging.
- Prefer `function` for named module operations and arrow functions for callbacks. All named TypeScript functions, including methods and named arrow bindings, explicitly declare return types; anonymous callbacks may infer them. Plain JavaScript uses its documentation/checking mechanisms, not TypeScript syntax.
- Default to `const`, using `let` for reassignment. Avoid implicit conversion/global state; distinguish `null`, `undefined`, empty strings, and zero rather than treating all absence through truthiness.
- Owned optional values default to `undefined`; reserve `null` for explicit business states or external protocols. Internal preferences must not alter protocol null semantics.
- Prefer `enum` for finite TypeScript states, documenting every member under Section 7. Projects requiring erasable-only syntax or other runtime restrictions register a Section 1.1 compatibility difference; do not silently disable compiler checks or change execution modes.
- Class-private fields default to TypeScript `private field`, not `#field`. Type-level access control is not runtime isolation. Plain JavaScript uses its actual encapsulation mechanisms, not TypeScript-only modifiers.
- Prefer named exports for owned modules, preserving framework-required defaults. Distinguish type/runtime imports; fix resolution, extensions, and browser/server targets in project configuration.
- Enable applicable strict checks, including null, index, and optional-field boundaries. Do not hide uncertainty through `any`, non-null assertions, casts, or ignore comments; validate unknown external data before converting it to domain types.
- Callers or explicit task owners observe Promise failures. Avoid unawaited async callbacks and hidden fire-and-forget. Handle cancellation, stale responses, and component destruction separately: signalling cancellation does not prove completion.
- Exposed data defaults to readonly intent; document shared references and runtime mutability. Type-level `readonly` does not deeply freeze objects; reference equality is not business identity.
- Retain semicolons, default to single-quoted ordinary strings, and use template strings for interpolation. Keep trailing commas in multiline structures where the target syntax permits, including applicable parameter lists. External fixed formats follow their syntax.
- Place unit/cross-module tests in separate `tests/` directories, organized by source responsibility and included in actual discovery. Do not expose production internals for this layout or migrate existing tests automatically during standards cleanup.
- Use the configured Prettier or existing equivalent. Do not weaken build/static diagnostics to obtain a pass. Importing business libraries must not start servers or alter the host environment.

<a id="web"></a>

### 8.7 HTML, CSS, and frontend components

- Use semantic HTML, with explicit control names, keyboard behavior, and focus handling. Visual appearance does not replace accessible names. Do not insert untrusted HTML through concatenation.
- CSS names express components/states; maintain shared design values through common variables/tokens. Control global scope, selector coupling, and cascade; escalating specificity or `!important` must not patch structural defects.
- Follow the owning public component/library's established CSS naming/isolation, including namespaces, states, and styling interfaces; BEM is not universally required. Establish ownership and match peers for new components. Do not replace conventions by preference; projects define missing conventions without implicitly introducing a library.
- Define component, event, input, and output contracts; component names default to `PascalCase`. Partition state by ownership; do not duplicate derivable state without justification or use array positions as mutable entities' stable identities.
- Component filenames match names (`Entry.vue`, `Entry.tsx`); ordinary modules follow JS/TS rules. Event handlers use `on`, such as `onClick` and `onChange`; business operations still use responsibility names.
- In Vue, prefer Composition API, explicit props/emits, and TypeScript single-file components; composables use `use`. Other frameworks retain their lifecycle idioms; migration to Vue is not required.
- Vue single-file sections appear as `script`, `template`, then `style`, omitting unnecessary sections.
- Reclaim listeners, observers, timers, animation frames, network tasks, and GPU resources with their owners. Keep high-frequency computation/external mutable engines separate from reactive UI state; avoid deep proxies that obscure updates/ownership.
- Separate business state, rendering, and data adaptation. Projects register frameworks, directories, style isolation, and build targets; one application's directory names are not a universal frontend layout.

<a id="jvm"></a>

### 8.8 Java / Kotlin

- Packages are lowercase; types use `PascalCase`; methods/fields use `camelCase`; constants use `UPPER_SNAKE_CASE`. Narrow visibility to actual contracts; public setters/mutable collections are not default APIs.
- Define Java null boundaries explicitly; prefer Kotlin immutable `val` and nullable types. Do not bypass validation with unjustified non-null assertions. Validate Java/Kotlin interop and reflection inputs too.
- Express exceptions/results consistently across calls, preserving causes and interruption/cancellation semantics. Do not catch everything and return successful defaults or return damaged state to callers.
- Close files, connections, and other closeable resources through structured scopes; GC does not replace closing. Threads, executors, coroutine scopes, and tasks need owners; global orphan tasks must not hide lifecycle duties.
- Distinguish readonly collection interfaces from underlying mutability, and synchronize actual concurrent access. Avoid deep inheritance for similar methods and annotations/reflection that conceal commits or resource ownership.
- Existing project tools control formatting/static diagnostics; register platform versions and interoperability separately.

<a id="csharp"></a>

### 8.9 C#

- Types, methods, and properties use `PascalCase`; parameters/locals use `camelCase`. Follow the `I` interface prefix and usual `Async` suffix for awaitable methods. Private instance fields have a trailing underscore, such as `entry_`.
- Use brace-block namespaces rather than defaulting to file-scoped namespaces. Use local `var` when the initializer makes the type obvious; otherwise spell out the type.
- Data models default to ordinary classes. Choose `record` individually for actual value equality, copying, and immutability needs; do not convert all data classes mechanically.
- Use nullable-reference analysis where supported and still validate external input. Null-forgiving operators must not remove unproven risks. Define copying semantics for values, references, and mutable collections.
- Use `using` or appropriate async disposal scopes. Specify ownership of native handles/FFI buffers; finalizers cannot guarantee timely release or business commits.
- Callers observe Task errors/completion and explicitly propagate CancellationToken. Avoid `async void` except framework-required event boundaries; synchronous waiting does not replace a correct async chain.
- Preserve causes during exception translation and distinguish cancellation from failure. Do not call unknown callbacks while locked; background tasks must not implicitly violate UI-thread/context constraints.
- Organize namespaces, source, and test projects by responsibility. Existing configuration controls formatting/analyzers. ABI, marshalling, and native dependencies are explicit interface contracts.

<a id="lua"></a>

### 8.10 Lua

- Owned functions, variables, and files default to `snake_case`. Use `local` state and explicitly return module APIs; do not share business state through implicit globals.
- Indent with four spaces. Recoverable failures default to `nil, error`, clearly distinguished from success. Handle programming errors/host-required exception boundaries by contract rather than downgrading every error.
- Distinguish `nil`, `false`, empty tables, and zero. Do not mix implicit length assumptions for lists, maps, and sparse tables or depend on unspecified iteration order.
- Tables are shared references; specify readonly-view/copy semantics. Metatables/dynamic dispatch serve actual requirements without hiding ownership or error boundaries.
- Fix external error conventions; use `pcall/xpcall` only at boundaries able to handle errors. Do not swallow failure or equate coroutine suspension with completion.
- Clean files, connections, native userdata, and coroutines according to runtime capabilities. Register version, sandbox, and numeric representation; Lua environments do not all share identical libraries/integer behavior.

<a id="sql"></a>

### 8.11 SQL and database migrations

- Owned tables, columns, and constraints default to `snake_case`; uppercase SQL keywords such as `SELECT` and `FROM`. New table names are plural (`entries`); do not rename published schemas for singular/plural uniformity. Follow the target dialect's case/quoting rules.
- Explicitly use `AS` for aliases where supported, including applicable table/column aliases; uniformity does not justify unsupported syntax.
- Bind data parameters. Dynamic identifiers come from explicit allowed sets and use correct dialect quoting; do not concatenate external text into executable SQL.
- Select required columns explicitly; specify ordering/tie handling for stable or paged results. Stable interfaces must not depend on `SELECT *` or incidental database order.
- Transactions, constraints, and application validation jointly enforce commitments. Define rollback and unknown outcomes; verify bulk-operation scope and do not hide partial success by catching errors and continuing.
- Version migrations by fixed order/content identity; do not rewrite deployed migrations. Define compatibility windows, locks/costs, rollback points, and irreversible boundaries. Presence in the repository does not authorize execution.

<a id="shell"></a>

### 8.12 Shell / PowerShell

- Declare interpreter/platform; do not confuse POSIX shell, Bash, and PowerShell syntax/exit rules. Follow environment naming conventions and do not repurpose reserved system variables for task state.
- Internal PowerShell functions use short complete responsibility names; external commands use Verb-Noun, preferably approved verbs. Owned Shell functions use lowercase words and `camelCase` for unavoidable compounds.
- Owned CLIs prefer readable long options, with documented short aliases for common ones. Use actual interpreter/parser syntax rather than forcing one prefix across platforms. Aliases preserve semantics; do not rewrite external tools' fixed arguments.
- Prefer argument arrays/literal paths, specifying wildcard, space, encoding, and stdin semantics. Do not eval/concatenate external inputs into commands; JSON escaping is not shell escaping.
- Check actual exit codes and every pipeline stage, not just global strict mode; propagate output-parsing errors too. Logs must not print credentials or complete secret-bearing commands.
- Assign ownership to temporary paths and descendant processes. Use verifiable exit/cancellation cleanup; resolve and confirm target boundaries before deletion/moving. Do not compose destructive operations across shells.
- Separate reusable logic from command entry points. Do not modify user/system environment for a local path issue, download dependencies implicitly, or turn one-off operations into import/startup side effects.

<a id="protocol"></a>

### 8.13 Protocols, configuration, and generation templates

- Define names, types, units, defaults, missing values, and unknown-value policy. Schemas own structure; product contracts add cross-field semantics. Do not maintain contradictory handwritten structures.
- Follow compatibility rules for field numbers, enum values, and reservations; never reuse reserved identities. JSON/YAML/TOML configuration must reject ambiguity, with explicit parser capabilities and duplicate-field policy.
- Maintain templates to production safety/coding standards. Encode inputs/outputs for their contexts; do not generate source, SQL, HTML, or shell by concatenating unescaped text.
- Trace inputs, generators, and outputs; do not hand-edit generated results. Projects decide whether to track artifacts; generation does not exempt consumer verification.
- Configuration must not contain real secrets. Mark example values clearly; missing required settings fail explicitly rather than silently selecting permissive access, external services, or production environments.

## 9. Documentation and design ownership

README owns purpose/navigation/startup; the development standard owns process/acceptance; this document owns coding/organization; feature documents own complete design; configuration owns executable parameters; validation owns evidence/boundaries. Reference or generate commands/thresholds from one configuration.

New features/public behavior changes follow development Section 2.4: named design, resolved critical choices/boundaries, then implementation approval. Directory examples are not approved designs; documented commands grant no execution authority.

Keep current documents and necessary compatibility material; version control owns obsolete process history. Preserve published API/migration/legal texts and historical protocol fixtures. Moves update navigation/references and migration arrangements for promised external links.

## 10. Reorganization, migration, and cleanup

Bound reorganization by users/objectives. Before moves/renames/deletion, inspect builds, imports, package/test discovery, generators, resources, installation, docs, and external consumers. Distinguish path cleanup from behavior/publication changes.

Remove dead code, obsolete switches, and temporary implementations only after reference/behavior checks. Text-search absence cannot rule out dynamic, platform, or compatibility use. Use version control, not `old`/`backup` source copies.

Regenerable does not mean safe to delete. Verify paths/ownership and preserve active outputs, raw evidence, user files, compatibility fixtures, and licenses. Ignore rules govern tracking, not deletion safety, publication, or secret checks.

Do not rename, convert, or upgrade dependencies repository-wide merely for style. Preserve user changes; development/project rules still govern verification and commits.

## 11. Integrating rules into automated checks

Register this content identity/applicable languages and map requirements to real configuration/check entries. Implement necessary integration explicitly; adding a document cannot authorize schema changes or invent supported fields/gate capabilities.

| Requirement | Appropriate enforcement |
| --- | --- |
| Formatting, naming form, imports, language diagnostics | Configured formatters, compilers, or static checks with explicit versions/file scope |
| Module public surface/dependency direction | Language visibility, build-target dependencies, and applicable boundary checks |
| Ownership of tests, examples, generated and published files | Actual discovery manifests, build/package inputs, and generation consistency checks |
| Semantic naming, responsibility, abstraction, complexity tradeoffs | Design/contracts, relevant counterexamples, and necessary focused review; do not pretend static tools understand every design |
| Suppressions, exclusions, migration | Versioned configuration/exact scope; verify protection changes/exceptions under the development standard |

Map obligations to existing DEV-CONFIG, DEV-CORE, DEV-TRACE, DEV-ORACLE, and DEV-IMPACT where appropriate. IDs neither prove execution nor change their meaning; register extensions in project namespaces.

Check maintained handwritten production, tests, and tools; generated/third-party exclusions need provenance. Moves cannot evade checks. Avoid responsibility-blind line/file/depth limits and empty metric-satisfying wrappers.

Documentation is ready when standard responsibilities, language/project registrations, and references agree, with no invented tool capabilities. Adoption/release still follow the development standard. Change common rules only for uncovered gaps; solve covered issues in project configuration, implementation, or counterexamples.
