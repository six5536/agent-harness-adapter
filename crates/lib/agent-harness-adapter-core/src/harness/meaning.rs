//! The comparison of a part's content by its meaning, through `same-same`:
//! one comparer for the process, the fingerprints that the record keeps, the
//! plain hashes of the record and the loop guard, and what a recorded value
//! says of a fingerprint found. No type of `same-same` leaves the crate.

use std::{fmt::Write as _, sync::OnceLock};

use same_same::{
    Cache, Comparer, Detail, Fingerprint, Format, Input, Method, Named, NamedSet, Verdict,
    WhyUnknown,
};

// sokf:begin formats
// sokf-implements: KIT-3_AC-2
// sokf-implements: KIT-3_AC-3
// sokf-implements: KIT-3_AC-5
/// The formats that the adapter compares by meaning: every format of its
/// parts, and the scripts of JavaScript and TypeScript that a skill can hold.
const FORMATS: [Format; 10] = [
    Format::Markdown,
    Format::Json,
    Format::Jsonc,
    Format::Toml,
    Format::Yaml,
    Format::JavaScript,
    Format::Jsx,
    Format::TypeScript,
    Format::Tsx,
    Format::TypeScriptDeclaration,
];

// sokf-implements: KIT-3_AC-4
/// The formats in which a Markdown fence is read, by its tag; a fence tagged
/// `json` reads as JSON with comments.
const FENCES: [Format; 8] = [
    Format::Markdown,
    Format::Jsonc,
    Format::Toml,
    Format::Yaml,
    Format::JavaScript,
    Format::Jsx,
    Format::TypeScript,
    Format::Tsx,
];
// sokf:end formats

/// The entries of the comparer's cache in memory: far more texts than a
/// run of the adapter reads, so that a run reads each text once, and a
/// long-lived process, such as a binding's, holds two maps of 2,048 slots of
/// about 112 bytes, under half a megabyte.
const CACHED: usize = 2048;

/// The comparer of the process, built once with [`FORMATS`] and [`FENCES`].
fn comparer() -> &'static Comparer {
    static COMPARER: OnceLock<Comparer> = OnceLock::new();
    COMPARER.get_or_init(|| {
        Comparer::builder()
            .formats(FORMATS)
            .fences(FENCES)
            .cache(Cache::memory_of(CACHED))
            .build()
            .expect("the crate's features compile every format that it names")
    })
}

fn fingerprint(input: &Input<'_>) -> Fingerprint {
    comparer().fingerprint(input)
}

/// The fingerprint of a file's text, in the format of its name, the last
/// component of `path`.
// sokf-implements: KIT-3_AC-2
pub(crate) fn file(path: &str, text: &str) -> Fingerprint {
    fingerprint(&Input::bytes(text.as_bytes(), name(path)))
}

/// The name that chooses a format: the last component of `path`.
fn name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The fingerprint of a region's block, in the format of its file's name,
/// never as a range of the whole file, whose `region-indent` the adapter
/// doesn't read (D-90 of issue 021),
/// as a part whose indentation is its own: with no front matter, so that a
/// leading `---` is a thematic break, and every line as written, so that the
/// block's indentation counts, as it does at the top level of the file.
// sokf-implements: KIT-3_AC-6
pub(crate) fn block(file: &str, block: &str) -> Fingerprint {
    fingerprint(&Input::bytes(block.as_bytes(), name(file)).fragment_as_written())
}

/// The fingerprint of an external part's text, of no known format, so by
/// its bytes.
pub(crate) fn text(text: &str) -> Fingerprint {
    fingerprint(&Input::bytes(text.as_bytes(), ""))
}

/// Whether `plain`, written as a plain scalar of YAML, reads back as the
/// same text as `quoted`, its double-quoted form, under YAML 1.2 and for a
/// YAML 1.1 reader: the library compares the two one-pair texts, and only
/// `Same` by meaning is the same text, so a value that either reader takes
/// as something other than text, such as `no`, `~`, or `010`, is not.
// sokf-implements: KIT-17_AC-11
pub(crate) fn reads_as_text(plain: &str, quoted: &str) -> bool {
    // A comparer of its own, with no cache, so that the values asked about
    // take no entry of the parts' cache.
    static YAML: OnceLock<Comparer> = OnceLock::new();
    let a = format!("v: {plain}\n");
    let b = format!("v: {quoted}\n");
    let yaml = YAML.get_or_init(|| {
        Comparer::builder()
            .formats([Format::Yaml])
            .cache(Cache::none())
            .build()
            .expect("the crate's features compile YAML")
    });
    let answer = yaml.compare(
        &Input::bytes(a.as_bytes(), "front.yaml"),
        &Input::bytes(b.as_bytes(), "front.yaml"),
        Detail::Verdict,
    );
    answer.verdict == Verdict::Same && matches!(answer.method, Method::Meaning { .. })
}

