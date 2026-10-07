# Requirements Specification

## Introduction

What agent-harness-adapter offers tools that are not written in Rust: a manifest file that declares a tool's integration, the `agent-harness-adapter` command that installs it and bridges hooks, and the JSON contracts between `agent-harness-adapter` and the tool. The manifest and the hook contract live in the library, so the CLI and the language bindings share them. Source: PLAN-011.

## Glossary

- MANIFEST: a TOML or JSON file holding a tool's name, its harnesses, where it keeps its record and declined parts, and its integration
- MANIFEST DIR: the directory holding the manifest; relative file paths in it resolve against it
- TEXT: a manifest value that is either a string or `{ file = "<path>" }`, the file's contents
- BRIDGED HOOK: a hook whose command is `agent-harness-adapter hook`, which runs the tool's own command with the hook contract
- HOOK CONTRACT: the neutral hook input the tool reads on stdin and the answer it writes on stdout, both JSON, version 1

## Stakeholders

- TOOL AUTHOR: builds a tool in any language that plugs into agent harnesses
- AGENT USER: installs the tool's integration with `agent-harness-adapter` and keeps their own edits

## Requirements

### AHA-1: Manifest [MUST]

AS A tool author, I WANT to declare my integration in a file, SO THAT I need no Rust code to install it.

ACCEPTANCE CRITERIA

