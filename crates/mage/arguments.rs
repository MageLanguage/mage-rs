use clap::{Parser, ValueEnum};

/// Internal output mode used after CLI parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Text,
    Json,
}

/// CLI-facing output mode accepted by clap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliOutput {
    Text,
    Json,
}

impl From<CliOutput> for Output {
    fn from(output: CliOutput) -> Self {
        match output {
            CliOutput::Text => Output::Text,
            CliOutput::Json => Output::Json,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliStage {
    Load,
    Flatten,
    Compile,
    Execute,
}

impl From<CliStage> for mage_contract::Stage {
    fn from(stage: CliStage) -> Self {
        match stage {
            CliStage::Load => mage_contract::Stage::Load,
            CliStage::Flatten => mage_contract::Stage::Flatten,
            CliStage::Compile => mage_contract::Stage::Compile,
            CliStage::Execute => mage_contract::Stage::Execute,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliFormat {
    Pretty,
    Simple,
}

impl From<CliFormat> for serde_text::Format {
    fn from(format: CliFormat) -> Self {
        match format {
            CliFormat::Pretty => serde_text::Format::Pretty,
            CliFormat::Simple => serde_text::Format::Simple,
        }
    }
}

#[derive(Debug, Clone, Parser)]
pub struct Arguments {
    pub path: String,
    #[arg(long, default_value = "execute")]
    pub stage: CliStage,
    #[arg(long, default_value = "text")]
    pub output: CliOutput,
    #[arg(long, default_value = "pretty")]
    pub format: CliFormat,
}

/// Parsed CLI arguments converted into internal runtime types.
pub struct ParsedArguments {
    pub path: String,
    pub stage: mage_contract::Stage,
    pub output: Output,
    pub format: serde_text::Format,
}

pub fn parse() -> ParsedArguments {
    let arguments = Arguments::parse();
    ParsedArguments {
        path: arguments.path,
        stage: arguments.stage.into(),
        output: arguments.output.into(),
        format: arguments.format.into(),
    }
}
