use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
pub struct CmdOptions {
    #[clap(subcommand)]
    pub command: AppCommand,
}

#[derive(Debug, Subcommand)]
pub enum AppCommand {
    Start {
        #[clap(
            long,
            help = "Specify the working directory, defaults to current directory"
        )]
        cwd: Option<String>,
    },
    Version,
}
