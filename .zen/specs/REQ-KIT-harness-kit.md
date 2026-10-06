# Requirements Specification

## Introduction

What agent-harness-kit guarantees to the tools that use it: installing and reporting a tool's harness parts, Claude Code's hook formats, the loop guard, findings reports, CLI conventions and file IO. Source: PLAN-009, carrying over the behaviour smllm's HOST-10, HOST-11, NFR-4, NFR-6 and PLAN-003 F19–F21 specified for the kit.

## Glossary

- TOOL: the CLI that uses the kit, e.g. smllm
- HARNESS: an agent runtime the tool plugs into, e.g. Claude Code; named by the tool (`claude`)
- SCOPE: where a harness's files live: `project` or `user`
- PROFILE: the parts of one harness at one scope
- PART: one piece of an integration, named by the tool (`instructions`, `hooks`, …), of one kind: file, region, merge or external
- ROOT: the directory a profile's paths are relative to
- RECORD: a TOML file holding, per harness, the hash of each part's content as the tool last wrote it
- DECLINED PARTS: the parts a user chose not to install (`--without`)
- REFUSAL: an error returned before any write
- MARKERS: `<!-- <tool>:harness -->` and `<!-- /<tool>:harness -->`, each on a line of its own

## Stakeholders

- TOOL AUTHOR: builds a CLI that installs into harnesses and answers their hooks
- AGENT USER: runs the tool's install and keeps their own edits in the same files

## Requirements

### KIT-1: Tool and profiles [MUST]

AS A tool author, I WANT to declare my parts as data per harness and scope, SO THAT the kit installs them without knowing my content.

ACCEPTANCE CRITERIA

- [ ] KIT-1_AC-1 [ubiquitous]: The system SHALL take from the tool its name, a profile per harness and scope, the root, the record path and the declined parts store of each scope, and SHALL embed no content of its own
- [ ] KIT-1_AC-2 [conditional]: IF the tool has no profile for the harness and scope THEN `install` and `status` SHALL refuse with an unknown-profile error

### KIT-2: Part kinds [MUST]

AS A tool author, I WANT four kinds of part, SO THAT I can own whole files, a block in the user's file, entries in a JSON file, or something only my tool can read.

ACCEPTANCE CRITERIA

- [ ] KIT-2_AC-1 [ubiquitous]: A file part SHALL own whole files under a directory, written with LF line endings
- [ ] KIT-2_AC-2 [ubiquitous]: A region part SHALL own the block between the tool's markers in one file, either a fixed path or a path a rule chooses under the root at install time
- [ ] KIT-2_AC-3 [ubiquitous]: A merge part SHALL own entries in the JSON object of one file: an array entry (found by equality), an object member (found by its key), or the tool's entries inside the groups of an array (found by a field that starts with, or contains, a text the tool chooses)
- [ ] KIT-2_AC-4 [ubiquitous]: An external part SHALL be read and written by the tool itself through the kit's interface, with a location shown in reports

### KIT-3: Part states [MUST]

AS AN agent user, I WANT each part's state told apart, SO THAT the tool never overwrites my edits by surprise.

ACCEPTANCE CRITERIA

