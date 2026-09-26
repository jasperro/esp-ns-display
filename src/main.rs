#![no_std]
#![no_main]

use embedded_graphics::{
    image::{Image, ImageRawLE},
    mono_font::{
        ascii::{FONT_5X8, FONT_6X10, FONT_8X13_BOLD},
        MonoTextStyle,
    },
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{CornerRadii, Line, PrimitiveStyle, Rectangle, RoundedRectangle},
    text::Text,
};
use esp_backtrace as _;
use esp_hal::{
    delay::Delay,
    i2c::master::{Config as I2cConfig, I2c},
    main,
    time::Rate,
};
use ssd1306::{mode::BufferedGraphicsMode, prelude::*, I2CDisplayInterface, Ssd1306};

// -----------------------------------------------------------------------------
// Display Settings Configuration
// -----------------------------------------------------------------------------

struct Settings {
    screen_dwell_ticks: u32,
    toggle_phase_ticks: u32,
    scroll_pause_ticks: u32,
}

const CONFIG: Settings = Settings {
    screen_dwell_ticks: 100, // ~5 seconds per screen mode
    toggle_phase_ticks: 30,  // ~1.5 seconds toggle for top-right track/logo
    scroll_pause_ticks: 20,
};

// -----------------------------------------------------------------------------
// Custom Operator Logos (16x10 px)
// -----------------------------------------------------------------------------

/// NS (Nederlandse Spoorwegen) Logo
#[rustfmt::skip]
const NS_LOGO_RAW: [u8; 16] = [
    0x0f, 0x10, 0x10, 0x88, 0x20, 0x44, 0x7c, 0x3e, 
    0x22, 0x04, 0x11, 0x08, 0x08, 0xf0, 0x00, 0x00
];

/// RRReis Logo
#[rustfmt::skip]
const RRR_LOGO_RAW: [u8; 16] = [
    0x73, 0x9c, 0x08, 0x42, 0x08, 0x42, 0x73, 0x9c, 
    0x63, 0x18, 0x52, 0x94, 0x4a, 0x52, 0x00, 0x00
];

/// Blauwnet Logo
#[rustfmt::skip]
const BLAUWNET_LOGO_RAW: [u8; 16] = [
    0x08, 0x00, 0x08, 0x00, 0x0b, 0xc0, 0x08, 0x60, 
    0x08, 0x20, 0x08, 0x20, 0x04, 0x20, 0x03, 0xa0
];

#[derive(Copy, Clone)]
enum OperatorLogo {
    NS,
    RRR,
    Blauwnet,
    None,
}

#[derive(Copy, Clone)]
enum TopLeftDisplay {
    Countdown,
    DepTime,
    CurrentTime,
}

struct Departure {
    dep_time: &'static str,
    current_time: &'static str,
    minutes_left: u8,
    display_name: &'static str,
    logo: OperatorLogo,
    destination: &'static str,
    via_route: &'static str,
    coupled_sets: [u8; 2],
    travel_left: bool,
    next_train_info: &'static str,
    track: &'static str,
    show_track: bool,
}

