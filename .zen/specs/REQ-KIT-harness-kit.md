# Requirements Specification

## Introduction

What agent-harness-kit-core (the library crate) guarantees to the tools that use it: a tool's integration declared once and installed into any number of harnesses with shared content deduplicated, hook input and answers for any harness, the loop guard, findings reports, CLI conventions and file IO. What each built-in harness does is in REQ-HAR. Source: PLAN-009 (carrying over smllm's HOST-10, HOST-11, NFR-4, NFR-6 and PLAN-003 F19–F21) and PLAN-010.

## Glossary

- TOOL: the CLI that uses the kit, e.g. smllm
- HARNESS: an agent runtime the tool plugs into, e.g. Claude Code, and the kit's adapter for it, named by an id (`claude`, `codex`, …)
- SCOPE: where a harness's files live: `project`, `user` (the home directory) or `local` (the project's git-ignored files)
- INTEGRATION: what a tool installs, declared once without naming a harness: ITEMS, plus raw parts for one harness
- ITEM: one kind of thing in an integration: instructions, skills, hooks, mcp, permissions, agents, commands
- PROFILE: the parts of one harness at one scope, rendered from the integration
- PART: one piece of a profile, of one kind: file, region, merge or external; named by its item (`instructions`, `hooks`, …) or by the tool for a raw part
- LOCATION: a file or directory, relative to the root, that a part writes or a harness reads
- SHARED: a part whose content another harness's part already puts in a location this harness reads
- HOOK EVENT: one of session start, session end, prompt submit, pre tool, post tool, stop, pre compact
- ROOT: the directory a profile's paths are relative to
- RECORD: a TOML file holding, per harness, the hash of each part's content as the tool last wrote it
- DECLINED PARTS: the parts a user chose not to install (`--without`)
- REFUSAL: an error returned before any write
- MARKERS: `<!-- <tool>:harness -->` and `<!-- /<tool>:harness -->`, each on a line of its own

## Stakeholders

- TOOL AUTHOR: builds a CLI that installs into harnesses and answers their hooks
- AGENT USER: runs the tool's install and keeps their own edits in the same files

## Requirements

### KIT-1: Tool [MUST]

AS A tool author, I WANT to declare my integration as data, SO THAT the kit installs it without knowing my content.

ACCEPTANCE CRITERIA

- [ ] KIT-1_AC-1 [ubiquitous]: The system SHALL take from the tool its name, the harnesses it supports, an integration per scope, the root, the record path and the declined parts store of each scope, and SHALL embed no content of its own
- [ ] KIT-1_AC-2 [conditional]: IF a harness named to `install` or `status` is not one the tool supports THEN the system SHALL refuse with an unknown-harness error; IF the harness has no files at the scope THEN it SHALL refuse with an unsupported-scope error
- [ ] KIT-1_AC-3 [ubiquitous]: The name `all` among harness names SHALL stand for every harness the tool supports that has files at the scope, in the tool's order

### KIT-2: Part kinds [MUST]

AS A tool author, I WANT four kinds of part, SO THAT I can own whole files, a block in the user's file, entries in a JSON file, or something only my tool can read.

ACCEPTANCE CRITERIA

- [ ] KIT-2_AC-1 [ubiquitous]: A file part SHALL own whole files under a directory, written with LF line endings
- [ ] KIT-2_AC-2 [ubiquitous]: A region part SHALL own the block between the tool's markers in one file, either a fixed path or a path a rule chooses under the root at install time
- [ ] KIT-2_AC-3 [ubiquitous]: A merge part SHALL own entries in the JSON object of one file: an array entry (found by equality), an object member (found by its key), the tool's entries in an array (found by a field that starts with, or contains, one of the texts the tool chooses), or the tool's entries inside the groups of an array (found the same way)
- [ ] KIT-2_AC-5 [ubiquitous]: A merge part SHALL own object members (found by their key) in the table of a TOML file
- [ ] KIT-2_AC-4 [ubiquitous]: An external part SHALL be read and written by the tool itself through the kit's interface, with a location shown in reports

### KIT-3: Part states [MUST]

AS AN agent user, I WANT each part's state told apart, SO THAT the tool never overwrites my edits by surprise.

ACCEPTANCE CRITERIA

