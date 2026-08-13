//! Which fabricator's table a command is about to use, and where it came from.
//!
//! Every command that checks, routes, scores or exports a board needs a fab
//! preset, and every one of them took it from a `--preset` flag whose default
//! was `jlcpcb`. A default in the flag means the command cannot tell a caller
//! who asked for JLCPCB from one who asked for nothing, so a board written for
//! another house was silently measured against the wrong table unless whoever
//! ran it remembered to say.
//!
//! `board b { fab oshpark }` puts the answer in the design, where the rest of
//! the board's facts live. The flag still wins when it is given - a caller
//! asking a specific question about a specific fab is not overridden by the
//! file - and JLCPCB remains the answer when neither says anything, which is
//! what it has always been.

use cypcb_rules::presets::RulesPreset;
use cypcb_world::BoardWorld;
use miette::Result;

/// Resolve the preset for a board: the flag, then the design, then the default.
///
/// `flag` is what `--preset` carried, or `None` when it was not given. An
/// unknown name is an error either way, and the message says where the name
/// came from - a typo in a file and a typo on the command line are fixed in
/// different places.
pub fn resolve(flag: Option<&str>, world: &BoardWorld) -> Result<RulesPreset> {
    if let Some(name) = flag {
        return by_name(name, Origin::Flag);
    }
    if let Some(name) = world.fab() {
        return by_name(name, Origin::Design);
    }
    Ok(RulesPreset::JlcpcbStandard2Layer)
}

/// Where a preset name was written.
#[derive(Clone, Copy)]
enum Origin {
    Flag,
    Design,
}

fn by_name(name: &str, origin: Origin) -> Result<RulesPreset> {
    RulesPreset::from_name(name).ok_or_else(|| {
        // Listed from the presets themselves, so the message cannot go stale
        // when one is added.
        let available: Vec<&str> = RulesPreset::all().iter().map(|p| p.name()).collect();
        match origin {
            Origin::Flag => miette::miette!(
                "Unknown preset '{}'. Available presets: {}",
                name,
                available.join(", ")
            ),
            Origin::Design => miette::miette!(
                "The board asks for fab '{}', which is not a preset this tool has. \
                 Available presets: {}",
                name,
                available.join(", ")
            ),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_world::components::Fab;

    /// A board that names a fab, or one that says nothing.
    fn board(fab: Option<&str>) -> BoardWorld {
        let mut world = BoardWorld::new();
        world.set_board(
            "b".to_string(),
            (cypcb_core::Nm::from_mm(20.0), cypcb_core::Nm::from_mm(20.0)),
            2,
        );
        if let Some(fab) = fab {
            world.set_fab(Fab(fab.to_string()));
        }
        world
    }

    #[test]
    fn the_design_is_read_when_the_flag_is_absent() {
        let preset = resolve(None, &board(Some("oshpark"))).expect("oshpark is a preset");
        assert_eq!(preset, RulesPreset::from_name("oshpark").unwrap());
    }

    #[test]
    fn the_flag_wins_over_the_design() {
        let preset = resolve(Some("pcbway"), &board(Some("oshpark"))).expect("pcbway is a preset");
        assert_eq!(preset, RulesPreset::from_name("pcbway").unwrap());
    }

    #[test]
    fn saying_nothing_anywhere_is_still_jlcpcb() {
        let preset = resolve(None, &board(None)).expect("the default resolves");
        assert_eq!(preset, RulesPreset::JlcpcbStandard2Layer);
    }

    /// A typo in a file and a typo on the command line are fixed in different
    /// places, so the message has to say which one happened.
    #[test]
    fn an_unknown_name_says_where_it_was_written() {
        let from_design = resolve(None, &board(Some("jlpcb")))
            .expect_err("jlpcb is not a preset")
            .to_string();
        assert!(
            from_design.contains("The board asks for fab 'jlpcb'"),
            "{from_design}"
        );

        let from_flag = resolve(Some("jlpcb"), &board(None))
            .expect_err("jlpcb is not a preset")
            .to_string();
        assert!(from_flag.contains("Unknown preset 'jlpcb'"), "{from_flag}");
    }
}
