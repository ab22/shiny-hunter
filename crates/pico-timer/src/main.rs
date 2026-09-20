#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]

use crate::switch_descriptor::{SwitchButton, SwitchGamepadDescriptor, SwitchHatValues};
use cyw43::{JoinOptions, aligned_bytes};
use cyw43_pio::{DEFAULT_CLOCK_DIVIDER, PioSpi};
use defmt::*;
use embassy_executor::Spawner;
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::{Config, IpAddress, IpEndpoint, Stack, StackResources};
use embassy_rp::clocks::RoscRng;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{DMA_CH0, PIO0, USB};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::{bind_interrupts, dma};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Instant, Timer};
use embassy_usb::class::hid::{
    Config as HidConfig, HidBootProtocol, HidSubclass, HidWriter, ReportId, RequestHandler,
    State as HidState,
};
use embassy_usb::control::OutResponse;
use embassy_usb::{Builder as UsbBuilder, Config as UsbConfig, UsbVersion};
use picoserve::request::RequestParts;
use picoserve::routing::get;
use picoserve::{AppBuilder, AppRouter};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

mod router;
mod switch_descriptor;

struct SwitchHidHandler;

impl RequestHandler for SwitchHidHandler {
    fn get_report(&mut self, _id: ReportId, buf: &mut [u8]) -> Option<usize> {
        buf.fill(0);
        Some(buf.len())
    }

    fn set_report(&mut self, _id: ReportId, _data: &[u8]) -> OutResponse {
        OutResponse::Accepted
    }
}

static HID_HANDLER: StaticCell<SwitchHidHandler> = StaticCell::new();

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>;
    USBCTRL_IRQ => embassy_rp::usb::InterruptHandler<USB>;
});

const WIFI_NETWORK: &str = "Fam. Alvarado 2F";
const WIFI_PASSWORD: &str = "Xchgeax,eax";

const WEB_TASK_POOL_SIZE: usize = 4;

// ---------- USB HID ----------

type UsbDriver = embassy_rp::usb::Driver<'static, USB>;

enum KeyEvent {
    Tap(SwitchButton),
    Hold(SwitchButton, u64),    // button, milliseconds
    DPad(SwitchHatValues, u64), // direction, milliseconds
    CharCreation,
}

static KEY_CHANNEL: Channel<CriticalSectionRawMutex, KeyEvent, 4> = Channel::new();
static LOG_CHANNEL: Channel<CriticalSectionRawMutex, &'static str, 16> = Channel::new();

macro_rules! ulog {
    ($msg:literal) => {
        LOG_CHANNEL.try_send($msg).ok();
    };
}

static USB_STATE: StaticCell<HidState<'static>> = StaticCell::new();
static USB_CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static USB_BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static USB_CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

#[embassy_executor::task]
async fn usb_task(mut usb: embassy_usb::UsbDevice<'static, UsbDriver>) -> ! {
    usb.run().await
}

#[embassy_executor::task]
async fn log_task(stack: Stack<'static>) -> ! {
    let mut rx_meta = [PacketMetadata::EMPTY; 2];
    let mut rx_buf = [0u8; 64];
    let mut tx_meta = [PacketMetadata::EMPTY; 4];
    let mut tx_buf = [0u8; 512];
    let mut socket = UdpSocket::new(stack, &mut rx_meta, &mut rx_buf, &mut tx_meta, &mut tx_buf);
    socket.bind(6001).ok();
    let dest = IpEndpoint::new(IpAddress::v4(255, 255, 255, 255), 5000);
    loop {
        let msg = LOG_CHANNEL.receive().await;
        let _ = socket.send_to(msg.as_bytes(), dest).await;
    }
}