- [ ] AHA-1_AC-1 [ubiquitous]: The system SHALL read a manifest as TOML, or as JSON when its file name ends in `.json`, or from a JSON value given by a binding
- [ ] AHA-1_AC-2 [conditional]: IF the manifest's `version` is not 1 THEN the system SHALL refuse it, naming the versions it reads
- [ ] AHA-1_AC-3 [ubiquitous]: The manifest SHALL give the tool's `name`, its `harnesses` (a list of ids in preference order, or `"all"`, the default), its `record` path and its `declined` path (relative to the scope's root; defaults `.<name>/harness.toml`, `.<name>/harness.local.toml` at local scope, and `.<name>/config.toml`)
- [ ] AHA-1_AC-4 [ubiquitous]: The manifest SHALL hold every item of an integration (KIT-17): instructions, skills with extra files, hooks, MCP servers, allowed commands, allowed MCP tools, agents, commands, the hook entry match, and raw `file`, `region` and `merge` parts for one harness; the integration it gives SHALL equal the one built with the library's API from the same values
- [ ] AHA-1_AC-5 [ubiquitous]: A TEXT value SHALL be a string or `{ file = "<path>" }` relative to the MANIFEST DIR, read when the manifest is loaded
- [ ] AHA-1_AC-6 [ubiquitous]: A table `scopes.<scope>` SHALL replace, for that scope only, each item it names and `record` and `declined`
- [ ] AHA-1_AC-7 [conditional]: IF the manifest has an unknown key, a value of the wrong type, an unknown harness, event or tool kind, or a TEXT file that cannot be read THEN the system SHALL refuse it with an error naming the manifest and the place (line and column, or the key)
- [ ] AHA-1_AC-8 [ubiquitous]: A hook SHALL give either `command` (a hook command template, KIT-11_AC-2) or `run` (a BRIDGED HOOK: the template `<adapter> hook --tool <name> [--tools <kind>] {harness} {event} -- <run>`, with `--tools` when the hook gives `tools`, where `<adapter>` is the manifest's `adapter` key, default `agent-harness-adapter`), not both
- [ ] AHA-1_AC-9 [ubiquitous]: A loaded manifest SHALL be a tool (KIT-1) whose root is the given project directory at project and local scope and the home directory at user scope
- [ ] AHA-1_AC-10 [ubiquitous]: A skill SHALL give either `dir` (a skill directory relative to the MANIFEST DIR, read when the manifest is loaded, KIT-17_AC-5) or `name`, `description`, `body` and optional `files`, not both

### AHA-2: Hook contract [MUST]

AS A tool author, I WANT one hook format for every harness, SO THAT my tool reads and answers hooks the same way everywhere.

ACCEPTANCE CRITERIA

- [ ] AHA-2_AC-1 [ubiquitous]: The hook input SHALL be a JSON object with `"v": 1` and the fields of the neutral hook input (KIT-11_AC-3): `harness`, `event`, `session_id`, `cwd`, `transcript_path`, `prompt`, `tool` (`name`, `kind`, `input`), `tool_output`, `source`, `continuing`, `last_message`, `raw`; an absent value SHALL be left out
- [ ] AHA-2_AC-2 [ubiquitous]: The answer SHALL be a JSON object `{"answer": "allow" | "deny" | "continue" | "context"}` with `stderr` (allow, optional), `reason` (deny, continue) or `text` (context); unknown fields SHALL be ignored and an optional `v` other than 1 SHALL be an error
- [ ] AHA-2_AC-3 [ubiquitous]: Pi's generated extension (HAR-7) and OpenCode's generated plugin (HAR-10) SHALL read answers in the same format

### AHA-3: Install and status [MUST]

AS AN agent user, I WANT `agent-harness-adapter install` and `agent-harness-adapter status`, SO THAT I install a manifest's integration like any tool's.

ACCEPTANCE CRITERIA

- [ ] AHA-3_AC-1 [event]: WHEN `agent-harness-adapter install --manifest <file> --harness <ids|all>` runs THEN the system SHALL install the manifest's integration for those harnesses (KIT-4), `all` being every harness of the manifest with files at the scope, with `--scope`, `--root`, `--force` and `--without <parts>` as the library's options
- [ ] AHA-3_AC-2 [event]: WHEN `agent-harness-adapter status --manifest <file>` runs THEN the system SHALL report the state of the named harnesses, or of the recorded ones when none is named (KIT-5)
- [ ] AHA-3_AC-3 [ubiquitous]: The system SHALL print the result as text, or as the result's JSON with `--json`, and exit 0; on an error it SHALL print `error: <message>` and exit 2 (KIT-14)
- [ ] AHA-3_AC-4 [conditional]: IF a part is edited and was not written, and the output is text THEN the system SHALL add a note on stderr that `--force` overwrites it
- [ ] AHA-3_AC-5 [event]: WHEN `agent-harness-adapter uninstall --manifest <file> --harness <ids|all>` runs THEN the system SHALL uninstall the manifest's integration from those harnesses (KIT-22), `all` being the installed ones, with `--scope`, `--root`, `--force` and `--json` as for install; an edited part kept gets the `--force` note (AHA-3_AC-4)

### AHA-4: Hook bridge [MUST]

AS A tool author, I WANT `agent-harness-adapter hook` to speak each harness's protocol, SO THAT my command only ever sees the hook contract.

ACCEPTANCE CRITERIA

- [ ] AHA-4_AC-1 [event]: WHEN `agent-harness-adapter hook <harness> <event> -- <command> [args]` runs THEN the system SHALL parse its stdin through the harness (KIT-11), run the command (no shell) with the hook input (AHA-2_AC-1) on its stdin and its stderr passed through, read the answer (AHA-2_AC-2) from its stdout, and emit it through the harness (KIT-11_AC-5)
- [ ] AHA-4_AC-2 [conditional]: IF the command cannot start, exits non-zero, or writes no valid answer THEN the system SHALL allow, with `<tool>: <reason>` on stderr (`--tool`, default the command's name)
- [ ] AHA-4_AC-3 [conditional]: IF stdin is not JSON THEN the system SHALL send an input holding only `v`, `harness` and `event`
- [ ] AHA-4_AC-4 [conditional]: IF the harness or the event is unknown THEN the system SHALL write `error: <message>` to stderr and exit 1 without running the command
- [ ] AHA-4_AC-5 [conditional]: IF `--tools <kind>` is given and the harness's input names a tool of another kind THEN the system SHALL allow without running the command, so a harness that cannot match tools (Copilot, Cursor, Pi, OpenCode) runs the tool's command only for its kind

### AHA-5: Schemas [SHOULD]

AS A tool author, I WANT the contracts as JSON Schema, SO THAT I validate and generate types in my language.

ACCEPTANCE CRITERIA

- [ ] AHA-5_AC-1 [event]: WHEN `agent-harness-adapter schema <manifest|hook-input|hook-answer|result>` runs THEN the system SHALL print that contract's JSON Schema
- [ ] AHA-5_AC-2 [ubiquitous]: The repository's `schema/*.json` SHALL equal what `agent-harness-adapter schema` prints

### AHA-6: Distribution [MUST]

AS AN agent user, I WANT `agent-harness-adapter` from cargo or npm, SO THAT I install it with the tools I have.

ACCEPTANCE CRITERIA

- [ ] AHA-6_AC-1 [event]: WHEN `agent-harness-adapter --version` runs THEN the system SHALL print `agent-harness-adapter <version>`
- [ ] AHA-6_AC-2 [ubiquitous]: The npm launcher SHALL run the prebuilt binary for the host's platform with the given arguments, stdio and exit code (128 + the signal number when killed by a signal)
- [ ] AHA-6_AC-3 [conditional]: IF the host has no prebuilt binary, or its package is not installed THEN the launcher SHALL exit 1 naming the supported platforms or the missing package, and `cargo install agent-harness-adapter`

## Assumptions

- `agent-harness-adapter` is on the harness's `PATH`, or the manifest's `adapter` key gives the command that runs it
- A bridged hook's harness gives the hook enough time for one extra process

## Constraints

- The manifest and hook contract versions change only with a release that says so (PLAN-011 D11-8)

## Out of Scope

- External parts in a manifest (they need code)
- Timeouts inside `agent-harness-adapter hook` (the harness's hook timeout applies)

## Change Log

- 0.1.0 (2026-10-07): Initial requirements (PLAN-011)
