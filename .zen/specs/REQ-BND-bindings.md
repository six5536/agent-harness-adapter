# Requirements Specification

## Introduction

Native Python and Node APIs over the library, for tools in those languages that would rather call the library than run `agent-harness-adapter`: install and status from a manifest, the hook contract without an extra process, and the contracts' schemas. They speak the same JSON formats as `agent-harness-adapter` (REQ-AHA). Source: PLAN-011 (D11-10, D11-13).

## Glossary

- BINDING: the Python package `six5536-agent-harness-adapter` (module `agent_harness_adapter`) or the Node package `@six5536/agent-harness-adapter-node`
- MANIFEST SOURCE: a manifest file's path, or a manifest as an object (dict) whose TEXT files resolve against the working directory
- RESULT: the install or status result as an object, equal to `agent-harness-adapter ... --json`

## Stakeholders

- TOOL AUTHOR: builds a tool in Python or JavaScript that plugs into agent harnesses

## Requirements

### BND-1: Install and status [MUST]

AS A tool author, I WANT to install my manifest from my own code, SO THAT my tool's install command needs no other program.

ACCEPTANCE CRITERIA

- [ ] BND-1_AC-1 [event]: WHEN `install(manifest, harnesses, scope, root, home, force, without)` is called THEN the BINDING SHALL install the MANIFEST SOURCE's integration and return the RESULT; `scope` defaults to project, `root` to the working directory, `home` to the user's home directory, `all` stands for every harness of the manifest with files at the scope
- [ ] BND-1_AC-2 [event]: WHEN `status(manifest, harnesses, scope, root, home)` is called THEN the BINDING SHALL return the RESULT of status, for the installed harnesses when none is named
- [ ] BND-1_AC-3 [conditional]: IF the library refuses (KIT-16) or the manifest is wrong (AHA-1_AC-7) THEN the BINDING SHALL raise (Python: `ValueError`) or throw (Node: `Error`) with the library's message

### BND-2: Hooks [MUST]

AS A tool author, I WANT the hook contract in my language, SO THAT my hook command answers every harness without `agent-harness-adapter`.

ACCEPTANCE CRITERIA

- [ ] BND-2_AC-1 [event]: WHEN `parse_hook(harness, event, text)` is called THEN the BINDING SHALL return the hook contract's input (AHA-2_AC-1) parsed from the harness's input text, holding only `v`, `harness` and `event` when the text is not JSON
- [ ] BND-2_AC-2 [event]: WHEN `answer_hook(harness, event, answer)` is called with a contract answer (AHA-2_AC-2) THEN the BINDING SHALL return what the hook command writes: `stdout` (with its final newline), `stderr` (or none) and `exit`, as `agent-harness-adapter hook` would emit it
- [ ] BND-2_AC-3 [conditional]: IF the harness or event is unknown, the answer is not a contract answer, or the harness cannot express it for the event THEN the BINDING SHALL raise or throw with the library's message
- [ ] BND-2_AC-4 [event]: WHEN `run_hook(harness, event, decide)` (Node: `runHook`) is called THEN the BINDING SHALL read the harness's input from stdin, pass its contract input to `decide`, write `decide`'s answer as the harness expects it to stdout and stderr, and return the exit code

### BND-3: Schemas and versions [SHOULD]

AS A tool author, I WANT the schemas and versions from the binding, SO THAT I validate against the contracts the binding speaks.

ACCEPTANCE CRITERIA

- [ ] BND-3_AC-1 [event]: WHEN `schema(contract)` is called with `manifest`, `hook-input`, `hook-answer` or `result` THEN the BINDING SHALL return that schema as an object, equal to `agent-harness-adapter schema`
- [ ] BND-3_AC-2 [ubiquitous]: The BINDING SHALL expose its version (`__version__` / `version`) and the contract versions (`MANIFEST_VERSION`, `HOOK_VERSION`)

### BND-4: Distribution [MUST]

AS A tool author, I WANT prebuilt packages, SO THAT installing the binding needs no Rust toolchain.

ACCEPTANCE CRITERIA

- [ ] BND-4_AC-1 [ubiquitous]: The Python BINDING SHALL ship as abi3 wheels (CPython 3.9 and newer) for Linux (x64, arm64), macOS (x64, arm64) and Windows (x64), and an sdist
- [ ] BND-4_AC-2 [ubiquitous]: The Node BINDING SHALL ship a prebuilt addon per platform (as the CLI's npm packages) behind one package with TypeScript types

## Constraints

- The bindings share one Rust implementation of their JSON glue; the languages differ only in their wrappers

## Out of Scope

- A native mirror of the Rust builder API (`Integration`, `Hook`, …): a manifest object is the declaration
- External parts (they need Rust)

## Change Log

- 0.1.0 (2026-10-07): Initial requirements (PLAN-011)
