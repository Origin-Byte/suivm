mod cmd;

use clap::Parser;

#[derive(Parser)]
#[command(name = "suivm")]
#[command(bin_name = "suivm")]
enum Suivm {
    Latest,
    List,
    Remove(Remove),
    Switch(Switch),
    ListLocal,
}

/// Remove from locally installed versions
#[derive(clap::Args)]
struct Remove {
    version: String,
}

/// Use given version, install if not yet
#[derive(clap::Args)]
struct Switch {
    version: String,
}

fn main() {
    match Suivm::parse() {
        Suivm::Latest => cmd::latest::handle(),
        Suivm::List => cmd::list::handle(),
        Suivm::Remove(cmd) => cmd::remove::handle(cmd.version),
        Suivm::Switch(cmd) => cmd::switch::handle(cmd.version),
        Suivm::ListLocal => cmd::list_local::handle(),
    }
}
