use clap::Parser;
use klef::cli::Cli;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => return handle_parse_error(&e),
    };
    match klef::run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(u8::try_from(e.exit_code()).unwrap_or(1))
        }
    }
}

fn handle_parse_error(e: &clap::Error) -> ExitCode {
    use clap::error::ErrorKind;

    // Detect the "user passed value as positional arg" case for `add` and `edit`.
    // Both subcommands take exactly one positional (`<NAME>`) — extra positionals here
    // almost always mean the user did `klef add stripe sk_live_xxxxx` thinking the
    // value goes on the command line.
    let raw = e.to_string();
    let triggered_subcmd = if raw.contains("klef add") {
        Some("add")
    } else if raw.contains("klef edit") {
        Some("edit")
    } else {
        None
    };

    if matches!(
        e.kind(),
        ErrorKind::UnknownArgument | ErrorKind::TooManyValues | ErrorKind::ArgumentConflict
    ) && let Some(sub) = triggered_subcmd
    {
        eprintln!(
            "error: 'klef {sub}' reads the secret value from a TTY prompt or stdin, \
             not as an argument."
        );
        eprintln!("hint:  klef {sub} <name>                # prompt");
        eprintln!("       echo -n value | klef {sub} <name>  # piped");
        return ExitCode::from(64); // sysexits.h EX_USAGE
    }

    // Default clap behavior for everything else (--help, --version, real errors).
    e.exit();
}
