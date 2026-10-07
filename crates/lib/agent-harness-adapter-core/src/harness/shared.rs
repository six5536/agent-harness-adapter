//! The shared-location choice: per item, the fewest locations every
//! harness in the set loads, so shared content is written once.
// @zen-component: KIT-Shared

use std::collections::BTreeSet;

use crate::{
    Error, Result,
    harness::{Part, Reads},
};

/// One harness's part for an item.
#[derive(Debug, Clone)]
pub(crate) struct Candidate<'a> {
    /// The harness's id.
    pub(crate) harness: &'a str,
    /// Whether the user named the harness (else it is in the set through the
    /// record): a named harness writes a location before one that is not.
    pub(crate) named: bool,
    /// Its part for the item.
    pub(crate) part: &'a Part,
    /// Where it loads the item.
    pub(crate) reads: &'a Reads,
}

/// What a candidate does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Role {
    /// It writes its part.
    Write,
    /// Its content is in `location`, which `by` writes.
    Shared { location: String, by: String },
}

/// The choice for one item: a role per candidate (in input order) and the
/// double-load warnings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Choice {
    pub(crate) roles: Vec<Role>,
    pub(crate) warnings: Vec<String>,
}

/// At most this many distinct locations are searched for the smallest
/// cover; more (never the case for the built-in harnesses) keep every
/// harness's own location.
const MAX_SEARCH: usize = 16;

