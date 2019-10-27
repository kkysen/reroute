use std::env;
use std::io;
use std::path::PathBuf;

use structopt::StructOpt;
use reroute::ReRoute;

#[derive(Debug, StructOpt)]
#[structopt(
    name = env!("CARGO_PKG_NAME"),
    version = env!("CARGO_PKG_VERSION"),
    author = env!("CARGO_PKG_AUTHORS"),
    about = env!("CARGO_PKG_DESCRIPTION"),
)]
struct Args {
    to: Option<PathBuf>,
    from: Option<PathBuf>,
}

impl From<Args> for ReRoute {
    fn from(args: Args) -> Self {
        ReRoute::default(args.from, args.to)
    }
}

#[paw::main]
fn main(args: Args) -> io::Result<()> {
    let router = ReRoute::from(args);
    println!("rerouting {:?} => {:?}", router.from, router.to);
    router.run(
        |event| {
            event
                .name
                .and_then(|it| it.extension())
                .and_then(|it| it.to_str())
                .unwrap_or("")
                != "tmp"
        },
        |e| eprintln!("{:?}", e),
    )
}
