//! A turn's process group: started whole, and ended whole.

/// Ends the CLI and the tools it started, then lets the caller reap it.
pub(crate) fn end_group(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        // The turn's CLI leads its own group (`own_group`), so its pid is the group's.
        let _ = std::process::Command::new("kill")
            .args(["-KILL", "--", &format!("-{}", child.id())])
            .status();
    }
    let _ = child.kill();
}

/// A turn's CLI leads a process group of its own: a forced stop ends the
/// tools it started with it, which signalling the CLI alone would leave running.
pub(crate) fn own_group(command: &mut std::process::Command) -> &mut std::process::Command {
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(command, 0);
    command
}
