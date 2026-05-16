use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod commands;

#[derive(Parser)]
#[command(
    name = "rylr998",
    about = "Configure and drive REYAX RYLR998 LoRa radio modules from the command line.",
    version,
)]
struct Cli {
    /// Override auto-discovery of /dev/cu.usbserial*.
    #[arg(long, hide = true, global = true)]
    port: Option<PathBuf>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Print all readable settings.
    Info,
    /// Write address, network ID, and optionally band/parameters.
    Provision {
        #[arg(long)]
        address: u16,
        #[arg(long)]
        net: u8,
        #[arg(long)]
        band: Option<u32>,
        /// "S,B,C,P" -- four small ints
        #[arg(long, value_parser = parse_params)]
        params: Option<rylr998_std::RfParams>,
    },
    /// AT+FACTORY then wait for +READY.
    Reset,
    /// AT+SEND. Use `-` for the message to read stdin.
    Send {
        #[arg(long)]
        to: u16,
        message: String,
    },
    /// Read events forever, one per line.
    Listen,
}

fn parse_params(s: &str) -> Result<rylr998_std::RfParams, String> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 4 {
        return Err(format!("expected S,B,C,P (4 ints), got {} parts", parts.len()));
    }
    let n = |s: &str| s.parse::<u8>().map_err(|e| e.to_string());
    Ok(rylr998_std::RfParams {
        sf: n(parts[0])?,
        bw: n(parts[1])?,
        cr: n(parts[2])?,
        preamble: n(parts[3])?,
    })
}

fn main() {
    let cli = Cli::parse();
    let result = (|| -> rylr998_std::Result<()> {
        let radio = match cli.port {
            Some(p) => rylr998_std::Radio::open(&p),
            None => {
                let path = rylr998_std::Radio::discover()?;
                eprintln!("using port: {}", path.display());
                rylr998_std::Radio::open(&path)
            }
        }?;
        match cli.cmd {
            Cmd::Info => commands::info(radio),
            Cmd::Provision { address, net, band, params } => {
                commands::provision(radio, address, net, band, params)
            }
            Cmd::Reset => commands::reset(radio),
            Cmd::Send { to, message } => commands::send(radio, to, &message),
            Cmd::Listen => commands::listen(radio),
        }
    })();

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
