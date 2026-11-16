//! The terminal's half of `devpit`: the keys read, the menu redrawn, and each
//! item done. What is drawn is decided in `terminal`.

use std::io::IsTerminal as _;

use crate::terminal::{self, Item, Look, Start};

/// The exit code, when these arguments are for the terminal; `None` opens
/// the app. `argv` is the process's whole argv, argv[0] and all.
pub(crate) fn from_args(argv: &[String]) -> Option<i32> {
    let (foreground, own) = groups();
    let terminal = terminal::owns_terminal(
        std::io::stdin().is_terminal(),
        std::io::stdout().is_terminal(),
        foreground,
        own,
    );
    match terminal::start(
        argv.get(1..).unwrap_or_default(),
        terminal,
        cfg!(debug_assertions),
    ) {
        Start::Window => None,
        Start::Menu => Some(menu()),
        Start::Run(item) => Some(run(item, &Look::PLAIN, "")),
        Start::NoTerminal => {
            eprintln!(
                "devpit: the menu needs a terminal; run `devpit open`, `status`, `pair`, `update` or `diagnostics`"
            );
            Some(2)
        }
    }
}

/// The terminal's foreground process group, if it has one, and this process's.
#[cfg(unix)]
fn groups() -> (Option<i32>, i32) {
    // SAFETY: both only read process groups.
    let foreground = unsafe { libc::tcgetpgrp(libc::STDIN_FILENO) };
    let own = unsafe { libc::getpgrp() };
    ((foreground >= 0).then_some(foreground), own)
}

#[cfg(not(unix))]
fn groups() -> (Option<i32>, i32) {
    (None, 0)
}

/// One item done, its output under it moved right by `pad`; the exit code.
fn run(item: Item, look: &Look, pad: &str) -> i32 {
    let say = |text: &str| print!("{}", terminal::indented(text, pad));
    let Ok(root) = devpit_core::Store::root() else {
        eprintln!("devpit: cannot find where devpit keeps its state");
        return 1;
    };
    match item {
        Item::Open => open(),
        Item::Status => {
            say(&terminal::status_lines(&crate::checkup::gather(&root)));
            0
        }
        Item::Diagnostics => {
            let checks = crate::checkup::checks(&crate::checkup::gather(&root), &root);
            say(&terminal::check_lines(look, &checks));
            i32::from(checks.iter().any(|check| !check.ok))
        }
        Item::Pair => pair(&root, pad),
        Item::Web => {
            say(&format!("Web version: {}\n", terminal::WEB_LATER));
            0
        }
        Item::Update => update(),
        Item::Quit => 0,
    }
}

/// The app, started apart from this terminal; one already open is brought
/// forward by its single-instance handover.
fn open() -> i32 {
    let Some(exe) = crate::agent_reach::exe() else {
        eprintln!("devpit: cannot find its own binary");
        return 1;
    };
    let mut command = std::process::Command::new(exe);
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    // Its own group, so closing this terminal does not close it.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    match command.spawn() {
        Ok(_) => {
            println!("devpit: opening");
            0
        }
        Err(err) => {
            eprintln!("devpit: could not open: {err}");
            1
        }
    }
}

fn pair(root: &std::path::Path, pad: &str) -> i32 {
    let answer = devpit_agentapi::client::ask(root, "pair", serde_json::json!({}), root);
    match answer {
        Ok(pairing) => {
            let text = |key: &str| pairing[key].as_str().unwrap_or_default().to_owned();
            let left = pairing["expiresAt"].as_f64().unwrap_or_default() - crate::remote::now();
            let minutes = (left / 60.0).round().max(1.0) as u64;
            let qr = std::io::stdout().is_terminal();
            let lines = terminal::pairing_lines(&text("code"), &text("url"), minutes, qr);
            print!("{}", terminal::indented(&lines, pad));
            0
        }
        Err(why) => {
            eprintln!("{pad}devpit: {}", terminal::told(&why));
            1
        }
    }
}

#[cfg(unix)]
fn update() -> i32 {
    match std::process::Command::new("sh")
        .args(["-c", terminal::INSTALLER])
        .status()
    {
        Ok(status) => status.code().unwrap_or(1),
        Err(err) => {
            eprintln!("devpit: could not run the installer: {err}");
            1
        }
    }
}

#[cfg(not(unix))]
fn update() -> i32 {
    eprintln!("devpit: on Windows devpit updates itself from the app");
    1
}

#[cfg(unix)]
fn menu() -> i32 {
    let look = Look::of(|name| std::env::var(name).ok(), columns());
    print!("{}", terminal::banner(&look));
    let mut selected = 0;
    loop {
        let drawn = terminal::menu(&look, selected);
        let Some(chunk) = raw::key_read(&drawn) else {
            return 1;
        };
        let (now, picked) = terminal::pressed(selected, terminal::key(&chunk));
        // The menu just drawn makes room for what comes next.
        print!("\x1b[{}A\x1b[J", drawn.lines().count());
        selected = now;
        let Some(item) = picked else { continue };
        if item == Item::Quit {
            return 0;
        }
        println!("  {}\n", look.dim(terminal::ITEMS[selected].1));
        let code = run(item, &look, "  ");
        // Open and Update hand over to the app and the installer.
        if matches!(item, Item::Open | Item::Update) {
            return code;
        }
        println!();
    }
}

#[cfg(not(unix))]
fn menu() -> i32 {
    run(Item::Open, &Look::PLAIN, "")
}

#[cfg(unix)]
fn columns() -> usize {
    // SAFETY: a struct the call fills in, on stdout, which is a terminal.
    let mut size: libc::winsize = unsafe { std::mem::zeroed() };
    if unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut size) } == 0
        && size.ws_col > 0
    {
        usize::from(size.ws_col)
    } else {
        80
    }
}

/// The terminal in raw mode for exactly one key, and back as it was.
#[cfg(unix)]
mod raw {
    use std::io::{Read as _, Write as _};

    struct Raw(libc::termios);

    impl Raw {
        fn on() -> Option<Raw> {
            // SAFETY: a struct tcgetattr fills in, for stdin, a terminal.
            let mut was: libc::termios = unsafe { std::mem::zeroed() };
            if unsafe { libc::tcgetattr(libc::STDIN_FILENO, &mut was) } != 0 {
                return None;
            }
            let mut raw = was;
            // Ctrl-C arrives as a key, so the terminal is put back first.
            raw.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG);
            raw.c_cc[libc::VMIN] = 1;
            raw.c_cc[libc::VTIME] = 0;
            if unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw) } != 0 {
                return None;
            }
            Some(Raw(was))
        }
    }

    impl Drop for Raw {
        fn drop(&mut self) {
            // SAFETY: the settings read before, put back.
            unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &self.0) };
            print!("\x1b[?25h");
            let _ = std::io::stdout().flush();
        }
    }

    /// `drawn` shown with the cursor hidden, and the next key's bytes.
    pub(super) fn key_read(drawn: &str) -> Option<Vec<u8>> {
        let _raw = Raw::on()?;
        print!("\x1b[?25l{drawn}");
        std::io::stdout().flush().ok()?;
        let mut bytes = [0u8; 8];
        let read = std::io::stdin().read(&mut bytes).ok()?;
        (read > 0).then(|| bytes[..read].to_vec())
    }
}
