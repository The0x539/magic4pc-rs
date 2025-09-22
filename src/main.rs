use std::collections::HashSet;
use std::hash::Hash;
use std::io::{self, ErrorKind};
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::str::FromStr;
use std::time::Duration;

use enigo::{Enigo, Keyboard, Mouse};

pub mod types;
use types::*;

const BROADCAST_PORT: u16 = 42830;
const SUBSCRIPTION_PORT: u16 = 42831;
const TIMEOUT: Duration = Duration::from_secs(5);

fn parse_known_devices<T: FromStr + Hash + Eq>() -> HashSet<T> {
    /// A text file with one IP address or MAC address on each line.
    /// Included at compile time for now, until I figure out an alternative I like.
    const KNOWN_DEVICES_TXT: &str = include_str!("../devices.txt");

    KNOWN_DEVICES_TXT
        .lines()
        .map(|l| l.trim())
        .flat_map(|l| l.parse())
        .collect()
}

fn main() -> io::Result<()> {
    let mut buf = vec![0; 2048];
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, BROADCAST_PORT))?;

    let known_ips = parse_known_devices::<IpAddr>();
    let known_macs = parse_known_devices::<MacAddr>();

    if known_ips.is_empty() && known_macs.is_empty() {
        eprintln!(
            "Error: A `devices.txt` file was present, but no device addresses were found inside."
        );
        std::process::exit(1);
    } else {
        println!("Listening for announcements from:");
        for addr in &known_ips {
            println!("  - {addr}");
        }
        for addr in &known_macs {
            println!("  - {addr}");
        }
    }

    loop {
        let (len, addr) = socket.recv_from(&mut buf)?;
        let data = &buf[..len];

        let ip = addr.ip();

        let ad = match serde_json::from_slice(data) {
            Ok(BroadcastPacket::Ad(ad)) => ad,
            Err(e) => {
                eprintln!("Warning: Failed to parse message from device at {ip}: {e}");
                eprintln!(
                    "Raw message (assuming text): {}",
                    String::from_utf8_lossy(data),
                );
                continue;
            }
        };

        let mac = ad.mac;

        if !known_macs.contains(&mac) && !known_ips.contains(&ip) {
            println!("Ignoring announcement from unrecognized device: {mac} @ {ip}");
        }

        let sub = Subscription {
            update_freq: Some(100),
            filter: Some(vec![Sensor::Coordinate]),
        };

        let conn = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, SUBSCRIPTION_PORT))?;
        conn.connect((ip, ad.port))?;

        let packet = serde_json::to_string(&SubscriptionPacket::Sub(sub)).unwrap();

        conn.send(packet.as_bytes())?;
        println!("connected to {mac} @ {ip}");

        // yeah stop listening for new ads while already connected I guess
        // don't even bother spawning a task!
        // not like this program has anything better to do
        if let Err(e) = handle_connection(conn) {
            println!("connection to {mac} @ {ip} died: {e}");
        }
    }
}

fn handle_connection(socket: UdpSocket) -> io::Result<()> {
    let mut buf = vec![0; 2048];

    let mut enigo = Enigo::new(&enigo::Settings::default()).unwrap();

    socket.set_read_timeout(Some(TIMEOUT))?;

    let mut cursor = (0, 0);

    loop {
        let data = match socket.recv(&mut buf) {
            Ok(len) => &buf[..len],
            Err(e) => {
                break match e.kind() {
                    // assume the TV dropped the connection
                    // gracefully return and let the parent loop find a new connection
                    ErrorKind::WouldBlock | ErrorKind::TimedOut => {
                        if let Ok(addr) = socket.peer_addr() {
                            println!("disconnected from {addr}");
                        }
                        Ok(())
                    }
                    // some kind of actual non-timeout network error
                    _ => Err(e),
                };
            }
        };

        match serde_json::from_slice::<UnicastPacket>(data) {
            // keepalive
            Ok(UnicastPacket::Keepalive) => _ = socket.send(b"{}")?,
            // actual packet (ignore enigo errors I guess)
            Ok(packet) => _ = handle_packet(packet, &mut enigo, &mut cursor),
            // unknown packet
            Err(e) => println!(
                "couldn't parse packet: {e} ({})",
                String::from_utf8_lossy(data)
            ),
        };
    }
}

fn handle_packet(
    packet: UnicastPacket,
    enigo: &mut Enigo,
    cursor: &mut (i32, i32),
) -> enigo::InputResult<()> {
    match packet {
        // handled before we get to this function,
        // because it's the only one that uses the socket
        UnicastPacket::Keepalive => unreachable!(),

        UnicastPacket::Input { parameters } => {
            use KeyCode as K;
            use enigo::Key as E;
            let dir = direction(parameters.is_down);
            let key = match parameters.key_code {
                K::Red | K::Green | K::Blue | K::Yellow => {
                    return enigo.button(enigo::Button::Right, dir);
                }

                K::ChanUp => E::PageUp,
                K::ChanDown => E::PageDown,

                K::Up => E::UpArrow,
                K::Down => E::DownArrow,
                K::Left => E::LeftArrow,
                K::Right => E::RightArrow,

                k if (K::Num0..=K::Num9).contains(&k) => {
                    let num = u32::from(k) - u32::from(K::Num0);
                    let ch = char::from_digit(num, 10).unwrap();
                    enigo::Key::Unicode(ch)
                }

                // unbound key
                _ => return Ok(()),
            };
            enigo.key(key, dir)
        }
        UnicastPacket::Mouse { mouse } => {
            let dir = direction(mouse.event == MouseEvent::MouseDown);
            enigo.button(enigo::Button::Left, dir)
        }
        UnicastPacket::Wheel { wheel } => {
            // It seems like webOS reports a delta of 120 per click.
            // Enigo just expects a quantity of clicks.
            enigo.scroll(-wheel.delta / 120, enigo::Axis::Vertical)
        }
        UnicastPacket::RemoteUpdate(update) => {
            let payload = update.payload;
            if payload.len() != 8 {
                // this isn't the payload we asked for, which is just the coordinates of the cursor
                // just ignore it, I guess
                return Ok(());
            }

            let x = i32::from_le_bytes(payload[..4].try_into().unwrap());
            let y = i32::from_le_bytes(payload[4..].try_into().unwrap());

            // this way, you can put the remote down and move the mouse with another device
            // and not have to wait for the remote to time out.
            // otherwise, it constantly tries to stay glued to the TV pointer.
            let new_cursor = (x, y);
            if new_cursor != *cursor {
                *cursor = new_cursor;
                enigo.move_mouse(x, y, enigo::Coordinate::Abs)?;
            }
            Ok(())
        }
    }
}

fn direction(down: bool) -> enigo::Direction {
    if down {
        enigo::Direction::Press
    } else {
        enigo::Direction::Release
    }
}
