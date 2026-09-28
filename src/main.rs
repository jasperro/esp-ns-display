#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]

use embassy_executor::Spawner;
use esp_hal::clock::CpuClock;
use esp_hal::rng::Rng;
use esp_hal::timer::timg::TimerGroup;
use esp_println::println;

use esp_ns_display::{display, web_server, wifi};

#[repr(C)]
pub struct EspAppDesc {
    pub magic_word: u32,
    pub secure_version: u32,
    pub reserv1: [u32; 2],
    pub version: [u8; 32],
    pub project_name: [u8; 32],
    pub time: [u8; 16],
    pub date: [u8; 16],
    pub idf_ver: [u8; 32],
    pub app_elf_sha256: [u8; 32],
    pub reserv2: [u32; 20],
}

#[used]
#[export_name = "esp_app_desc"]
#[link_section = ".app_desc"]
pub static ESP_APP_DESC: EspAppDesc = EspAppDesc {
    magic_word: 0xabcd5432,
    secure_version: 0,
    reserv1: [0; 2],
    version: *b"0.1.0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
    project_name: *b"esp-ns-display\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
    time: *b"00:00:00\0\0\0\0\0\0\0\0",
    date: *b"Jan  1 2026\0\0\0\0\0",
    idf_ver: *b"v5.0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
    app_elf_sha256: [0; 32],
    reserv2: [0; 20],
};

#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    println!("{info}");
    loop {}
}

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    esp_println::logger::init_logger_from_env();

    println!("Welkom!");

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(72 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let timg1 = TimerGroup::new(peripherals.TIMG1);
    let mut rng = Rng::new(peripherals.RNG);

    esp_hal_embassy::init(timg0.timer0);

    // Start Wi-Fi network stack
    let stack = wifi::start_wifi(timg1.timer0, &mut rng, peripherals.RADIO_CLK, peripherals.WIFI, spawner).await;

    // Start Display Driver Task
    spawner.must_spawn(display::display_task(
        peripherals.I2C0,
        peripherals.GPIO16,
        peripherals.GPIO17,
    ));

    // Start Web Server Tasks
    web_server::start_web_server(spawner, stack).await;
}