/// The plain hash of `bytes`: the library's fingerprint of the bytes.
pub(crate) fn plain(bytes: &[u8]) -> Fingerprint {
    Fingerprint::of_bytes(bytes)
}

/// The 32 hexadecimal digits of `bytes`' plain hash with nothing folded,
/// whose value covers where each CRLF stands and a byte order mark as well,
/// so that every byte counts.
pub(crate) fn digits(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(32);
    for byte in plain(bytes).folded(NamedSet::new()).value() {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Whether a verdict is the same for the adapter: every named thing is the
/// same but how a YAML 1.1 reader takes a value.
// sokf-implements: KIT-3_AC-7
// sokf-implements: KIT-3_AC-8
fn is_same(verdict: &Verdict) -> bool {
    match verdict {
        Verdict::Same => true,
        Verdict::SameApartFrom(named) => !named.contains(Named::Yaml11Value),
        _ => false,
    }
}

/// Whether two fingerprints of the same rules mean the same for the adapter.
pub(crate) fn same(a: &Fingerprint, b: &Fingerprint) -> bool {
    is_same(&a.compare(b))
}

/// Why a recorded value can't tell an edit from what the tool wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Unusable {
    /// The value isn't of the record's form, such as a hash of 0.4.0.
    Old,
    /// The value is of the record's form, but of another shape than the
    /// part's: a table for one content, or one value for a file part.
    Shape,
    /// The fingerprint holds named things of a later release of the library
    /// than this one.
    Later,
    /// The recorded fingerprint and the one found come from other rules,
    /// or methods, of the library, and their values differ.
    Rules,
}

impl Unusable {
    /// The warning for `harness`'s part `part` in `path`, which names the
    /// forced install that writes the tool's version.
    // sokf-implements: KIT-3_AC-10
    /// With `uninstall`, the forced uninstall, which removes the part.
    pub(crate) fn warning(&self, harness: &str, part: &str, path: &str, uninstall: bool) -> String {
        let why = match self {
            Unusable::Old => "its record is from an older version",
            Unusable::Later => "its record is from a later version",
            Unusable::Rules => "its record was made under other rules",
            Unusable::Shape => "its record doesn't fit the part",
        };
        let remedy = if uninstall {
            format!("a forced uninstall of {harness} removes it")
        } else {
            format!("install --force {part} writes the tool's version")
        };
        format!("{harness}: {part} in {path} may not be an edit, as {why}; {remedy}")
    }
}

/// What a recorded value says of the content found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Vouch {
    /// The record holds the content found: the tool wrote it.
    Holds,
    /// The content found differs from the record: an edit.
    Differs,
    /// The record can't tell.
    Unusable(Unusable),
}

/// What the recorded value `recorded` says of the fingerprint `found`,
/// with `wanted` the fingerprint of the tool's content where there is one:
/// a text found by another method than the record's, while the tool's
/// content still reads by the record's, is an edit, not a change of rules.
// sokf-implements: KIT-3_AC-10
pub(crate) fn vouch(recorded: &str, found: &Fingerprint, wanted: Option<&Fingerprint>) -> Vouch {
    let Ok(recorded) = recorded.parse::<Fingerprint>() else {
        return Vouch::Unusable(Unusable::Old);
    };
    if is_same(&recorded.compare(found)) {
        return Vouch::Holds;
    }
    match recorded.why_unknown(found) {
        // A difference, also of two texts by their bytes.
        None | Some(WhyUnknown::Bytes) => Vouch::Differs,
        Some(WhyUnknown::LaterNamed) => Vouch::Unusable(Unusable::Later),
        // The tool's content still reads by the record's method: only the
        // text found changed its reader, an edit.
        Some(_) if wanted.is_some_and(|w| w.method() == recorded.method()) => Vouch::Differs,
        Some(_) => Vouch::Unusable(Unusable::Rules),
    }
}

#[cfg(test)]
pub(crate) mod tests;
