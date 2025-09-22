use std::fmt::{self, Write};

use num_enum::{FromPrimitive, IntoPrimitive};
use serde::{Deserialize, Serialize};
use serde_with::base64::Base64;
use serde_with::{DeserializeFromStr, serde_as, skip_serializing_none};

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct Ad {
    pub version: u32,
    pub model: String,
    pub port: u16,
    pub mac: MacAddr,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "t")]
pub enum BroadcastPacket {
    #[serde(rename = "magic4pc_ad")]
    Ad(Ad),
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub update_freq: Option<u32>,
    pub filter: Option<Vec<Sensor>>,
}

#[derive(Debug, Serialize, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "camelCase")]
pub enum Sensor {
    ReturnValue,
    DeviceId,
    Coordinate,
    Gyroscope,
    Acceleration,
    Quaternion,
}

#[derive(Debug, Serialize)]
#[serde(tag = "t")]
pub enum SubscriptionPacket {
    #[serde(rename = "sub_sensor")]
    Sub(Subscription),
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde_as]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum UnicastPacket {
    Input { parameters: Input },
    Mouse { mouse: Mouse },
    Wheel { wheel: Wheel },
    RemoteUpdate(Binary),
    Keepalive,
}

// Limitations in serde_as force this to be a separate struct.
// Specifically, the macro doesn't reject putting the attribute on an enum field,
// but it also doesn't actually *work properly*.
#[serde_as]
#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct Binary {
    #[serde_as(as = "Base64")]
    pub payload: Vec<u8>,
}

#[derive(Debug, Deserialize, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub key_code: KeyCode,
    pub is_down: bool,
}

#[derive(Debug, Deserialize, Copy, Clone, PartialEq, Eq)]
pub struct Mouse {
    #[serde(rename = "type")]
    pub event: MouseEvent,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Deserialize, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MouseEvent {
    MouseDown,
    MouseUp,
}

#[derive(Debug, Deserialize, Copy, Clone, PartialEq, Eq)]
pub struct Wheel {
    pub x: i32,
    pub y: i32,
    pub delta: i32,
}

#[derive(
    Debug,
    Deserialize,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    FromPrimitive,
    IntoPrimitive,
)]
#[repr(u32)]
#[serde(from = "u32")]
pub enum KeyCode {
    Left = 37,
    Up = 38,
    Right = 39,
    Down = 40,
    Ok = 13,
    Back = 461,
    Red = 403,
    Green = 404,
    Yellow = 405,
    Blue = 406,

    Play = 415,
    Pause = 19,
    FastForward = 417,
    Rewind = 412,
    Stop = 413,

    Num0 = 48,
    Num1 = 49,
    Num2 = 50,
    Num3 = 51,
    Num4 = 52,
    Num5 = 53,
    Num6 = 54,
    Num7 = 55,
    Num8 = 56,
    Num9 = 57,

    ChanUp = 33,
    ChanDown = 34,

    PointerMode = 1536,
    DpadMode = 1537,

    #[num_enum(catch_all)]
    Unknown(u32),
}

#[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, DeserializeFromStr)]
pub struct MacAddr(pub [u8; 6]);

impl fmt::Display for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, v) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_char(':')?;
            }
            write!(f, "{v:02X}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl std::str::FromStr for MacAddr {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut arr = [0; 6];
        for (i, segment) in s.splitn(6, ':').enumerate() {
            arr[i] = u8::from_str_radix(segment, 0x10)?;
        }
        Ok(Self(arr))
    }
}
