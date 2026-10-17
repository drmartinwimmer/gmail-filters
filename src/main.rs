use clap::Parser;
use gmail_triage::GmailTriageCommand;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "gmail-triage",
    about = "CLI tools and validators for agentic Gmail filter reviews and gmailctl configuration management",
    version
)]
struct Cli {
    #[command(flatten)]
    cmd: GmailTriageCommand,
}

impl Cli {
    fn run(self) -> ExitCode {
        match self.cmd.run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("Error: {err}");
                ExitCode::from(2)
            }
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    cli.run()
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::PathBuf;

    #[googletest::test]
    fn run_cli_existing_path_returns_success() {
        let cli = Cli {
            cmd: GmailTriageCommand::new(Some(PathBuf::from(".")), true),
        };
        assert_that!(cli.run(), eq(ExitCode::SUCCESS));
    }

    #[googletest::test]
    fn run_cli_nonexistent_path_returns_exit_code_2() {
        let cli = Cli {
            cmd: GmailTriageCommand::new(Some(PathBuf::from("nonexistent_dir_404")), true),
        };
        assert_that!(cli.run(), eq(ExitCode::from(2)));
    }
}
