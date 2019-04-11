#[macro_use]
extern crate structopt;

use std::fs;
use libc;
use std::ffi::CString;
use std::collections::HashSet;
use pnet::datalink::{self, NetworkInterface};
use pnet::datalink::Channel::Ethernet;
use pnet::packet::{Packet, MutablePacket};
use pnet::packet::ethernet::{EthernetPacket, MutableEthernetPacket};
use pnet::datalink::{DataLinkReceiver, DataLinkSender};
use std::env;
use std::path::PathBuf;
use structopt::StructOpt;

fn main() {
    let opt:Opt = Opt::from_args();
    match opt.cmd {
        Command::bridge {
            i1,i2
        } => {
            bridge(i1,i2);
        }
        Command::link { pid } => {
            make_netns_symlink_frompid(pid)
        }
    }
}

fn clean_netns_symlink() {
    let pids = get_pids();
    let nss = get_ns();
    let nss_to_clean = nss.iter().flat_map(|ns| {
        if pids.get(ns).is_some() { None } else { Some(ns) }
    });
    nss_to_clean.for_each(|ns|{
        if fs::remove_file(format!("/var/run/netns/{}", ns)).is_err() {
            println!("remove file error:{}",format!("/var/run/netns/{}", ns));
        }
    })
}

fn make_netns_symlink_frompid(pid: u32) {
    let dst = CString::new(format!("/var/run/netns/{}", pid)).expect("CString::new failed");
    let src = CString::new(format!("/proc/{}/ns/net", pid)).expect("CString::new failed");

    unsafe {
        if libc::symlink(src.as_ptr(), dst.as_ptr()) == -1 {
            println!("symlink error");
        }
    }
}

fn get_pids() -> HashSet<u32> {
    let pid_paths = fs::read_dir("/proc/").unwrap();
    let pids = pid_paths.flat_map(move |path| {
        let path_unwrap = path.unwrap();
        let is_dir = path_unwrap.file_type().unwrap().is_dir();
        let name = path_unwrap.file_name().into_string().unwrap();
        let pid = name.parse::<u32>().ok();
        if is_dir {
            pid
        }
        else {
            None
        }
    });
    pids.collect()
}

fn get_ns() -> HashSet<u32> {
    let ns_paths = fs::read_dir("/var/run/netns/").unwrap();
    let nss = ns_paths.flat_map(move |ns| {
        let ns_unwrap = ns.unwrap();
        let is_symlink = ns_unwrap.file_type().unwrap().is_symlink();
        let name = ns_unwrap.file_name().into_string().unwrap();
        let pid = name.parse::<u32>().ok();
        if is_symlink {
            pid
        }
        else {
            None
        }
    });
    nss.collect()
}

fn create_channel(interface_name: String) -> (Box<DataLinkSender>, Box<DataLinkReceiver>) {
    let interface_names_match =
        |iface: &NetworkInterface| iface.name == interface_name;

    // Find the network interface with the provided name
    let interfaces = datalink::interfaces();
    let interface = interfaces.into_iter()
        .filter(interface_names_match)
        .next()
        .unwrap();

    // Create a new channel, dealing with layer 2 packets
    let (mut tx, mut rx) = match datalink::channel(&interface, Default::default()) {
        Ok(Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("Unhandled channel type"),
        Err(e) => panic!("An error occurred when creating the datalink channel: {}", e)
    };

    (tx, rx)
}

fn bridge(interface1_name:String,interface2_name:String) {
    let (mut tx1, mut rx1) = create_channel(interface1_name.clone());
    let (mut tx2, mut rx2) = create_channel(interface2_name.clone());

    let i1 = interface1_name.clone();
    let i2 = interface2_name.clone();
    let j1 = std::thread::spawn(move || {
        loop {
            match rx1.next() {
                Ok(packet) => {
                    println!("{} -> {} {} bytes", &i1, &i2, packet.len());
                    tx2.send_to(packet, None).unwrap();
                }
                Err(err) => {
                    dbg!(err);
                    break;
                }
            }
        }
    });
    let j2 = std::thread::spawn(move || {
        let i1 = interface1_name.clone();
        let i2 = interface2_name.clone();
        loop {
            match rx2.next() {
                Ok(packet) => {
                    println!("{} -> {} {} bytes", &i2, &i1, packet.len());
                    tx1.send_to(packet, None).unwrap();
                }
                Err(err) => {
                    dbg!(err);
                    break;
                }
            }
        }
    });
    j1.join();
}

#[derive(Debug, StructOpt)]
#[structopt(name = "main", about = "An tool for mininet.")]
struct Opt {
    #[structopt(short = "d", long = "debug")]
    debug: bool,
    #[structopt(subcommand)]  // Note that we mark a field as a subcommand
    cmd: Command
}

#[derive(Debug,StructOpt)]
enum Command {
    #[structopt(name="bridge")]
    /// bridge two interface by copying packet to each other, used by attachByPeer
    bridge {
        #[structopt(long = "i1")]
        i1:String,
        #[structopt(long = "i2")]
        i2:String
    },
    /// make net namespace visible to ip tool
    link {
        #[structopt(short="p")]
        pid:u32
    }
}