//! The contract every harness implements, and the built-in harnesses.
// @zen-component: KIT-Adapter

use std::{
    fmt::Debug,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    Result,
    harness::{Part, PartResult, Scope},
    hook::{Answer, Event, HookInput, Output},
    integration::{Integration, Item},
};

/// An agent harness: where it reads a tool's integration and how it talks
/// to hooks. The kit's own harnesses are in [`builtin`]; a tool may add its
/// own, and [`install`](crate::harness::install), [`status`](crate::harness::status)
/// and the hook functions work with it unchanged.
// @zen-impl: KIT-18_AC-1
// @zen-impl: KIT-18_AC-3
pub trait Harness: Debug + Send + Sync {
    /// The harness's id, e.g. `claude`: the name users give, and `{harness}`
    /// in a hook command.
    fn id(&self) -> &str;

    /// The scopes it has files at.
    fn scopes(&self) -> &[Scope];

    /// The parts for `integration`'s items at `cx`, at most one per item,
    /// each named by its item ([`Item::as_str`]). An item the harness
    /// cannot take at the scope has no part.
    // @zen-impl: KIT-18_AC-4
    fn render(&self, integration: &Integration, cx: &Context) -> Result<Vec<Part>>;

    /// The locations it loads for `item` at `cx`: those it always loads
    /// (which must include where it renders the item) and those it may
    /// load.
    fn reads(&self, item: Item, cx: &Context) -> Result<Reads>;

    /// `text`, the input it sent a hook for `event`, as a [`HookInput`].
    fn parse_hook(&self, event: Event, text: &str) -> serde_json::Result<HookInput>;

    /// `answer` to `event` in its form; [`Error::Unsupported`](crate::Error::Unsupported)
    /// when it cannot express it.
    fn answer(&self, event: Event, answer: &Answer) -> Result<Output>;

    /// What the user still has to do after an install, from its parts'
    /// results, e.g. trust the folder.
    // @zen-impl: KIT-20_AC-1
    fn notes(&self, cx: &Context, parts: &[PartResult]) -> Vec<String> {
        let _ = (cx, parts);
        Vec::new()
    }
}

/// What a harness renders for and reads at: the tool, the scope and its
/// root, and the user's home.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Context {
    /// The tool's name.
    pub tool: String,
    /// The scope.
    pub scope: Scope,
    /// The scope's root: the project, or the home directory.
    pub root: PathBuf,
    /// The home directory, when known (a project setting may live there).
    pub user_root: Option<PathBuf>,
}

impl Context {
    /// A context.
    pub fn new(
        tool: impl Into<String>,
        scope: Scope,
        root: impl Into<PathBuf>,
        user_root: Option<PathBuf>,
    ) -> Self {
        Context {
            tool: tool.into(),
            scope,
            root: root.into(),
            user_root,
        }
    }

    /// `rel` (relative to the root, `/`-separated) as a path.
    pub fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    /// Whether `rel` is a file under the root.
    pub fn is_file(&self, rel: &str) -> bool {
        self.path(rel).is_file()
    }

    /// The home directory joined with `rel`, when the home is known.
    pub fn user_path(&self, rel: &str) -> Option<PathBuf> {
        self.user_root.as_deref().map(|h: &Path| h.join(rel))
    }
}

/// The locations a harness loads for an item, relative to the root.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Reads {
    /// Loaded whenever they exist.
    pub always: Vec<String>,
    /// Loaded in some setups (a setting, one of the harness's front ends).
    pub maybe: Vec<String>,
}

impl Reads {
    /// Always loads `locations`.
    pub fn always<I: IntoIterator<Item = S>, S: Into<String>>(locations: I) -> Self {
        Reads {
            always: locations.into_iter().map(Into::into).collect(),
            maybe: Vec::new(),
        }
    }

    /// Also may load `locations`.
    #[must_use]
    pub fn maybe<I: IntoIterator<Item = S>, S: Into<String>>(mut self, locations: I) -> Self {
        self.maybe.extend(locations.into_iter().map(Into::into));
        self
    }

    /// Whether `location` is always loaded.
    pub fn loads(&self, location: &str) -> bool {
        self.always.iter().any(|l| l == location)
    }

    /// Whether `location` is loaded, always or maybe.
    pub fn may_load(&self, location: &str) -> bool {
        self.loads(location) || self.maybe.iter().any(|l| l == location)
    }
}

/// Every built-in harness, in a stable order.
// @zen-impl: KIT-18_AC-2
pub fn builtin() -> Vec<Arc<dyn Harness>> {
    vec![Arc::new(crate::claude::Claude)]
}

/// The built-in harness with id `id`.
pub fn find(id: &str) -> Option<Arc<dyn Harness>> {
    builtin().into_iter().find(|h| h.id() == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_context() {
        let r = Reads::always(["AGENTS.md"]).maybe(["CLAUDE.md"]);
        assert!(r.loads("AGENTS.md") && !r.loads("CLAUDE.md"));
        assert!(r.may_load("CLAUDE.md") && !r.may_load("X"));
        let cx = Context::new("t", Scope::Project, "/p", Some("/h".into()));
        assert_eq!(cx.path("a/b"), Path::new("/p/a/b"));
        assert_eq!(cx.user_path(".x"), Some(PathBuf::from("/h/.x")));
        assert!(!cx.is_file("nope"));
        assert_eq!(
            Context::new("t", Scope::User, "/h", None).user_path("x"),
            None
        );
    }

    // @zen-test: KIT-18_AC-2
    #[test]
    fn built_in_harnesses_are_found_by_id() {
        let ids: Vec<_> = builtin().iter().map(|h| h.id().to_string()).collect();
        for id in &ids {
            assert_eq!(find(id).unwrap().id(), id);
        }
        assert!(find("nope").is_none());
    }
}
