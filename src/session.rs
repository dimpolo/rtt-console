// Adapted from https://github.com/knurling-rs/probe-run

use anyhow::{anyhow, bail};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use clap::Parser;
use probe_rs::DebugProbeError::ProbeSpecific;
use probe_rs::{DebugProbeInfo, Permissions, Probe, Session};

/// A RTT console
#[derive(Parser)]
#[command()]
pub struct Opts {
    /// The chip to program.
    #[arg(long, required = true)]
    chip: String,

    /// Path to chip description file, in YAML format.
    #[arg(long)]
    chip_description_path: Option<PathBuf>,

    /// Connect to device when NRST is pressed.
    #[arg(long)]
    connect_under_reset: bool,

    /// The probe to use (eg. `VID:PID`, `VID:PID:Serial`, or just `Serial`).
    #[arg(long)]
    probe: Option<String>,

    /// The probe clock frequency in kHz
    #[arg(long)]
    speed: Option<u32>,
}

pub fn get_session() -> anyhow::Result<Session> {
    let opts = Opts::parse();
    let probe_target = lookup_probe_target(&opts.chip, &opts)?;
    let sess = attach_to_probe(probe_target.clone(), &opts)?;
    Ok(sess)
}

fn lookup_probe_target(chip_name: &str, opts: &Opts) -> anyhow::Result<probe_rs::Target> {
    // register chip description
    if let Some(cdp) = &opts.chip_description_path {
        probe_rs::config::add_target_from_yaml(fs::File::open(cdp)?)?;
    }

    // look up target
    let probe_target = probe_rs::config::get_target_by_name(chip_name)?;
    Ok(probe_target)
}

fn attach_to_probe(probe_target: probe_rs::Target, opts: &Opts) -> anyhow::Result<Session> {
    let permissions = Permissions::default();
    let probe = open(opts)?;
    let sess = if opts.connect_under_reset {
        probe.attach_under_reset(probe_target, permissions)
    } else {
        let probe_attach = probe.attach(probe_target, permissions);
        if let Err(probe_rs::Error::Probe(ProbeSpecific(e))) = &probe_attach {
            // FIXME Using `to_string().contains(...)` is a workaround as the concrete type
            // of `e` is not public and therefore does not allow downcasting.
            if e.to_string().contains("JtagNoDeviceConnected") {
                eprintln!("Info: Jtag cannot find a connected device.");
                eprintln!("Help:");
                eprintln!("    Check that the debugger is connected to the chip, if so");
                eprintln!("    try using probe-run with option `--connect-under-reset`");
                eprintln!("    or, if using cargo:");
                eprintln!("        cargo run -- --connect-under-reset");
                eprintln!("    If using this flag fixed your issue, this error might");
                eprintln!("    come from the program currently in the chip and using");
                eprintln!("    `--connect-under-reset` is only a workaround.\n");
            }
        }
        probe_attach
    }?;
    Ok(sess)
}

const NO_PROBE_FOUND_ERR: &str = "no probe was found.\n
Common reasons for this are faulty cables or missing permissions.
For detailed instructions, visit: https://github.com/knurling-rs/probe-run#troubleshooting";

pub fn open(opts: &Opts) -> Result<Probe, anyhow::Error> {
    let all_probes = Probe::list_all();
    let filtered_probes = if let Some(probe_opt) = opts.probe.as_deref() {
        let selector = probe_opt.parse()?;
        filter(&all_probes, &selector)
    } else {
        all_probes
    };

    if filtered_probes.is_empty() {
        bail!("{}", NO_PROBE_FOUND_ERR)
    }

    if filtered_probes.len() > 1 {
        print(&filtered_probes);
        bail!("more than one probe found; use --probe to specify which one to use");
    }

    let mut probe = filtered_probes[0].open()?;

    if let Some(speed) = opts.speed {
        probe.set_speed(speed)?;
    }

    Ok(probe)
}

pub fn print(probes: &[DebugProbeInfo]) {
    if !probes.is_empty() {
        println!("the following probes were found:");
        probes
            .iter()
            .enumerate()
            .for_each(|(num, link)| println!("[{num}]: {link:?}"));
    } else {
        println!("Error: {NO_PROBE_FOUND_ERR}");
    }
}

fn filter(probes: &[DebugProbeInfo], selector: &ProbeFilter) -> Vec<DebugProbeInfo> {
    probes
        .iter()
        .filter(|probe| {
            if let Some((vid, pid)) = selector.vid_pid {
                if probe.vendor_id != vid || probe.product_id != pid {
                    return false;
                }
            }

            if let Some(serial) = &selector.serial {
                if probe.serial_number.as_deref() != Some(serial) {
                    return false;
                }
            }

            true
        })
        .cloned()
        .collect()
}

struct ProbeFilter {
    vid_pid: Option<(u16, u16)>,
    serial: Option<String>,
}

impl FromStr for ProbeFilter {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = s.split(':').collect::<Vec<_>>();
        match *parts {
            [serial] => Ok(Self {
                vid_pid: None,
                serial: Some(serial.to_string()),
            }),
            [vid, pid] => Ok(Self {
                vid_pid: Some((u16::from_str_radix(vid, 16)?, u16::from_str_radix(pid, 16)?)),
                serial: None,
            }),
            [vid, pid, serial] => Ok(Self {
                vid_pid: Some((u16::from_str_radix(vid, 16)?, u16::from_str_radix(pid, 16)?)),
                serial: Some(serial.to_string()),
            }),
            _ => Err(anyhow!("invalid probe filter")),
        }
    }
}