#[main]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    let delay = Delay::new();

    let i2c = I2c::new(
        peripherals.I2C0,
        I2cConfig::default().with_frequency(Rate::from_khz(400)),
    )
    .expect("Failed to initialize I2C")
    .with_sda(peripherals.GPIO21)
    .with_scl(peripherals.GPIO22);

    let interface = I2CDisplayInterface::new(i2c);
    let mut display: Ssd1306<_, _, BufferedGraphicsMode<_>> =
        Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
            .into_buffered_graphics_mode();

    display.init().unwrap();
    display.clear(BinaryColor::Off).unwrap();
    display.flush().unwrap();

        let departures = [
        Departure {
            dep_time: "08:12",
            current_time: "08:08",
            minutes_left: 4,
            display_name: "SPR",
            logo: OperatorLogo::NS,
            destination: "Amersfoort Vathorst",
            via_route: "Amersfoort Schothorst",
            coupled_sets: [3, 4],
            travel_left: true,
            next_train_info: "08:22 IC Enschede",
            track: "1",
            show_track: true,
        },
        Departure {
            dep_time: "08:18",
            current_time: "08:08",
            minutes_left: 10,
            display_name: "RS34",
            logo: OperatorLogo::RRR,
            destination: "Barneveld Zuid",
            via_route: "Hoevelaken, Barneveld Noord, Barneveld Centrum",
            coupled_sets: [4, 4],
            travel_left: false,
            next_train_info: "08:48 RS34 Ede-Wageningen",
            track: "4b",
            show_track: true,
        },
        Departure {
            dep_time: "08:25",
            current_time: "08:08",
            minutes_left: 17,
            display_name: "ICD",
            logo: OperatorLogo::NS,
            destination: "Schiphol Airport",
            via_route: "Hilversum, Duivendrecht, Amsterdam Z.",
            coupled_sets: [9, 0],
            travel_left: true,
            next_train_info: "08:55 IC Amsterdam Centraal",
            track: "7",
            show_track: true,
        },
    ];

    let small_font = MonoTextStyle::new(&FONT_5X8, BinaryColor::On);
    let med_font = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
    let bold_font = MonoTextStyle::new(&FONT_8X13_BOLD, BinaryColor::On);
    let inv_font = MonoTextStyle::new(&FONT_5X8, BinaryColor::Off);

    let mut screen_mode = 0;

    let mut dest_scroll_x: i32 = 0;
    let mut dest_pause: u32 = CONFIG.scroll_pause_ticks;

    let mut via_scroll_x: i32 = 0;
    let mut via_pause: u32 = CONFIG.scroll_pause_ticks;

    let mut hierna_scroll_x: i32 = 0;
    let mut hierna_pause: u32 = CONFIG.scroll_pause_ticks;

    let mut ov_dest_scroll: [i32; 3] = [0; 3];
    let mut ov_dest_pause: [u32; 3] = [CONFIG.scroll_pause_ticks; 3];
    let mut ov_via_scroll: [i32; 3] = [0; 3];
    let mut ov_via_pause: [u32; 3] = [CONFIG.scroll_pause_ticks; 3];

    let mut tick: u32 = 0;

    loop {
        display.clear(BinaryColor::Off).unwrap();

        if screen_mode < departures.len() {
            // =================================================================
            // TRACK-SPECIFIC CTA DISPLAY
            // =================================================================
            let train = &departures[screen_mode];

            // 1. TOP BAR (Y = 0..11)
            let top_mode = match (tick / CONFIG.toggle_phase_ticks) % 3 {
                0 => TopLeftDisplay::Countdown,
                1 => TopLeftDisplay::DepTime,
                _ => TopLeftDisplay::CurrentTime,
            };

            let mut top_buf = [0u8; 16];
            let top_left_str = match top_mode {
                TopLeftDisplay::Countdown => format_countdown(train.minutes_left, &mut top_buf),
                TopLeftDisplay::DepTime => train.dep_time,
                TopLeftDisplay::CurrentTime => train.current_time,
            };

            Text::new(top_left_str, Point::new(0, 8), med_font)
                .draw(&mut display)
                .unwrap();

            // Dynamic layout calculations for right-aligned items
            let show_track_top_right = ((tick / CONFIG.toggle_phase_ticks) % 2) == 0;
            let track_box_w = (train.track.len() as i32 * 5) + 8;
            let track_origin_x = 128 - track_box_w;

            // Determine right edge limit of the top-right element (Track box or 16px wide logo)
            let right_element_left_x = if show_track_top_right && train.show_track {
                track_origin_x
            } else {
                match train.logo {
                    OperatorLogo::NS | OperatorLogo::RRR | OperatorLogo::Blauwnet => 128 - 16,
                    OperatorLogo::None => track_origin_x,
                }
            };

            // Right-align train display name precisely to the left of the active right-side element
            let name_len_px = (train.display_name.len() * 5) as i32;
            let name_x = (right_element_left_x - 4 - name_len_px).max(52);
            Text::new(train.display_name, Point::new(name_x, 7), small_font)
                .draw(&mut display)
                .unwrap();

            if show_track_top_right && train.show_track {
                draw_track_box(&mut display, train.track, Point::new(track_origin_x, 0), small_font);
            } else {
                match train.logo {
                    OperatorLogo::NS => {
                        let raw = ImageRawLE::new(&NS_LOGO_RAW, 16);
                        Image::new(&raw, Point::new(128 - 16, 0)).draw(&mut display).unwrap();
                    }
                    OperatorLogo::RRR => {
                        let raw = ImageRawLE::new(&RRR_LOGO_RAW, 16);
                        Image::new(&raw, Point::new(128 - 16, 0)).draw(&mut display).unwrap();
                    }
                    OperatorLogo::Blauwnet => {
                        let raw = ImageRawLE::new(&BLAUWNET_LOGO_RAW, 16);
                        Image::new(&raw, Point::new(128 - 16, 0)).draw(&mut display).unwrap();
                    }
                    OperatorLogo::None => {
                        if train.show_track {
                            draw_track_box(&mut display, train.track, Point::new(track_origin_x, 0), small_font);
                        }
                    }
                }
            }

            // 2. MAIN DESTINATION (Y = 11..22)
            let dest_px = (train.destination.len() * 8) as i32;
            update_popback_scroll(&mut dest_scroll_x, &mut dest_pause, dest_px, 128);

            Text::new(train.destination, Point::new(-dest_scroll_x, 21), bold_font)
                .draw(&mut display)
                .unwrap();

            // 3. VIA ROUTE (Y = 24..31)
            let via_px = (train.via_route.len() * 5) as i32;
            let available_via_w = 128 - 20;
            update_popback_scroll(&mut via_scroll_x, &mut via_pause, via_px, available_via_w);

            Text::new(train.via_route, Point::new(20 - via_scroll_x, 31), small_font)
                .draw(&mut display)
                .unwrap();

            Rectangle::new(Point::new(0, 24), Size::new(20, 8))
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
                .draw(&mut display)
                .unwrap();

            Text::new("via ", Point::new(0, 31), small_font)
                .draw(&mut display)
                .unwrap();

            // 4. CARRIAGES (Y = 41..50)
            let car_w = 12i32;
            let car_h = 9i32;
            let car_gap = 1i32;
            let set_gap = 4i32;

            let mut total_train_w = 0i32;
            let mut active_set_count = 0;
            let mut first_active_idx = None;
            let mut last_active_idx = None;
            let mut last_car_in_last_set = 0;

            for (idx, &car_count) in train.coupled_sets.iter().enumerate() {
                if car_count > 0 {
                    if first_active_idx.is_none() {
                        first_active_idx = Some(idx);
                    }
                    last_active_idx = Some(idx);
                    last_car_in_last_set = car_count - 1;
                    active_set_count += 1;

                    total_train_w += (car_count as i32 * car_w) + ((car_count as i32 - 1) * car_gap);
                }
            }
            if active_set_count > 1 {
                total_train_w += (active_set_count - 1) * set_gap;
            }

            let start_x = (128 - total_train_w) / 2;
            let origin = Point::new(start_x, 41);

            let mut current_x = origin.x;
            let arrow_style = MonoTextStyle::new(&FONT_5X8, BinaryColor::On);
            let mut rendered_sets = 0;

            for (set_idx, &car_count) in train.coupled_sets.iter().enumerate() {
                if car_count == 0 {
                    continue;
                }

                if rendered_sets > 0 {
                    current_x += set_gap;
                }
                rendered_sets += 1;

                for car_i in 0..car_count {
                    let is_first_car = car_i == 0;
                    let is_last_car = car_i == (car_count - 1);

                    let top_left = Point::new(current_x, origin.y);
                    let rect = Rectangle::new(top_left, Size::new(car_w as u32, car_h as u32));
                    let style = PrimitiveStyle::with_stroke(BinaryColor::On, 1);

                    let radii = if is_first_car && is_last_car {
                        CornerRadii {
                            top_left: Size::new(3, 3),
                            top_right: Size::new(3, 3),
                            bottom_right: Size::zero(),
                            bottom_left: Size::zero(),
                        }
                    } else if is_first_car {
                        CornerRadii {
                            top_left: Size::new(3, 3),
                            top_right: Size::zero(),
                            bottom_right: Size::zero(),
                            bottom_left: Size::zero(),
                        }
                    } else if is_last_car {
                        CornerRadii {
                            top_left: Size::zero(),
                            top_right: Size::new(3, 3),
                            bottom_right: Size::zero(),
                            bottom_left: Size::zero(),
                        }
                    } else {
                        CornerRadii {
                            top_left: Size::zero(),
                            top_right: Size::zero(),
                            bottom_right: Size::zero(),
                            bottom_left: Size::zero(),
                        }
                    };

                    RoundedRectangle::new(rect, radii)
                        .into_styled(style)
                        .draw(&mut display)
                        .ok();

                    // Correctly place direction arrow on the leading car based on travel direction
                    let is_leading_car = (train.travel_left && Some(set_idx) == first_active_idx && is_first_car)
                        || (!train.travel_left && Some(set_idx) == last_active_idx && car_i == last_car_in_last_set);

                    if is_leading_car {
                        let arrow_str = if train.travel_left { "<" } else { ">" };
                        Text::new(arrow_str, Point::new(current_x + 3, origin.y + 7), arrow_style)
                            .draw(&mut display)
                            .ok();
                    }

                    current_x += car_w + car_gap;
                }
            }

            // 5. INVERTED BOTTOM BAR (Y = 53..64)
            Rectangle::new(Point::new(0, 53), Size::new(128, 11))
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(&mut display)
                .unwrap();

            let hierna_px = (train.next_train_info.len() * 5) as i32;
            let available_hierna_w = 128 - 41;
            update_popback_scroll(&mut hierna_scroll_x, &mut hierna_pause, hierna_px, available_hierna_w);

            Text::new(train.next_train_info, Point::new(41 - hierna_scroll_x, 60), inv_font)
                .draw(&mut display)
                .unwrap();

            Rectangle::new(Point::new(0, 53), Size::new(41, 11))
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(&mut display)
                .unwrap();

            Text::new("Hierna: ", Point::new(1, 60), inv_font)
                .draw(&mut display)
                .unwrap();

        } else {
            // =================================================================
            // GENERAL DEPARTURE BOARD OVERVIEW SCREEN (3 DEPARTURES)
            // =================================================================
            Rectangle::new(Point::new(0, 0), Size::new(128, 9))
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(&mut display)
                .unwrap();

            Text::new("Vertrek", Point::new(2, 7), inv_font)
                .draw(&mut display)
                .unwrap();

            Text::new("14:21", Point::new(98, 7), inv_font)
                .draw(&mut display)
                .unwrap();

            let y_offsets = [11i32, 28i32, 45i32];

            for (i, train) in departures.iter().enumerate().take(3) {
                let row_y = y_offsets[i];

                let track_box_w = (train.track.len() as i32 * 5) + 8;
                let track_origin_x = 128 - track_box_w;
                let available_ov_w = track_origin_x - 30 - 2;

                // Destination
                let dest_px = (train.destination.len() * 5) as i32;
                update_popback_scroll(&mut ov_dest_scroll[i], &mut ov_dest_pause[i], dest_px, available_ov_w);

                Text::new(train.destination, Point::new(30 - ov_dest_scroll[i], row_y + 6), small_font)
                    .draw(&mut display)
                    .unwrap();

                // Via Summary
                let mut via_buf = [0u8; 32];
                let via_str = format_via_summary(train.via_route, &mut via_buf);
                let via_px = (via_str.len() * 5) as i32;

                update_popback_scroll(&mut ov_via_scroll[i], &mut ov_via_pause[i], via_px, available_ov_w);

                Text::new(via_str, Point::new(30 - ov_via_scroll[i], row_y + 13), small_font)
                    .draw(&mut display)
                    .unwrap();

                // Masking left time zone
                Rectangle::new(Point::new(0, row_y), Size::new(30, 17))
                    .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
                    .draw(&mut display)
                    .unwrap();

                Text::new(train.dep_time, Point::new(0, row_y + 6), small_font)
                    .draw(&mut display)
                    .unwrap();

                // Masking right track box zone
                let right_mask_w = track_box_w + 3;
                Rectangle::new(Point::new(128 - right_mask_w, row_y), Size::new(right_mask_w as u32, 17))
                    .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
                    .draw(&mut display)
                    .unwrap();

                // Track Box
                draw_track_box(&mut display, train.track, Point::new(track_origin_x, row_y), small_font);

                // Dividers safely placed between rows
                if i < 2 {
                    Line::new(Point::new(0, row_y + 17), Point::new(128, row_y + 17))
                        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
                        .draw(&mut display)
                        .unwrap();
                }
            }
        }

        display.flush().unwrap();
        delay.delay_millis(50);

        tick += 1;
        if tick >= CONFIG.screen_dwell_ticks {
            tick = 0;

            dest_scroll_x = 0;
            dest_pause = CONFIG.scroll_pause_ticks;

            via_scroll_x = 0;
            via_pause = CONFIG.scroll_pause_ticks;

            hierna_scroll_x = 0;
            hierna_pause = CONFIG.scroll_pause_ticks;

            ov_dest_scroll = [0; 3];
            ov_dest_pause = [CONFIG.scroll_pause_ticks; 3];
            ov_via_scroll = [0; 3];
            ov_via_pause = [CONFIG.scroll_pause_ticks; 3];

            screen_mode = (screen_mode + 1) % (departures.len() + 1);
        }
    }
}

