use embassy_net::dns::DnsSocket;
use embassy_net::tcp::TcpSocket;
use embassy_net::Stack;
use embassy_time::{Duration, Timer};
use embedded_io_async::Write;
use embedded_tls::{Aes256GcmSha384, TlsConfig, TlsContext, TlsConnection, UnsecureProvider};
use esp_hal::rng::Trng;
use esp_println::println;
use heapless::{String, Vec};
use picoserve::make_static;
use serde::Deserialize;

use crate::common::{Departure, OperatorLogo, DEPARTURES};
use crate::config::SETTINGS;

// --- DTO Structs ---
#[derive(Deserialize, Debug)]
struct NsDepartureProduct<'a> {
    #[serde(borrow, rename = "shortCategory")]
    short_category: Option<&'a str>,
    #[serde(borrow, rename = "operatorCode")]
    operator_code: Option<&'a str>,
}

#[derive(Deserialize, Debug)]
struct NsDepartureItem<'a> {
    direction: Option<&'a str>,
    #[serde(borrow, rename = "plannedDepartureTime")]
    planned_departure_time: Option<&'a str>,
    #[serde(borrow, rename = "actualTrack")]
    actual_track: Option<&'a str>,
    #[serde(borrow, rename = "plannedTrack")]
    planned_track: Option<&'a str>,
    #[serde(borrow)]
    product: Option<NsDepartureProduct<'a>>,
}

#[derive(Deserialize, Debug)]
struct NsApiResponse<'a> {
    #[serde(borrow)]
    departures: Option<Vec<NsDepartureItem<'a>, 10>>,
}

#[derive(Deserialize, Debug)]
struct NsApiPayload<'a> {
    #[serde(borrow)]
    payload: Option<NsApiResponse<'a>>,
}