#[embassy_executor::task]
async fn hid_task(mut writer: HidWriter<'static, UsbDriver, 8>) -> ! {
    use embassy_futures::select::{Either, select};

    loop {
        // Wait for USB host to enumerate the device.
        ulog!("USB_WAITING");
        writer.ready().await;
        ulog!("USB_READY");

        // L then R separately, exactly as tud_mount_cb does in the C firmware.
        // Pressing them simultaneously doesn't work in the NSO GBA emulator.
        let neutral = SwitchGamepadDescriptor::neutral();
        let mut press_l = SwitchGamepadDescriptor::neutral();
        press_l.buttons = SwitchButton::BtnL as u16;
        let mut press_r = SwitchGamepadDescriptor::neutral();
        press_r.buttons = SwitchButton::BtnR as u16;

        let _ = writer.write(press_l.as_bytes()).await;
        Timer::after(Duration::from_millis(120)).await;
        let _ = writer.write(neutral.as_bytes()).await;
        let _ = writer.write(press_r.as_bytes()).await;
        Timer::after(Duration::from_millis(120)).await;
        let _ = writer.write(neutral.as_bytes()).await;

        // Send reports every 5 ms, matching the C firmware's main loop rate.
        let mut report = SwitchGamepadDescriptor::neutral();
        const TAP_DURATION_MS: u64 = 50u64;
        loop {
            match select(
                KEY_CHANNEL.receive(),
                Timer::after(Duration::from_millis(5)),
            )
            .await
            {
                Either::First(event) => {
                    ulog!("CMD_RECV");
                    match event {
                        KeyEvent::Tap(btn) => {
                            ulog!("CMD_TAP");
                            report.buttons = btn as u16;
                            let steps = (TAP_DURATION_MS / 5).max(1);

                            // Send pressed reports at 8 ms intervals for the full hold duration.
                            for _ in 0..steps {
                                if writer.write(report.as_bytes()).await.is_err() {
                                    break;
                                }

                                Timer::after(Duration::from_millis(5)).await;
                            }

                            report = SwitchGamepadDescriptor::neutral();
                            let _ = writer.write(report.as_bytes()).await;
                        }
                        KeyEvent::Hold(btn, ms) => {
                            report.buttons = btn as u16;
                            let steps = (ms / 5).max(1);

                            // Send pressed reports at 8 ms intervals for the full hold duration.
                            for _ in 0..steps {
                                if writer.write(report.as_bytes()).await.is_err() {
                                    break;
                                }

                                Timer::after(Duration::from_millis(5)).await;
                            }

                            report = SwitchGamepadDescriptor::neutral();
                            let _ = writer.write(report.as_bytes()).await;
                        }
                        KeyEvent::DPad(dir, ms) => {
                            report.hat = dir as u8;
                            let steps = (ms / 5).max(1);

                            // Send pressed reports at 8 ms intervals for the full hold duration.
                            for _ in 0..steps {
                                if writer.write(report.as_bytes()).await.is_err() {
                                    break;
                                }

                                Timer::after(Duration::from_millis(5)).await;
                            }

                            report = SwitchGamepadDescriptor::neutral();
                            let _ = writer.write(report.as_bytes()).await;
                        }
                        KeyEvent::CharCreation => {
                            // Press A for character creation.
                            report.buttons = SwitchButton::BtnA as u16;
                            let steps = (TAP_DURATION_MS / 5).max(1);

                            // Start the deadline the moment A is pressed.
                            let second_press_at = Instant::now() + Duration::from_millis(25114);
                            for _ in 0..steps {
                                if writer.write(report.as_bytes()).await.is_err() {
                                    break;
                                }
                                Timer::after(Duration::from_millis(5)).await;
                            }

                            report = SwitchGamepadDescriptor::neutral();
                            let _ = writer.write(report.as_bytes()).await;

                            // Keep sending neutral reports every 8ms until the deadline.
                            // A plain Timer::at() would starve the HID keep-alive and drop the controller.
                            while Instant::now() < second_press_at {
                                if writer.write(report.as_bytes()).await.is_err() {
                                    break;
                                }
                                Timer::after(Duration::from_millis(5)).await;
                            }

                            // Press A for "is about to unfold" message.
                            report.buttons = SwitchButton::BtnA as u16;
                            for _ in 0..steps {
                                if writer.write(report.as_bytes()).await.is_err() {
                                    break;
                                }
                                Timer::after(Duration::from_millis(5)).await;
                            }

                            report = SwitchGamepadDescriptor::neutral();
                            let _ = writer.write(report.as_bytes()).await;
                        }
                    };
                }
                Either::Second(_) => {
                    // Keep-alive tick — send current state.
                    if writer.write(report.as_bytes()).await.is_err() {
                        ulog!("USB_KEEPALIVE_ERR");
                        break;
                    }
                }
            }
        }
    }
}

