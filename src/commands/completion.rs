use std::io;

use clap::{Args, CommandFactory};
use clap_complete::{generate, Shell};

use crate::error::Result;

#[derive(Args, Debug, Clone)]
pub struct CompletionArgs {
    /// Target shell to generate completions for
    #[arg(value_enum)]
    pub shell: Shell,
}

pub fn generate_completion<C: CommandFactory, W: io::Write>(shell: Shell, buf: &mut W) {
    let mut cmd = C::command();
    generate(shell, &mut cmd, "rocky", buf);
}

pub fn run(args: CompletionArgs) -> Result<()> {
    let mut stdout = io::stdout();
    generate_completion::<super::Cli, _>(args.shell, &mut stdout);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_bash_completion() {
        let mut buf = Vec::new();
        generate_completion::<super::super::Cli, _>(Shell::Bash, &mut buf);
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("complete -F _rocky"));
    }

    #[test]
    fn test_generate_zsh_completion() {
        let mut buf = Vec::new();
        generate_completion::<super::super::Cli, _>(Shell::Zsh, &mut buf);
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("#compdef rocky"));
    }

    #[test]
    fn test_generate_fish_completion() {
        let mut buf = Vec::new();
        generate_completion::<super::super::Cli, _>(Shell::Fish, &mut buf);
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("complete -c rocky"));
    }
}