- [ ] KIT-3_AC-1 [ubiquitous]: The system SHALL give each part exactly one state: skipped (declined), shared (KIT-19), absent (nothing the tool could own; for a file part, none of its files), current (equal to the tool's content), stale (differs, and equals the recorded hash), edited (differs from both, or present with no recorded hash)
- [ ] KIT-3_AC-2 [ubiquitous]: The system SHALL compare files and external text ignoring CRLF/LF differences, a region block by its words, and merge entries by JSON equality

### KIT-4: Install [MUST]

AS AN agent user, I WANT install to create, update or leave each part by its state, SO THAT reinstalling is safe.

ACCEPTANCE CRITERIA

- [ ] KIT-4_AC-1 [event]: WHEN `install` runs THEN the system SHALL write absent and stale parts, leave current and skipped parts, and leave edited parts unless forced
- [ ] KIT-4_AC-2 [event]: WHEN `install` writes or finds current a part THEN the system SHALL record its content hash; a skipped part's hash SHALL be removed; an edited part left as found SHALL keep its recorded hash
- [ ] KIT-4_AC-3 [ubiquitous]: The result SHALL give, per harness named, in the tool's order, per part in profile order, its name, its path relative to the root (an external part's location; a shared part's covering location), the state found, the harness that writes a shared part, and what install did: created (absent before; for a file part, none of its files existed), rewrote (a file part that existed), updated (a region or merge that existed), or nothing; then the items the harness does not support at the scope, and its notes (KIT-20); and, for the whole run, the warnings of KIT-19_AC-4
- [ ] KIT-4_AC-4 [ubiquitous]: The text form SHALL be, per harness, a `<harness>:` line, then, indented two spaces, one line per part, `<word> <path> (<part>)` (`(<part>, by <harness>)` for a shared part), the word (the action, else the state) padded to seven columns, then `unsupported: <items>` when any, then one `note: <text>` line per note; then one `warning: <text>` line per warning; the JSON form SHALL carry the same content

DEPENDS ON: KIT-3

### KIT-5: Status [MUST]

AS AN agent user, I WANT the state of every part without changes, SO THAT I can see what install would do.

ACCEPTANCE CRITERIA

- [ ] KIT-5_AC-1 [event]: WHEN `status` runs THEN the system SHALL report each part's state as `install` would find it and SHALL write nothing
- [ ] KIT-5_AC-2 [ubiquitous]: The system SHALL list the harnesses installed at a scope (those in its record), in the tool's order, writing nothing

### KIT-6: Refusals write nothing [MUST]

AS AN agent user, I WANT a failed install to leave every file as found, SO THAT nothing is half-installed.

ACCEPTANCE CRITERIA

- [ ] KIT-6_AC-1 [conditional]: IF a harness is unknown or lacks the scope, `--without` names a part no named harness's profile has, two parts of one location differ (KIT-19_AC-5), or a file the kit must read (a merge target, the record, the declined store) does not parse or has the wrong shape THEN `install` SHALL refuse before any write, naming the file
- [ ] KIT-6_AC-2 [ubiquitous]: The system SHALL write external parts before any file, so a failing external part leaves every file as found

### KIT-7: Writes [MUST]

AS AN agent user, I WANT the tool's writes to be minimal and safe, SO THAT my files and their history stay clean.

ACCEPTANCE CRITERIA

- [ ] KIT-7_AC-1 [ubiquitous]: The system SHALL write a file only when its text changes, atomically, after planning every write; several parts in one file SHALL produce one write
- [ ] KIT-7_AC-2 [ubiquitous]: The record SHALL be the tool's header line, then one table per installed harness in name order (empty when it has no hash, every part shared or declined) with part hashes in name order, LF; a record that does not change SHALL not be written unless the file is missing

### KIT-8: Declined parts [MUST]

AS AN agent user, I WANT to decline a part once, SO THAT later installs keep skipping it.

ACCEPTANCE CRITERIA

- [ ] KIT-8_AC-1 [event]: WHEN `install` is given `--without` THEN the system SHALL use that list as the declined parts of every harness named, leaving out names a harness's profile lacks, and store it per harness after the parts are written; otherwise each harness's stored list SHALL apply
- [ ] KIT-8_AC-2 [ubiquitous]: The system SHALL never read a declined part's target, so a broken file only a declined part uses cannot block the rest
- [ ] KIT-8_AC-3 [ubiquitous]: The TOML store SHALL keep the list as `[harness.<name>] without = [...]` in a file the tool names, edited in place with its other content and comments kept

### KIT-9: Region rules [MUST]

AS AN agent user, I WANT the tool's block kept apart from my text, SO THAT reinstalling never touches my content.

ACCEPTANCE CRITERIA

- [ ] KIT-9_AC-1 [ubiquitous]: The region SHALL be the one the first closing marker ends, opened by the nearest opening marker before it; a stray opening marker SHALL stay the user's text
- [ ] KIT-9_AC-2 [event]: WHEN a region is written THEN the system SHALL replace the block between the markers, else append it after one blank line, or make it the whole file when the file is absent or empty; keep the file's line endings; and put a blank line after the opening marker when the block is not empty
- [ ] KIT-9_AC-4 [ubiquitous]: Reading a region SHALL drop only the one blank line that follows the opening marker, so a block reads back as written (LF line endings, a final newline)
- [ ] KIT-9_AC-3 [ubiquitous]: The system SHALL change nothing outside the region

### KIT-10: Merge rules [MUST]

AS AN agent user, I WANT the tool's JSON entries merged into my file as I wrote it, SO THAT diffs show only the tool's change.

ACCEPTANCE CRITERIA

- [ ] KIT-10_AC-1 [ubiquitous]: A merge SHALL keep every other entry, the key order, the file's indent (that of its first indented line, else two spaces) and whether it ends with a newline; a new file SHALL use two spaces and a final newline
- [ ] KIT-10_AC-2 [ubiquitous]: A merge SHALL create missing containers and SHALL refuse, naming the file and path, when a container has another type
- [ ] KIT-10_AC-3 [ubiquitous]: For group entries the tool SHALL own only its entries inside the groups of an array: the user's entries and other keys SHALL be kept and not count as an edit; each of the tool's groups SHALL take the place of the tool's entries in the first group with the same other keys (e.g. a matcher) that held one, else be appended; a group that held only the tool's entries and gets none back SHALL be removed
- [ ] KIT-10_AC-4 [ubiquitous]: For owned entries in an array the tool's entries SHALL take the place of its first one, else be appended, and its other entries be removed; the user's entries SHALL be kept and not count as an edit
- [ ] KIT-10_AC-6 [ubiquitous]: An owned or group operation with no entries SHALL only remove the tool's entries, create no container, and expect nothing, so the tool's entries for a hook event it no longer uses make the part stale
- [ ] KIT-10_AC-5 [ubiquitous]: A TOML merge SHALL keep every other key, comments, key order and formatting; a new file SHALL hold only the tool's tables

### KIT-11: Hooks [MUST]

AS A tool author, I WANT one hook model for every harness, SO THAT my hook command is written once.

ACCEPTANCE CRITERIA

- [ ] KIT-11_AC-1 [ubiquitous]: A hook SHALL be a hook event and a command template; each harness SHALL install it under its own name for the event, and leave out events it lacks
- [ ] KIT-11_AC-2 [ubiquitous]: The command template SHALL be any command line the tool chooses, with no required words or argument order; the installed command SHALL be the template with each `{harness}` replaced by the harness id, each `{event}` by the event's name (`session-start`, `session-end`, `prompt-submit`, `pre-tool`, `post-tool`, `stop`, `pre-compact`), and `{{` / `}}` by literal braces; placeholders SHALL be optional; a hook MAY give its own template for one harness
- [ ] KIT-11_AC-7 [ubiquitous]: A hook's entries SHALL be found by the entry match set on the hook, else the one set for all the tool's hooks, else a prefix of each of its templates: the text before its first placeholder, or the whole template when it has none
- [ ] KIT-11_AC-8 [conditional]: IF a hook's match would be an empty prefix (the template starts with a placeholder) THEN the system SHALL refuse with an internal error
- [ ] KIT-11_AC-3 [ubiquitous]: Hook input SHALL parse, by the harness and event, into one input: session, working directory, transcript path, prompt, the tool call (name, kind, input), the tool's output, the session start source, whether the agent is already continuing from a stop hook, the last assistant message, and the raw JSON; every field optional, unknown fields ignored; input that is not JSON SHALL be an error the caller handles
- [ ] KIT-11_AC-4 [ubiquitous]: An answer SHALL be allow (optionally with text for stderr), deny with a reason (before a tool or a prompt), continue with a reason (at stop), or context text (session start, prompt submit, after a tool); each harness SHALL render it as its stdout JSON and exit code; an answer a harness cannot express for the event SHALL be an error
- [ ] KIT-11_AC-5 [event]: WHEN a hook answer is emitted THEN the system SHALL write its stderr text, then the harness's stdout text, and give the harness's exit code; WHEN the hook failed THEN it SHALL write `error: <message>` on stderr, nothing on stdout, and give exit 1
- [ ] KIT-11_AC-6 [ubiquitous]: A tool call's kind SHALL be shell, read, write (create or edit a file), mcp or other, from the harness's tool name; a hook MAY be limited to one kind, which a harness that can match tool names SHALL install as its matcher and another SHALL ignore

DEPENDS ON: KIT-2, KIT-10, KIT-18

### KIT-12: Loop guard [SHOULD]

AS A tool author, I WANT a stop hook to block once per text, SO THAT the agent is never stuck in a loop.

ACCEPTANCE CRITERIA

- [ ] KIT-12_AC-1 [event]: WHEN asked whether to block on a text for a key THEN the system SHALL answer no if it blocked on the same text last time for that key, else yes and record the text's hash
- [ ] KIT-12_AC-2 [ubiquitous]: The guard SHALL keep one file per key in a directory the caller names, created with a `.gitignore` of `*`; a key that is not short and safe SHALL be stored under its hash; failures SHALL be silent

### KIT-13: Findings report [MUST]

AS A tool author, I WANT findings reported one way, SO THAT my tools' output agrees.

ACCEPTANCE CRITERIA

- [ ] KIT-13_AC-1 [ubiquitous]: A finding SHALL have a path, an optional line, a severity (error, warning, info), a message and an optional authority
- [ ] KIT-13_AC-2 [ubiquitous]: A report SHALL keep each severity's findings ordered by path, line and message, without duplicates
- [ ] KIT-13_AC-3 [ubiquitous]: The text form SHALL be one `<path>:<line>: <level>: <message> (<authority>)` line per shown finding (without `:<line>` or the authority when absent), ordered by path then line, then `<n> error(s), <n> warning(s), <n> info`; errors are always shown, warnings and info on request
- [ ] KIT-13_AC-4 [ubiquitous]: The JSON form SHALL be `{"errors":[…],"warnings":[…],"info":[…]}`, each finding `{path, line?, message, authority?}`

### KIT-14: CLI conventions [MUST]

AS A tool author, I WANT shared exit and output behaviour, SO THAT my CLIs behave alike in pipes and scripts.

ACCEPTANCE CRITERIA

- [ ] KIT-14_AC-1 [ubiquitous]: Exit codes SHALL be 0 (ok), 1 (errors found) and 2 (usage or internal error)
- [ ] KIT-14_AC-2 [event]: WHEN a command fails THEN the system SHALL print `error: <message>` on stderr and exit 2, unless the failure is a closed downstream pipe, which SHALL exit 0 silently
- [ ] KIT-14_AC-3 [ubiquitous]: Stdout writes SHALL be flushed before returning, so a broken pipe surfaces as an error; a JSON line SHALL be one object and a newline

### KIT-15: File IO [MUST]

AS A tool author, I WANT reads and writes that respect the user's setup, SO THAT a write never corrupts or replaces what they arranged.

ACCEPTANCE CRITERIA

- [ ] KIT-15_AC-1 [ubiquitous]: Reading a file SHALL give its text, or nothing when it is absent; other failures SHALL be errors naming the path
- [ ] KIT-15_AC-2 [ubiquitous]: An atomic write SHALL create parent directories, write a temp file beside the target (unique per write) with the target's permissions, and rename it over the target; it SHALL replace the file a symlink points to, not the link; on failure it SHALL remove the temp file
- [ ] KIT-15_AC-3 [ubiquitous]: The user's home directory SHALL be `HOME`, else `USERPROFILE`, when set and not empty

### KIT-16: Errors [MUST]

AS A tool author, I WANT typed errors, SO THAT I can react to each kind of refusal.

ACCEPTANCE CRITERIA

- [ ] KIT-16_AC-1 [ubiquitous]: Each refusal SHALL be its own error kind: unknown scope, unknown hook event, unknown harness, unsupported scope, unknown part, an unsupported answer, a file that does not parse or has the wrong shape, and a refusal from a tool's own external part or store; IO failures SHALL name the path; each SHALL display a one-line message

### KIT-17: Integration [MUST]

AS A tool author, I WANT to declare my integration once, SO THAT every harness gets it in its own form.

ACCEPTANCE CRITERIA

- [ ] KIT-17_AC-1 [ubiquitous]: An integration SHALL hold an instructions block, skills (name, description, body, extra files), hooks (KIT-11), MCP servers (stdio: command, arguments, environment; or http: URL, headers), allowed commands (a command prefix) and allowed MCP tools (server, tool), agents (name, description, prompt), commands (name, description, prompt with `$ARGUMENTS`), and raw parts for one harness
- [ ] KIT-17_AC-2 [ubiquitous]: Each item SHALL become at most one part per harness, named by the item; a raw part SHALL keep the tool's name
- [ ] KIT-17_AC-3 [conditional]: IF a raw part's name equals that of a part the harness rendered or another raw part's THEN the system SHALL refuse with an internal error; a raw part named like an item the harness does not render SHALL stand for that item
- [ ] KIT-17_AC-4 [ubiquitous]: An item a harness cannot take at a scope SHALL be left out of its profile and listed as unsupported, never an error

### KIT-18: Harnesses [MUST]

AS A tool author, I WANT harnesses as values behind one contract, SO THAT I pick the ones I support and can add my own.

ACCEPTANCE CRITERIA

- [ ] KIT-18_AC-1 [ubiquitous]: A harness SHALL give its id, its scopes, the hook events it runs, the locations it reads per item and scope (those it always loads, and those it may load), the parts it renders from an integration, hook input parsing, answer rendering, and notes
- [ ] KIT-18_AC-2 [ubiquitous]: The system SHALL provide the harnesses of REQ-HAR, a list of them all, and a lookup by id
- [ ] KIT-18_AC-3 [ubiquitous]: A harness from outside the kit SHALL work with `install`, `status` and the hook functions without a kit change
- [ ] KIT-18_AC-4 [ubiquitous]: A harness SHALL render and decide what it reads from the integration and the files under the root, so its choices follow the user's tree

### KIT-19: Shared locations [MUST]

AS AN agent user, I WANT shared content installed once, SO THAT no agent loads the tool's content twice.

ACCEPTANCE CRITERIA

- [ ] KIT-19_AC-1 [ubiquitous]: `install` and `status` SHALL work on a set of harnesses: those named, plus those the tool supports with a table in the scope's record; only the named harnesses' parts SHALL be written and reported
- [ ] KIT-19_AC-2 [ubiquitous]: Per item, the system SHALL choose, from the locations the set's parts would write, the fewest that every harness not declining the item always loads one of; among equal choices, the one whose locations the most harnesses load, then the earliest harness order
- [ ] KIT-19_AC-3 [ubiquitous]: Each chosen location SHALL be written by the earliest named harness whose part has it, else the earliest; every other harness's part for the item SHALL be shared, naming the first chosen location it loads and that location's writer; nothing SHALL be written or recorded for it
- [ ] KIT-19_AC-4 [conditional]: IF a harness named loads, or may load, more than one chosen location of one item THEN the result SHALL carry a warning naming the harness, the item and the locations
- [ ] KIT-19_AC-5 [conditional]: IF two harnesses' parts for one item and location differ THEN the system SHALL refuse with an internal error
- [ ] KIT-19_AC-6 [conditional]: IF a harness's record holds a hash for a part that is now shared THEN the result SHALL warn that its earlier copy is left at its location, and the hash SHALL be kept
- [ ] KIT-19_AC-7 [ubiquitous]: The harness order SHALL be the tool's list order, whatever order they are named in, so the same set always gives the same choice of locations; a part's recorded hash SHALL be its writer's, else that of another harness in the set whose part for the item has the same location

### KIT-20: Notes [SHOULD]

AS AN agent user, I WANT to be told what is left to do after an install, SO THAT installed parts take effect.

ACCEPTANCE CRITERIA

- [ ] KIT-20_AC-1 [ubiquitous]: A harness MAY give notes after an install from its parts' results (e.g. approve hooks, trust the folder, reload), shown after its parts

### KIT-21: Scopes [MUST]

AS AN agent user, I WANT project, user and local installs, SO THAT I choose who gets the tool.

ACCEPTANCE CRITERIA

- [ ] KIT-21_AC-1 [ubiquitous]: Scopes SHALL be `project` (the project root), `user` (the home directory) and `local` (the project root, git-ignored files only); each harness SHALL state which scopes it has

## Assumptions

- Each harness's files and hook formats behave as its documentation says on 2026-10-06 (REQ-HAR)

## Constraints

- The core knows no harness's formats; each harness's formats live in its own module
- No public API exposes a type from another crate's 0.x release
- MSRV 1.85

## Out of Scope

- Removing (uninstalling) parts
- Moving or removing a part a harness no longer needs after the set changes (follows uninstall)
- Running hooks or commands for the tool

## Change Log

- 0.1.0 (2026-10-06): Initial requirements (PLAN-009); entry matching by contained text and exact region round trip, from the sokf port (D9-20); integration, harnesses, shared locations, neutral hooks, TOML merge, owned entries, local scope, notes (PLAN-010)