// -----------------------------------------------------------------------------
// Helper: Draw Track Box with Vertically Centered Text
// -----------------------------------------------------------------------------
fn draw_track_box<D: DrawTarget<Color = BinaryColor>>(
    display: &mut D,
    track: &str,
    origin: Point,
    font: MonoTextStyle<'_, BinaryColor>,
)
{
    let text_w = track.len() as i32 * 5;
    let box_w = text_w + 8;
    let box_h = 12i32;

    let corner_w = 4i32;
    let corner_h = 4i32;

    let style = PrimitiveStyle::with_stroke(BinaryColor::On, 1);
    let x = origin.x;
    let y = origin.y;

    // 1. Solid top-left corner block
    Rectangle::new(Point::new(x, y), Size::new(corner_w as u32, corner_h as u32))
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
        .draw(display)
        .ok();

    // 2. Outline border
    Line::new(Point::new(x + corner_w, y), Point::new(x + box_w - 1, y))
        .into_styled(style)
        .draw(display)
        .ok();
    Line::new(Point::new(x + box_w - 1, y), Point::new(x + box_w - 1, y + box_h - 1))
        .into_styled(style)
        .draw(display)
        .ok();
    Line::new(Point::new(x + box_w - 1, y + box_h - 1), Point::new(x, y + box_h - 1))
        .into_styled(style)
        .draw(display)
        .ok();
    Line::new(Point::new(x, y + box_h - 1), Point::new(x, y + corner_h))
        .into_styled(style)
        .draw(display)
        .ok();

    // 3. Right-aligned and vertically centered text inside box
    let txt_x = x + box_w - text_w - 2;
    let txt_y = y + 9;
    Text::new(track, Point::new(txt_x, txt_y), font)
        .draw(display)
        .ok();
}

