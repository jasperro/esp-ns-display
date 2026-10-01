use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Watch;
use embassy_time::Duration;
use esp_println::println;
use picoserve::{
    extract::Json,
    make_static,
    routing::{get, get_service},
    AppBuilder, AppRouter,
};

use crate::config::{self, SETTINGS};

pub static DISPLAY_FRAME: Watch<CriticalSectionRawMutex, [u8; 1024], 2> = Watch::new();

pub struct AppProps;

struct DisplayStream;

impl picoserve::response::sse::EventSource for DisplayStream {
    async fn write_events<W: picoserve::io::Write>(
        self,
        mut writer: picoserve::response::sse::EventWriter<'_, W>,
    ) -> Result<(), W::Error> {
        let mut receiver = match DISPLAY_FRAME.receiver() {
            Some(r) => r,
            None => return Ok(()),
        };
        const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

        loop {
            let frame = receiver.changed().await;
            
            // Stream the 1024-byte frame in 4 smaller 256-byte chunks (512 hex chars)
            // to avoid storing a 2KB buffer inside the async Future state machine.
            let mut hex_buf = [0u8; 512];

            for chunk_idx in 0..4 {
                let start = chunk_idx * 256;
                let chunk = &frame[start..start + 256];

                for (i, &byte) in chunk.iter().enumerate() {
                    hex_buf[i * 2] = HEX_DIGITS[(byte >> 4) as usize];
                    hex_buf[i * 2 + 1] = HEX_DIGITS[(byte & 0x0F) as usize];
                }

                if let Ok(hex_str) = core::str::from_utf8(&hex_buf) {
                    writer.write_event("frame", hex_str).await?;
                }
            }
        }
    }
}

impl AppBuilder for AppProps {
    type PathRouter = impl picoserve::routing::PathRouter;

    fn build_app(self) -> picoserve::Router<Self::PathRouter> {
        picoserve::Router::new()
            .route(
                "/",
                get_service(picoserve::response::File::html(include_str!(
                    "web/index.html"
                ))),
            )
            .route(
                "/style.css",
                get_service(picoserve::response::File::css(include_str!(
                    "web/style.css"
                ))),
            )
            .route(
                "/main.js",
                get_service(picoserve::response::File::javascript(include_str!(
                    "web/main.js"
                ))),
            )
            .route(
                "/api/settings",
                get(|| async {
                    let settings = SETTINGS.lock().await.clone();
                    Json(settings)
                })
                .post(|Json(new_settings)| async move {
                    let mut lock = SETTINGS.lock().await;
                    *lock = new_settings;
                    Json(lock.clone())
                }),
            )
            .route(
                "/api/display/stream",
                get(|| async { picoserve::response::sse::EventStream(DisplayStream) }),
            )
    }
}

#[embassy_executor::task(pool_size = config::WEB_TASK_POOL_SIZE)]
async fn web_task(
    id: usize,
    stack: embassy_net::Stack<'static>,
    app: &'static AppRouter<AppProps>,
    config: &'static picoserve::Config,
) -> ! {
    let mut tcp_rx_buffer = [0; 512];
    let mut tcp_tx_buffer = [0; 512];
    let mut http_buffer = [0; 1024];

    println!("Web server listening on port {}", config::HTTP_SERVER_PORT);

    picoserve::Server::new(app, config, &mut http_buffer)
        .listen_and_serve(id, stack, config::HTTP_SERVER_PORT, &mut tcp_rx_buffer, &mut tcp_tx_buffer)
        .await
        .into_never()
}

pub async fn start_web_server(spawner: Spawner, stack: embassy_net::Stack<'static>) {
    println!("Starting web server with {} tasks...", config::WEB_TASK_POOL_SIZE);

    let app = make_static!(AppRouter<AppProps>, AppProps.build_app());

    let config = make_static!(
        picoserve::Config,
        picoserve::Config::new(picoserve::Timeouts {
            start_read_request: Duration::from_secs(5),
            persistent_start_read_request: Duration::from_secs(1),
            read_request: Duration::from_secs(1),
            write: Duration::from_secs(1),
        })
        .keep_connection_alive()
    );

    for id in 0..config::WEB_TASK_POOL_SIZE {
        spawner.must_spawn(web_task(id, stack, app, config));
    }
}