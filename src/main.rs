use std::io;
use std::net::{Ipv4Addr, UdpSocket};

use enigo::{Enigo, Keyboard, Mouse};

pub mod types;
use types::*;

const BROADCAST_PORT: u16 = 42830;
const SUBSCRIPTION_PORT: u16 = 42831;

fn main() -> io::Result<()> {
    let mut buf = vec![0; 2048];
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, BROADCAST_PORT))?;
    loop {
        let (len, addr) = socket.recv_from(&mut buf)?;
        let data = &buf[..len];

        let Ok(BroadcastPacket::Ad(ad)) = serde_json::from_slice(data) else {
            continue;
        };

        let sub = Subscription {
            update_freq: Some(100),
            filter: Some(vec![Sensor::Coordinate]),
        };

        let conn = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, SUBSCRIPTION_PORT))?;
        conn.connect((addr.ip(), ad.port))?;

        let packet = serde_json::to_string(&SubscriptionPacket::Sub(sub)).unwrap();

        conn.send(packet.as_bytes())?;
        println!("connected to {}", addr.ip());

        // yeah stop listening for new ads while already connected I guess
        // don't even bother spawning a task!
        // not like this program has anything better to do
        if let Err(e) = handle_connection(conn) {
            println!("conn died: {e}");
        }
    }
}

fn handle_connection(socket: UdpSocket) -> io::Result<()> {
    let mut buf = vec![0; 2048];

    let mut enigo = Enigo::new(&enigo::Settings::default()).unwrap();

    // TODO: keepalive timeout: keepalive is sent every so often,
    // so if we go a bit without seeing ANY message, disconnect.

    let mut cursor = (0, 0);

    loop {
        let len = socket.recv(&mut buf)?;
        let data = &buf[..len];

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