- [ ] KIT-3_AC-1 [ubiquitous]: The system SHALL give each part exactly one state: skipped (declined), absent (nothing the tool could own), current (equal to the tool's content), stale (differs, and equals the recorded hash), edited (differs from both, or present with no recorded hash)
- [ ] KIT-3_AC-2 [ubiquitous]: The system SHALL compare files and external text ignoring CRLF/LF differences, a region block by its words, and merge entries by JSON equality

### KIT-4: Install [MUST]

AS AN agent user, I WANT install to create, update or leave each part by its state, SO THAT reinstalling is safe.

ACCEPTANCE CRITERIA

- [ ] KIT-4_AC-1 [event]: WHEN `install` runs THEN the system SHALL write absent and stale parts, leave current and skipped parts, and leave edited parts unless forced
- [ ] KIT-4_AC-2 [event]: WHEN `install` writes or finds current a part THEN the system SHALL record its content hash; a skipped part's hash SHALL be removed; an edited part left as found SHALL keep its recorded hash
- [ ] KIT-4_AC-3 [ubiquitous]: The result SHALL give, per part in profile order, its name, its path relative to the root (an external part's location), the state found, and what install did: created (absent before), rewrote (a file part that existed), updated (a region or merge that existed), or nothing
- [ ] KIT-4_AC-4 [ubiquitous]: The text form SHALL be one line per part, `<word> <path> (<part>)`, the word (the action, else the state) padded to seven columns; the JSON form SHALL carry the same parts

DEPENDS ON: KIT-3

### KIT-5: Status [MUST]

AS AN agent user, I WANT the state of every part without changes, SO THAT I can see what install would do.

ACCEPTANCE CRITERIA

- [ ] KIT-5_AC-1 [event]: WHEN `status` runs THEN the system SHALL report each part's state as `install` would find it and SHALL write nothing

### KIT-6: Refusals write nothing [MUST]

AS AN agent user, I WANT a failed install to leave every file as found, SO THAT nothing is half-installed.

ACCEPTANCE CRITERIA

- [ ] KIT-6_AC-1 [conditional]: IF the profile is unknown, `--without` names a part the profile lacks, or a file the kit must read (a merge target, the record, the declined store) does not parse or has the wrong shape THEN `install` SHALL refuse before any write, naming the file
- [ ] KIT-6_AC-2 [ubiquitous]: The system SHALL write external parts before any file, so a failing external part leaves every file as found

### KIT-7: Writes [MUST]

AS AN agent user, I WANT the tool's writes to be minimal and safe, SO THAT my files and their history stay clean.

ACCEPTANCE CRITERIA

- [ ] KIT-7_AC-1 [ubiquitous]: The system SHALL write a file only when its text changes, atomically, after planning every write; several parts in one file SHALL produce one write
- [ ] KIT-7_AC-2 [ubiquitous]: The record SHALL be the tool's header line, then one table per harness in name order with part hashes in name order, LF; a record that does not change SHALL not be written unless the file is missing

### KIT-8: Declined parts [MUST]

AS AN agent user, I WANT to decline a part once, SO THAT later installs keep skipping it.

ACCEPTANCE CRITERIA

- [ ] KIT-8_AC-1 [event]: WHEN `install` is given `--without` THEN the system SHALL use that list as the declined parts and store it after the parts are written; otherwise the stored list SHALL apply
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
- [ ] KIT-10_AC-3 [ubiquitous]: For a group entry the tool SHALL own only its entries inside a group: the user's entries and other keys in the same group SHALL be kept and not count as an edit; the tool's entries SHALL replace its earlier ones in the first group holding one, else come as a new group

### KIT-11: Claude Code [MUST]

AS A tool author, I WANT Claude Code's formats ready-made, SO THAT my hooks and parts match what Claude Code reads.

ACCEPTANCE CRITERIA

- [ ] KIT-11_AC-1 [ubiquitous]: The instructions file SHALL be chosen under the root as: no `AGENTS.md` → `CLAUDE.md`; only `AGENTS.md` → it; both, with a line `@AGENTS.md` in `CLAUDE.md` → `AGENTS.md`; both without it → `CLAUDE.md`
- [ ] KIT-11_AC-2 [ubiquitous]: Hook input SHALL parse with every known field optional and unknown fields ignored; input that is not JSON SHALL be an error the caller handles
- [ ] KIT-11_AC-3 [ubiquitous]: A hook answer SHALL be `{}` (allow, optionally with text for stderr), `{"decision":"block","reason":…}`, or `{"hookSpecificOutput":{"hookEventName":…,"additionalContext":…}}`
- [ ] KIT-11_AC-4 [event]: WHEN a hook answer is emitted THEN the system SHALL write its stderr text, then the JSON and a newline on stdout, and give exit 0; WHEN the hook failed THEN it SHALL write `error: <message>` on stderr, nothing on stdout, and give exit 1
- [ ] KIT-11_AC-5 [ubiquitous]: A hook command part SHALL be the group `{"hooks":[{"type":"command","command":…}]}` under `hooks.<event>` of a settings file, the tool's hooks being those whose command matches the tool's rule (a prefix, or a text it contains)

DEPENDS ON: KIT-2, KIT-10

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

### KIT-16: Errors [MUST]

AS A tool author, I WANT typed errors, SO THAT I can react to each kind of refusal.

ACCEPTANCE CRITERIA

- [ ] KIT-16_AC-1 [ubiquitous]: Each refusal SHALL be its own error kind: unknown scope, unknown profile, unknown part, a file that does not parse or has the wrong shape, and a refusal from a tool's own external part or store; IO failures SHALL name the path; each SHALL display a one-line message

## Assumptions

- Claude Code's hook JSON and settings format behave as documented (confirmed for smllm, 2026-09-25)

## Constraints

- `harness` knows no harness's formats; harness formats live in their own modules (`claude`)
- No public API exposes a type from another crate's 0.x release
- MSRV 1.85

## Out of Scope

- Removing (uninstalling) parts
- Harnesses other than Claude Code (a later module)
- Running hooks or commands for the tool

## Change Log

- 0.1.0 (2026-10-06): Initial requirements (PLAN-009); entry matching by contained text and exact region round trip, from the sokf port (D9-20)
