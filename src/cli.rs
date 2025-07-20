use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Clone, ValueEnum)]
pub enum Output {
    Text,
    Json,
}

#[derive(Debug, Clone, Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
    /// output
    #[arg(long, default_value = "text")]
    pub output: Output,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Run
    #[command()]
    Run(Run),
    /// Print environment
    #[command()]
    Environment,
    /// Run language server
    #[command()]
    LanguageServer,
}

#[derive(Debug, Clone, Args)]
pub struct Run {
    /// path
    pub path: Option<String>,
}