// ---------- WiFi / HTTP ----------

#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<'static, cyw43::SpiBus<Output<'static>, PioSpi<'static, PIO0, 0>>>,
) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}

struct App;

impl AppBuilder for App {
    type PathRouter = impl picoserve::routing::PathRouter;

    fn build_app(self) -> picoserve::Router<Self::PathRouter> {
        picoserve::Router::new()
            .route(
                "/",
                get(|r: RequestParts<'_>| async {
                    let query = r.query();

                    KEY_CHANNEL.send(KeyEvent::Tap(SwitchButton::BtnA)).await;
                    "OK"
                }),
            )
            .route(
                "/hold",
                get(|| async {
                    KEY_CHANNEL
                        .send(KeyEvent::Hold(SwitchButton::BtnA, 2000))
                        .await;
                    "OK"
                }),
            )
            .route(
                "/dpad/up",
                get(|| async {
                    KEY_CHANNEL
                        .send(KeyEvent::DPad(SwitchHatValues::Up, 2000))
                        .await;
                    "OK"
                }),
            )
            .route(
                "/dpad/down",
                get(|| async {
                    KEY_CHANNEL
                        .send(KeyEvent::DPad(SwitchHatValues::Down, 2000))
                        .await;
                    "OK"
                }),
            )
            .route(
                "/dpad/left",
                get(|| async {
                    KEY_CHANNEL
                        .send(KeyEvent::DPad(SwitchHatValues::Left, 2000))
                        .await;
                    "OK"
                }),
            )
            .route(
                "/dpad/right",
                get(|| async {
                    KEY_CHANNEL
                        .send(KeyEvent::DPad(SwitchHatValues::Right, 2000))
                        .await;
                    "OK"
                }),
            )
            .route("/status", get(|| async { "ONLINE" }))
    }
}

static APP_CONFIG: picoserve::Config = picoserve::Config::const_default().keep_connection_alive();

