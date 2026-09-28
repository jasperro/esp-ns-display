pub const WIFI_STA_SSID: &str = option_env!("SSID").unwrap_or("Wokwi-GUEST");
pub const WIFI_STA_PASSWORD: &str = option_env!("PASSWORD").unwrap_or("");

pub const HTTP_SERVER_PORT: u16 = 80;
pub const WEB_TASK_POOL_SIZE: usize = 4;