fn update_popback_scroll(scroll_x: &mut i32, pause_timer: &mut u32, text_px: i32, view_w: i32) {
    if text_px > view_w {
        let max_scroll = text_px - view_w + 6;
        if *pause_timer > 0 {
            *pause_timer -= 1;
        } else {
            *scroll_x += 1;
            if *scroll_x >= max_scroll {
                *pause_timer = CONFIG.scroll_pause_ticks;
                *scroll_x = 0;
            }
        }
    } else {
        *scroll_x = 0;
    }
}

fn format_countdown<'a>(min: u8, buf: &'a mut [u8; 16]) -> &'a str {
    let mut len = 0;
    if min >= 10 {
        buf[len] = b'0' + (min / 10);
        len += 1;
    }
    buf[len] = b'0' + (min % 10);
    len += 1;

    let suffix = b" minuten";
    buf[len..len + suffix.len()].copy_from_slice(suffix);
    len += suffix.len();

    core::str::from_utf8(&buf[..len]).unwrap_or("4 min")
}

fn format_via_summary<'a>(route: &str, buf: &'a mut [u8; 32]) -> &'a str {
    let prefix = b"via ";
    buf[..4].copy_from_slice(prefix);

    let route_bytes = route.as_bytes();
    let copy_len = route_bytes.len().min(27);
    buf[4..4 + copy_len].copy_from_slice(&route_bytes[..copy_len]);

    core::str::from_utf8(&buf[..4 + copy_len]).unwrap_or("via ...")
}