#[embassy_executor::task(pool_size = WEB_TASK_POOL_SIZE)]
async fn web_task(task_id: usize, stack: Stack<'static>, app: &'static AppRouter<App>) -> ! {
    let mut tcp_rx_buffer = [0; 1024];
    let mut tcp_tx_buffer = [0; 1024];
    let mut http_buffer = [0; 2048];

    picoserve::Server::new(app, &APP_CONFIG, &mut http_buffer)
        .listen_and_serve(task_id, stack, 80, &mut tcp_rx_buffer, &mut tcp_tx_buffer)
        .await
        .into_never()
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut rng = RoscRng;

    // ---------- CYW43 / WiFi setup ----------
    // USB is intentionally started AFTER WiFi, matching the C firmware (tusb_init is called
    // after cyw43_arch_wifi_connect). The GBA NSO app rejects controllers that were enumerated
    // before the game session started, but accepts ones that appear after it is running.
    let fw = aligned_bytes!("../firmware/cyw43/43439A0.bin");
    let clm = aligned_bytes!("../firmware/cyw43/43439A0_clm.bin");
    let nvram = aligned_bytes!("../firmware/cyw43/nvram_rp2040.bin");

    let pwr = Output::new(p.PIN_23, Level::Low);
    let cs = Output::new(p.PIN_25, Level::High);
    let mut pio = Pio::new(p.PIO0, Irqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        DEFAULT_CLOCK_DIVIDER,
        pio.irq0,
        cs,
        p.PIN_24,
        p.PIN_29,
        dma::Channel::new(p.DMA_CH0, Irqs),
    );

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw, nvram).await;
    spawner.spawn(unwrap!(cyw43_task(runner)));

    control.init(clm).await;
    // PowerSave lets the chip sleep between transmissions — bad for a server that
    // must respond promptly. Use None (always-on) for reliable HTTP serving.
    control
        .set_power_management(cyw43::PowerManagementMode::None)
        .await;

    let config = Config::dhcpv4(Default::default());
    let seed = rng.next_u64();

    // StackResources: 2 internal (DNS + DHCP) + WEB_TASK_POOL_SIZE TCP + 1 UDP log socket.
    static RESOURCES: StaticCell<StackResources<{ WEB_TASK_POOL_SIZE + 3 }>> = StaticCell::new();
    let (stack, runner) = embassy_net::new(
        net_device,
        config,
        RESOURCES.init(StackResources::new()),
        seed,
    );

    spawner.spawn(unwrap!(net_task(runner)));

    // Checkpoint 1: CYW43 init done — 1 slow blink
    blink(&mut control, 1).await;

    loop {
        match control
            .join(WIFI_NETWORK, JoinOptions::new(WIFI_PASSWORD.as_bytes()))
            .await
        {
            Ok(_) => break,
            Err(err) => {
                info!("join failed: {:?}", err);
                Timer::after(Duration::from_secs(1)).await;
            }
        }
    }

    // Checkpoint 2: WiFi joined — 2 slow blinks
    blink(&mut control, 2).await;

    stack.wait_link_up().await;
    stack.wait_config_up().await;

    // Checkpoint 3: DHCP up — 3 slow blinks
    blink(&mut control, 3).await;

    // ---------- USB HID setup (after WiFi, matching C firmware init order) ----------
    let usb_driver = embassy_rp::usb::Driver::new(p.USB, Irqs);

    let mut usb_config = UsbConfig::new(0x0F0D, 0x0092);
    usb_config.manufacturer = Some("HORI CO.,LTD.");
    usb_config.product = Some("POKKEN CONTROLLER");
    usb_config.serial_number = None;
    usb_config.max_power = 250;
    usb_config.max_packet_size_0 = 64;
    usb_config.device_release = 0x0100;
    usb_config.bcd_usb = UsbVersion::Two;

    let mut builder = UsbBuilder::new(
        usb_driver,
        usb_config,
        USB_CONFIG_DESC.init([0; 256]),
        USB_BOS_DESC.init([0; 256]),
        &mut [],
        USB_CONTROL_BUF.init([0; 64]),
    );

    let hid_config = HidConfig {
        report_descriptor: switch_descriptor::DESCRIPTOR,
        request_handler: Some(HID_HANDLER.init(SwitchHidHandler)),
        poll_ms: 5,
        max_packet_size: 64,
        hid_subclass: HidSubclass::No,
        hid_boot_protocol: HidBootProtocol::None,
    };

    let hid_writer =
        HidWriter::<_, 8>::new(&mut builder, USB_STATE.init(HidState::new()), hid_config);

    let usb = builder.build();
    spawner.spawn(unwrap!(usb_task(usb)));
    spawner.spawn(unwrap!(hid_task(hid_writer)));

    // ---------- HTTP + logging ----------
    static APP: StaticCell<AppRouter<App>> = StaticCell::new();
    let app = APP.init(App.build_app());

    spawner.spawn(unwrap!(log_task(stack)));

    for task_id in 0..WEB_TASK_POOL_SIZE {
        spawner.spawn(unwrap!(web_task(task_id, stack, app)));
    }

    loop {
        Timer::after(Duration::from_secs(3600)).await;
    }
}

async fn blink(control: &mut cyw43::Control<'_>, times: u32) {
    for _ in 0..times {
        control.gpio_set(0, true).await;
        Timer::after(Duration::from_millis(400)).await;
        control.gpio_set(0, false).await;
        Timer::after(Duration::from_millis(400)).await;
    }
    Timer::after(Duration::from_millis(800)).await;
}
