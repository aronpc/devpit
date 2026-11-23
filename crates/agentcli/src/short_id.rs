//! The short id `--bg` prints, read out of its help block.

/// The short id out of what `--bg` printed.
///
/// The output is a small help block, not a bare id:
///
/// ```text
/// backgrounded · fa35a378
///   claude agents             list sessions
///   claude attach fa35a378    open in this terminal
/// ```
///
/// Read off the `attach` line rather than the first: that line exists to be
/// copied, so its second-to-last token is the handle by construction. A guess
/// at "the last bare word" matched nothing here and failed at attach time,
/// where the cause is no longer on screen.
pub(crate) fn short_id_in(stdout: &str) -> Option<String> {
    let from_attach = stdout.lines().find_map(|line| {
        // `<binary> attach <id>`, whatever the binary is called: a profile may
        // name another build of the same CLI, and it prints its own name here.
        let mut words = line.split_whitespace();
        let _binary = words.next()?;
        (words.next()? == "attach").then(|| words.next()).flatten()
    });
    from_attach
        .or_else(|| {
            // Or the announcement itself, when the help block is not printed.
            stdout
                .lines()
                .find(|line| line.starts_with("backgrounded"))
                .and_then(|line| line.split_whitespace().next_back())
        })
        .filter(|id| is_handle(id))
        .map(ToOwned::to_owned)
}

pub(crate) fn is_handle(token: &str) -> bool {
    !token.is_empty()
        && token.len() <= 64
        && token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}
