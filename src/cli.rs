use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Clone, ValueEnum)]
pub enum Output {
    Text,
    Json,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum Stage {
    Flatten,
    Compile,
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
    Run(Run),
    /// Print environment
    Environment,
    /// Run language server
    LanguageServer,
}

#[derive(Debug, Clone, Args)]
pub struct Run {
    /// path
    #[arg(action)]
    pub path: Option<String>,
    /// stage
    #[arg(long, default_value = "flatten")]
    pub stage: Stage,
}