#[embassy_executor::task]
pub async fn ns_api_task(stack: Stack<'static>, trng: &'static mut Trng<'static>) {
    // Static Network Buffers
    let tcp_rx = make_static!([u8; 1536], [0u8; 1536]);
    let tcp_tx = make_static!([u8; 1536], [0u8; 1536]);
    let tls_rx = make_static!([u8; 4096], [0u8; 4096]);
    let tls_tx = make_static!([u8; 1024], [0u8; 1024]);
    let body_buf = make_static!([u8; 1024], [0u8; 1024]);

    const HOST: &str = "gateway.apiportal.ns.nl";

    println!("NS API: Task started and running.");

    loop {
        if !stack.is_config_up() {
            println!("NS API: Network stack is down, waiting...");
            Timer::after(Duration::from_millis(500)).await;
            continue;
        }

        let (station_code_buf, api_key_buf) = {
            let settings = SETTINGS.lock().await;
            (settings.ns_station_code.clone(), settings.ns_api_key.clone())
        };

        let station_code = if station_code_buf.is_empty() {
            "Amf"
        } else {
            station_code_buf.as_str()
        };

        let api_key = api_key_buf.as_str();

        if api_key.is_empty() {
            println!("NS API: Waiting for API key configuration...");
            Timer::after(Duration::from_secs(10)).await;
            continue;
        }

        println!("NS API: Resolving DNS for {}...", HOST);

        let dns = DnsSocket::new(stack);
        match dns.query(HOST, embassy_net::dns::DnsQueryType::A).await {
            Ok(addrs) => {
                if let Some(ip) = addrs.first() {
                    println!("NS API: Resolved IP: {}. Connecting TCP...", ip);

                    let mut socket = TcpSocket::new(stack, tcp_rx, tcp_tx);
                    match socket.connect((*ip, 443)).await {
                        Ok(_) => {
                            println!("NS API: TCP connected. Handshaking TLS...");

                            let config = TlsConfig::new().with_server_name(HOST)
                                .with_max_fragment_length(embedded_tls::MaxFragmentLength::Bits12);
                            let mut provider = UnsecureProvider::new(&mut *trng);
                            let tls_context = TlsContext::new(&config, &mut provider);

                            let mut tls_conn: TlsConnection<'_, TcpSocket<'_>, Aes256GcmSha384> =
                                TlsConnection::new(socket, tls_rx, tls_tx);

                            match tls_conn.open(tls_context).await {
                                Ok(_) => {
                                    println!("NS API: TLS Handshake complete. Formatting request...");

                                    let mut req_str = String::<256>::new();
                                    let _ = core::fmt::write(
                                        &mut req_str,
                                        format_args!(
                                            "GET /reisinformatie-api/api/v2/departures?station={}&maxJourneys=3 HTTP/1.1\r\nHost: {}\r\nOcp-Apim-Subscription-Key: {}\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
                                            station_code, HOST, api_key
                                        ),
                                    );

                                    println!("NS API: Sending HTTP Request...");
                                    match tls_conn.write_all(req_str.as_bytes()).await {
                                        Ok(_) => {
                                            println!("NS API: Request sent. Reading response...");

                                            let mut read_len = 0;
                                            loop {
                                                if read_len >= body_buf.len() {
                                                    println!("NS API: WARNING - Buffer full ({} bytes)", body_buf.len());
                                                    break;
                                                }

                                                match tls_conn.read(&mut body_buf[read_len..]).await {
                                                    Ok(0) => {
                                                        println!("NS API: Connection closed by server (EOF).");
                                                        break;
                                                    }
                                                    Ok(n) => {
                                                        read_len += n;
                                                        println!("NS API: Read {} bytes (Total: {})", n, read_len);
                                                    }
                                                    Err(e) => {
                                                        println!("NS API: Read error: {:?}", e);
                                                        break;
                                                    }
                                                }
                                            }

                                            println!("NS API: Total received bytes: {}", read_len);

                                            // Locate \r\n\r\n header split
                                            if let Some(body_start) = body_buf[..read_len]
                                                .windows(4)
                                                .position(|w| w == b"\r\n\r\n")
                                                .map(|p| p + 4)
                                            {
                                                println!("NS API: Header end found at byte offset {}", body_start);

                                                // Print headers for status code diagnostics
                                                if let Ok(header_str) = core::str::from_utf8(&body_buf[..body_start]) {
                                                    println!("--- HTTP HEADERS ---\n{}\n--------------------", header_str);
                                                }

                                                let json_bytes = &body_buf[body_start..read_len];
                                                
                                                // Safely print JSON string
                                                match core::str::from_utf8(json_bytes) {
                                                    Ok(json_str) => {
                                                        println!("--- RAW JSON BODY ---\n{}\n---------------------", json_str);
                                                    }
                                                    Err(_) => {
                                                        println!("NS API: JSON payload contains invalid UTF-8 (Raw length: {} bytes)", json_bytes.len());
                                                    }
                                                }

                                                println!("NS API: Attempting JSON deserialization...");
                                                match serde_json::from_slice::<NsApiPayload>(json_bytes) {
                                                    Ok(parsed) => {
                                                        println!("NS API: JSON parsed successfully!");

                                                        if let Some(payload) = parsed.payload {
                                                            if let Some(items) = payload.departures {
                                                                println!("NS API: Found {} departure item(s)", items.len());
                                                                let mut result_vec = Vec::<Departure, 3>::new();

                                                                for (idx, item) in items.iter().take(3).enumerate() {
                                                                    let mut dep = Departure::default();

                                                                    if let Some(dir) = item.direction {
                                                                        dep.destination = String::try_from(dir).unwrap_or_default();
                                                                    }

                                                                    let track_str = item
                                                                        .actual_track
                                                                        .or(item.planned_track)
                                                                        .unwrap_or("-");
                                                                    dep.track = String::try_from(track_str).unwrap_or_default();
                                                                    dep.show_track = true;

                                                                    if let Some(ref prod) = item.product {
                                                                        if let Some(cat) = prod.short_category {
                                                                            dep.display_name = String::try_from(cat).unwrap_or_default();
                                                                        }
                                                                        dep.logo = match prod.operator_code.unwrap_or("") {
                                                                            "NS" => OperatorLogo::NS,
                                                                            "RRR" | "Keolis" => OperatorLogo::RRR,
                                                                            "Arriva" | "Blauwnet" => OperatorLogo::Blauwnet,
                                                                            _ => OperatorLogo::None,
                                                                        };
                                                                    }

                                                                    if let Some(time_str) = item.planned_departure_time {
                                                                        if time_str.len() >= 16 {
                                                                            dep.dep_time = String::try_from(&time_str[11..16]).unwrap_or_default();
                                                                        }
                                                                    }

                                                                    println!(
                                                                        "  [#{}] Time: {}, Dest: {}, Track: {}, Type: {}",
                                                                        idx + 1,
                                                                        dep.dep_time.as_str(),
                                                                        dep.destination.as_str(),
                                                                        dep.track.as_str(),
                                                                        dep.display_name.as_str()
                                                                    );

                                                                    let _ = result_vec.push(dep);
                                                                }

                                                                DEPARTURES.sender().send(result_vec);
                                                                println!("NS API: Broadcasted departures successfully.");
                                                            } else {
                                                                println!("NS API: 'payload.departures' field was None");
                                                            }
                                                        } else {
                                                            println!("NS API: 'payload' field was None");
                                                        }
                                                    }
                                                    Err(e) => {
                                                        println!("NS API: Deserialization failed: {:?}", e);
                                                    }
                                                }
                                            } else {
                                                println!("NS API: Failed to find header delimiter (\\r\\n\\r\\n) in response!");
                                            }
                                        }
                                        Err(e) => println!("NS API: Write request error: {:?}", e),
                                    }
                                }
                                Err(e) => println!("NS API: TLS Handshake failed: {:?}", e),
                            }
                        }
                        Err(e) => println!("NS API: TCP Connect failed: {:?}", e),
                    }
                } else {
                    println!("NS API: DNS query returned an empty address list.");
                }
            }
            Err(e) => println!("NS API: DNS Resolution failed: {:?}", e),
        }

        println!("NS API: Sleeping for 45 seconds before next poll...");
        Timer::after(Duration::from_secs(45)).await;
    }
}
