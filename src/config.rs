use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use heapless::String;
use serde::{Deserialize, Serialize};

pub const WIFI_STA_SSID: &str = option_env!("SSID").unwrap_or("Wokwi-GUEST");
pub const WIFI_STA_PASSWORD: &str = option_env!("PASSWORD").unwrap_or("");

pub const HTTP_SERVER_PORT: u16 = 80;
pub const WEB_TASK_POOL_SIZE: usize = 2;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub screen_dwell_ticks: u32,
    pub toggle_phase_ticks: u32,
    pub scroll_pause_ticks: u32,
    pub ns_station_code: String<8>,
    pub ns_api_key: String<64>,
}

pub static SETTINGS: Mutex<CriticalSectionRawMutex, Settings> = Mutex::new(Settings {
    screen_dwell_ticks: 100, // ~5 seconds per screen
    toggle_phase_ticks: 30,  // ~1.5 seconds toggle phase
    scroll_pause_ticks: 20,
    ns_station_code: String::new(),
    ns_api_key: String::new(),
});
