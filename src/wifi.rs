use embassy_executor::Spawner;
use embassy_net::{Runner, Stack, StackResources};
use esp_hal::peripherals::{RADIO_CLK, WIFI};
use esp_hal::rng::Rng;
use esp_println::println;
use esp_wifi::wifi::{
    ClientConfiguration, Configuration, WifiController, WifiDevice, WifiEvent, WifiStaDevice,
};
use esp_wifi::{init as init_wifi, EspWifiController};
use picoserve::make_static;

use crate::config;

#[embassy_executor::task]
async fn connection_task(mut controller: WifiController<'static>) -> ! {
    println!("Starting Wi-Fi connection loop...");

    if let Err(e) = controller.start_async().await {
        println!("Failed to start Wi-Fi driver: {e:?}");
    }

    println!("Loading client config...");

    let client_config = Configuration::Client(ClientConfiguration {
        ssid: config::WIFI_STA_SSID.try_into().unwrap_or_default(),
        password: config::WIFI_STA_PASSWORD.try_into().unwrap_or_default(),
        ..Default::default()
    });

    println!("Setting configuration...");

    if let Err(e) = controller.set_configuration(&client_config) {
        println!("Failed to set Wi-Fi config: {e:?}");
    }

    loop {
        println!("Connecting to Wi-Fi SSID: {}...", config::WIFI_STA_SSID);

        match controller.connect_async().await {
            Ok(_) => {
                println!("Wi-Fi connected!");
                // Wait until hardware signals disconnection before retrying
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                println!("Wi-Fi disconnected! Retrying in 5 seconds...");
            }
            Err(e) => {
                println!("Failed to connect to Wi-Fi: {e:?}");
            }
        }

        embassy_time::Timer::after_secs(5).await;
    }
}

pub async fn start_wifi(
    timer0: esp_hal::timer::timg::Timer,
    rng: &mut Rng,
    radio_clk: RADIO_CLK,
    wifi: WIFI,
    spawner: Spawner,
) -> Stack<'static> {
    let init = make_static!(
        EspWifiController<'static>,
        init_wifi(timer0, rng.clone(), radio_clk).expect("Failed to init Wi-Fi")
    );

    let (wifi_interface, controller) =
        esp_wifi::wifi::new_with_mode(init, wifi, WifiStaDevice)
            .expect("Failed to create Wi-Fi device");

    let config = embassy_net::Config::dhcpv4(Default::default());
    let seed = (rng.random() as u64) << 32 | (rng.random() as u64);

    let resources = make_static!(StackResources<6>, StackResources::<6>::new());

    let (stack, runner) = embassy_net::new(
        wifi_interface,
        config,
        resources,
        seed,
    );

    spawner.must_spawn(connection_task(controller));
    spawner.must_spawn(net_task(runner));

    println!("Waiting for IP assignment...");
    
    // Efficiently suspend task execution until DHCP assigns IP address
    stack.wait_config_up().await;

    if let Some(config) = stack.config_v4() {
        println!("Acquired IP Address: {}", config.address);
    }

    stack
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static, WifiStaDevice>>) {
    runner.run().await
}