/// Choose the locations for `item` among `candidates`, given in the tool's
/// harness order. See DESIGN-KIT KIT-Shared.
// @zen-impl: KIT-19_AC-2
// @zen-impl: KIT-19_AC-3
// @zen-impl: KIT-19_AC-4
// @zen-impl: KIT-19_AC-5
// @zen-impl: KIT-19_AC-7
pub(crate) fn choose(item: &str, candidates: &[Candidate<'_>]) -> Result<Choice> {
    // Distinct locations in first-seen order; equal locations, equal parts.
    let mut locations: Vec<String> = Vec::new();
    for (i, c) in candidates.iter().enumerate() {
        let at = c.part.location();
        if let Some(first) = candidates[..i].iter().find(|o| o.part.location() == at) {
            if !first.part.same_content(c.part) {
                return Err(Error::Internal(format!(
                    "harnesses `{}` and `{}` write different {item} to {at}",
                    first.harness, c.harness
                )));
            }
        } else {
            locations.push(at);
        }
    }
    // A harness is covered by a location it always loads, and by its own.
    let covers = |c: &Candidate<'_>, l: &str| c.reads.loads(l) || c.part.location() == l;
    let may = |c: &Candidate<'_>, l: &str| c.reads.may_load(l) || c.part.location() == l;
    let n = locations.len();
    let chosen: Vec<usize> = if n > MAX_SEARCH {
        (0..n).collect()
    } else {
        let mut best: Option<(usize, usize, Vec<usize>)> = None;
        for mask in 1u32..(1 << n) {
            let set: Vec<usize> = (0..n).filter(|i| mask & (1 << i) != 0).collect();
            if !candidates
                .iter()
                .all(|c| set.iter().any(|&l| covers(c, &locations[l])))
            {
                continue;
            }
            let loads: usize = set
                .iter()
                .map(|&l| candidates.iter().filter(|c| may(c, &locations[l])).count())
                .sum();
            let better = match &best {
                None => true,
                Some((size, score, order)) => {
                    (set.len(), std::cmp::Reverse(loads), &set)
                        < (*size, std::cmp::Reverse(*score), order)
                }
            };
            if better {
                best = Some((set.len(), loads, set));
            }
        }
        best.map(|(_, _, s)| s).unwrap_or_default()
    };
    // Each chosen location's writer: the earliest named harness rendering
    // it, else the earliest.
    let writer = |l: &str| -> usize {
        let at = |c: &&Candidate<'_>| c.part.location() == l;
        candidates
            .iter()
            .position(|c| c.named && at(&c))
            .or_else(|| candidates.iter().position(|c| at(&c)))
            .expect("a chosen location has a candidate")
    };
    let mut roles = Vec::with_capacity(candidates.len());
    let mut warnings = Vec::new();
    for (i, c) in candidates.iter().enumerate() {
        let own = c.part.location();
        if chosen.iter().any(|&l| locations[l] == own) && writer(&own) == i {
            roles.push(Role::Write);
        } else {
            let l = chosen
                .iter()
                .map(|&l| &locations[l])
                .find(|l| covers(c, l))
                .expect("every candidate is covered");
            roles.push(Role::Shared {
                location: l.clone(),
                by: candidates[writer(l)].harness.to_string(),
            });
        }
        let loaded: BTreeSet<&str> = chosen
            .iter()
            .map(|&l| locations[l].as_str())
            .filter(|l| may(c, l))
            .collect();
        // Only for the harnesses named: the others are not this run's concern.
        if c.named && loaded.len() > 1 {
            let list: Vec<&str> = chosen
                .iter()
                .map(|&l| locations[l].as_str())
                .filter(|l| loaded.contains(l))
                .collect();
            warnings.push(format!(
                "{} may load the {item} twice: {}",
                c.harness,
                list.join(", ")
            ));
        }
    }
    Ok(Choice { roles, warnings })
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn region(file: &str) -> Part {
        Part::region("instructions", file, "b\n")
    }

    fn cand<'a>(harness: &'a str, part: &'a Part, reads: &'a Reads) -> Candidate<'a> {
        Candidate {
            harness,
            named: true,
            part,
            reads,
        }
    }

    // @zen-test: KIT-19_AC-2
    // @zen-test: KIT-19_AC-3
    #[test]
    fn two_harnesses_reading_agents_md_share_one_copy() {
        let agents = region("AGENTS.md");
        let r = Reads::always(["AGENTS.md"]);
        let c = choose(
            "instructions",
            &[cand("codex", &agents, &r), cand("pi", &agents, &r)],
        )
        .unwrap();
        assert_eq!(
            c.roles,
            [
                Role::Write,
                Role::Shared {
                    location: "AGENTS.md".into(),
                    by: "codex".into()
                }
            ]
        );
        assert!(c.warnings.is_empty());
    }

    // A harness that also loads another's location is covered by it.
    #[test]
    fn a_cross_read_covers_a_harness() {
        let claude_md = region("CLAUDE.md");
        let agents = region("AGENTS.md");
        let claude = Reads::always(["CLAUDE.md"]);
        let copilot = Reads::always(["AGENTS.md", "CLAUDE.md"]);
        let codex = Reads::always(["AGENTS.md"]);
        let c = choose(
            "instructions",
            &[
                cand("claude", &claude_md, &claude),
                cand("copilot", &agents, &copilot),
            ],
        )
        .unwrap();
        assert_eq!(c.roles[0], Role::Write);
        assert_eq!(
            c.roles[1],
            Role::Shared {
                location: "CLAUDE.md".into(),
                by: "claude".into()
            }
        );
        // With codex too, both files are needed and copilot loads both.
        // @zen-test: KIT-19_AC-4
        let c = choose(
            "instructions",
            &[
                cand("claude", &claude_md, &claude),
                cand("codex", &agents, &codex),
                cand("copilot", &agents, &copilot),
            ],
        )
        .unwrap();
        assert_eq!(c.roles[..2], [Role::Write, Role::Write]);
        assert_eq!(
            c.roles[2],
            Role::Shared {
                location: "CLAUDE.md".into(),
                by: "claude".into()
            }
        );
        assert_eq!(
            c.warnings,
            ["copilot may load the instructions twice: CLAUDE.md, AGENTS.md"]
        );
    }

    #[test]
    fn a_maybe_read_warns_but_does_not_cover() {
        let claude = Part::merge("hooks", ".claude/settings.json", vec![]);
        let cursor = Part::merge("hooks", ".cursor/hooks.json", vec![]);
        let rc = Reads::always([".claude/settings.json"]);
        let ru = Reads::always([".cursor/hooks.json"]).maybe([".claude/settings.json"]);
        let c = choose(
            "hooks",
            &[cand("claude", &claude, &rc), cand("cursor", &cursor, &ru)],
        )
        .unwrap();
        assert_eq!(c.roles, [Role::Write, Role::Write]);
        assert_eq!(
            c.warnings,
            ["cursor may load the hooks twice: .claude/settings.json, .cursor/hooks.json"]
        );
    }

    // @zen-test: KIT-19_AC-5
    #[test]
    fn different_parts_at_one_location_are_refused() {
        let a = Part::region("instructions", "AGENTS.md", "a\n");
        let b = Part::region("instructions", "AGENTS.md", "b\n");
        let r = Reads::always(["AGENTS.md"]);
        let e = choose("instructions", &[cand("codex", &a, &r), cand("pi", &b, &r)]).unwrap_err();
        assert!(matches!(e, Error::Internal(_)), "{e}");
    }

    #[test]
    fn harnesses_not_named_are_not_warned_about() {
        let claude = Part::merge("hooks", ".claude/settings.json", vec![]);
        let cursor = Part::merge("hooks", ".cursor/hooks.json", vec![]);
        let rc = Reads::always([".claude/settings.json"]);
        let ru = Reads::always([".cursor/hooks.json"]).maybe([".claude/settings.json"]);
        let mut unnamed = cand("cursor", &cursor, &ru);
        unnamed.named = false;
        let c = choose("hooks", &[cand("claude", &claude, &rc), unnamed]).unwrap();
        assert!(c.warnings.is_empty());
    }

    // A named harness writes a location before one only in the record.
    #[test]
    fn a_named_harness_writes() {
        let agents = region("AGENTS.md");
        let r = Reads::always(["AGENTS.md"]);
        let mut first = cand("codex", &agents, &r);
        first.named = false;
        let c = choose("instructions", &[first, cand("pi", &agents, &r)]).unwrap();
        assert_eq!(
            c.roles,
            [
                Role::Shared {
                    location: "AGENTS.md".into(),
                    by: "pi".into()
                },
                Role::Write
            ]
        );
    }

    #[test]
    fn too_many_locations_keep_their_own() {
        let parts: Vec<Part> = (0..=MAX_SEARCH)
            .map(|i| region(&format!("F{i}.md")))
            .collect();
        let reads: Vec<Reads> = (0..=MAX_SEARCH)
            .map(|i| Reads::always([format!("F{i}.md")]))
            .collect();
        let names: Vec<String> = (0..=MAX_SEARCH).map(|i| format!("h{i}")).collect();
        let cands: Vec<Candidate<'_>> = (0..=MAX_SEARCH)
            .map(|i| cand(&names[i], &parts[i], &reads[i]))
            .collect();
        let c = choose("instructions", &cands).unwrap();
        assert!(c.roles.iter().all(|r| *r == Role::Write));
    }

    const FILES: [&str; 4] = ["A", "B", "C", "D"];

    /// Per harness: its own location, and the other locations it always or
    /// maybe loads.
    fn arb_world() -> impl Strategy<Value = Vec<(usize, Vec<usize>, Vec<usize>)>> {
        prop::collection::vec(
            (
                0usize..4,
                prop::collection::vec(0usize..4, 0..3),
                prop::collection::vec(0usize..4, 0..2),
            ),
            1..6,
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        // @zen-test: KIT_P-9
        // @zen-test: KIT_P-10
        #[test]
        fn minimal_cover(world in arb_world()) {
            let names: Vec<String> = (0..world.len()).map(|i| format!("h{i}")).collect();
            let parts: Vec<Part> = world.iter().map(|(own, _, _)| region(FILES[*own])).collect();
            let reads: Vec<Reads> = world
                .iter()
                .map(|(own, always, maybe)| {
                    let mut a = vec![FILES[*own]];
                    a.extend(always.iter().map(|i| FILES[*i]));
                    Reads::always(a).maybe(maybe.iter().map(|i| FILES[*i]))
                })
                .collect();
            let cands: Vec<Candidate<'_>> = (0..world.len()).map(|i| cand(&names[i], &parts[i], &reads[i])).collect();
            let c = choose("instructions", &cands).unwrap();
            let chosen: BTreeSet<String> = c.roles.iter().zip(&cands).map(|(r, cd)| match r {
                Role::Write => cd.part.location(),
                Role::Shared { location, .. } => location.clone(),
            }).collect();
            // Every harness is covered, and each chosen location has one writer.
            for (r, cd) in c.roles.iter().zip(&cands) {
                if let Role::Shared { location, by } = r {
                    prop_assert!(cd.reads.loads(location));
                    let w = cands.iter().zip(&c.roles).find(|(o, _)| o.harness == by).unwrap();
                    prop_assert_eq!(w.1, &Role::Write);
                    prop_assert_eq!(&w.0.part.location(), location);
                }
            }
            for l in &chosen {
                let writers = cands.iter().zip(&c.roles).filter(|(cd, r)| **r == Role::Write && &cd.part.location() == l).count();
                prop_assert_eq!(writers, 1);
            }
            // No smaller set of locations covers every harness.
            let all: Vec<&str> = FILES.to_vec();
            for mask in 1u32..16 {
                let set: Vec<&str> = (0..4).filter(|i| mask & (1 << i) != 0).map(|i| all[i]).collect();
                if set.len() >= chosen.len() { continue; }
                let offered = set.iter().all(|l| cands.iter().any(|cd| cd.part.location() == *l));
                let covers = cands.iter().all(|cd| set.iter().any(|l| cd.reads.loads(l)));
                prop_assert!(!(offered && covers), "{set:?} is smaller than {chosen:?}");
            }
            // A warning exactly for a harness that may load two chosen locations.
            for cd in &cands {
                let n = chosen.iter().filter(|l| cd.reads.may_load(l)).count();
                let warned = c.warnings.iter().any(|w| w.starts_with(&format!("{} ", cd.harness)));
                prop_assert_eq!(warned, n > 1);
            }
        }
    }
}
