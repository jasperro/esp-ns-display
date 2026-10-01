use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Watch;
use heapless::{String, Vec};
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperatorLogo {
    NS,
    RRR,
    Blauwnet,
    None,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TopLeftDisplay {
    Countdown,
    DepTime,
    CurrentTime,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Departure {
    pub dep_time: String<8>,
    pub current_time: String<8>,
    pub minutes_left: u8,
    pub display_name: String<8>,
    pub logo: OperatorLogo,
    pub destination: String<32>,
    pub via_route: String<64>,
    pub coupled_sets: [u8; 2],
    pub travel_left: bool,
    pub next_train_info: String<32>,
    pub track: String<8>,
    pub show_track: bool,
}

impl Default for Departure {
    fn default() -> Self {
        Self {
            dep_time: String::try_from("--:--").unwrap_or_default(),
            current_time: String::try_from("--:--").unwrap_or_default(),
            minutes_left: 0,
            display_name: String::try_from("---").unwrap_or_default(),
            logo: OperatorLogo::None,
            destination: String::try_from("Geen gegevens").unwrap_or_default(),
            via_route: String::new(),
            coupled_sets: [0, 0],
            travel_left: true,
            next_train_info: String::new(),
            track: String::try_from("-").unwrap_or_default(),
            show_track: false,
        }
    }
}

pub static DEPARTURES: Watch<CriticalSectionRawMutex, Vec<Departure, 3>, 2> = Watch::new();
