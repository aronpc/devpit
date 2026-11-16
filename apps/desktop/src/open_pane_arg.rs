//! The `--open-pane` a second launch carries: the pane it asks to be
//! brought forward on, read where the running copy hears the new one's
//! arguments.

/// The leaf `--open-pane` names, when it names one.
pub(crate) fn leaf_from(argv: &[String]) -> Option<&str> {
    // The handover's argv carries the binary's own argv[0].
    let mut args = argv.iter().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--open-pane" {
            // The leaf comes from outside the app: only a whole leaf is one.
            return args
                .next()
                .map(String::as_str)
                .filter(|value| is_leaf(value));
        }
    }
    None
}

/// `leaf_` and a ULID's 26 uppercase letters or digits, nothing around them.
fn is_leaf(value: &str) -> bool {
    value.strip_prefix("leaf_").is_some_and(|body| {
        body.len() == 26
            && body
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    })
}

/// What the `--open-pane` in `argv` opens: the project that holds the leaf,
/// and the leaf. `Ok(None)` when the arguments name no pane, or the leaf is
/// no project's; the holder's word for a store that would not answer.
pub(crate) fn opening<H>(argv: &[String], holder: H) -> Result<Option<(String, String)>, String>
where
    H: FnOnce(&str) -> Result<Option<String>, String>,
{
    let Some(leaf) = leaf_from(argv) else {
        return Ok(None);
    };
    holder(leaf).map(|held| held.map(|project| (project, leaf.to_owned())))
}

#[cfg(test)]
#[path = "open_pane_arg_tests.rs"]
mod tests;
