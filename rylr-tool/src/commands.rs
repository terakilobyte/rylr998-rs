use rylr998_std::{OwnedEvent, Radio, Result, RfParams};
use std::io::Read;
use std::time::Duration;

type R = Radio<Box<dyn serialport::SerialPort>>;

pub fn info(mut r: R) -> Result<()> {
    println!("ping        {}", marker(r.ping()));
    println!("address     {:?}", r.address()?);
    println!("network_id  {:?}", r.network_id()?);
    println!("band        {:?}", r.band()?);
    let p = r.parameters()?;
    println!(
        "parameters  sf={} bw={} cr={} preamble={}",
        p.sf, p.bw, p.cr, p.preamble
    );
    println!("crfop       {:?}", r.crfop()?);
    println!("uid         {}", r.uid()?);
    println!("version     {}", r.version()?);
    Ok(())
}

pub fn provision(
    mut r: R,
    address: u16,
    net: u8,
    band: Option<u32>,
    params: Option<RfParams>,
) -> Result<()> {
    r.set_address(address)?;
    if r.address()? != address {
        return Err(verify_failed("address"));
    }
    r.set_network_id(net)?;
    if r.network_id()? != net {
        return Err(verify_failed("network_id"));
    }
    if let Some(b) = band {
        r.set_band(b)?;
        if r.band()? != b {
            return Err(verify_failed("band"));
        }
    }
    if let Some(p) = params {
        r.set_parameters(p)?;
        if r.parameters()? != p {
            return Err(verify_failed("parameters"));
        }
    }
    println!(
        "provisioned address={} net={} band={} params={}",
        address,
        net,
        band.map(|b| b.to_string())
            .unwrap_or_else(|| "(unchanged)".into()),
        match params {
            Some(p) => format!("{},{},{},{}", p.sf, p.bw, p.cr, p.preamble),
            None => "(unchanged)".into(),
        }
    );
    Ok(())
}

pub fn reset(mut r: R) -> Result<()> {
    r.factory_reset()
}

pub fn send(mut r: R, to: u16, message: &str) -> Result<()> {
    let bytes: Vec<u8> = if message == "-" {
        let mut buf = Vec::new();
        std::io::stdin().read_to_end(&mut buf)?;
        buf
    } else {
        message.as_bytes().to_vec()
    };
    r.send(to, &bytes)?;
    println!("+OK");
    Ok(())
}

pub fn listen(mut r: R) -> Result<()> {
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let s2 = stop.clone();
    ctrlc::set_handler(move || s2.store(true, std::sync::atomic::Ordering::SeqCst))
        .map_err(std::io::Error::other)?;

    while !stop.load(std::sync::atomic::Ordering::SeqCst) {
        match r.next_event(Duration::from_secs(1)) {
            Ok(OwnedEvent::Recv {
                from,
                data,
                rssi,
                snr,
            }) => {
                let s = String::from_utf8_lossy(&data);
                println!("from={from} rssi={rssi} snr={snr} \"{s}\"");
            }
            Ok(OwnedEvent::Ready) => eprintln!("(radio rebooted)"),
            Err(rylr998_std::Error::Timeout) => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

fn marker(r: Result<()>) -> &'static str {
    match r {
        Ok(()) => "ok",
        Err(_) => "fail",
    }
}

fn verify_failed(field: &str) -> rylr998_std::Error {
    rylr998_std::Error::Io(std::io::Error::other(format!(
        "provisioning verify failed: {field}"
    )))
}
