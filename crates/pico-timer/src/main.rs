#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]

use crate::switch_descriptor::{SwitchButton, SwitchGamepadDescriptor};
use cyw43::{JoinOptions, aligned_bytes};
use cyw43_pio::{DEFAULT_CLOCK_DIVIDER, PioSpi};
use defmt::*;
use embassy_executor::Spawner;
use embassy_net::{Config, Stack, StackResources};
use embassy_rp::clocks::RoscRng;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{DMA_CH0, PIO0, USB};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::{bind_interrupts, dma};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Timer};
use embassy_usb::class::hid::{
    Config as HidConfig, HidBootProtocol, HidSubclass, HidWriter, State as HidState,
};
use embassy_usb::{Builder as UsbBuilder, Config as UsbConfig};
use picoserve::routing::get;
use picoserve::{AppBuilder, AppRouter};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

mod switch_descriptor;

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
    Hold(SwitchButton, u64), // button, milliseconds
}

static KEY_CHANNEL: Channel<CriticalSectionRawMutex, KeyEvent, 4> = Channel::new();

static USB_STATE: StaticCell<HidState<'static>> = StaticCell::new();
static USB_CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static USB_BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static USB_CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

#[embassy_executor::task]
async fn usb_task(mut usb: embassy_usb::UsbDevice<'static, UsbDriver>) -> ! {
    usb.run().await
}

#[embassy_executor::task]
async fn hid_task(mut writer: HidWriter<'static, UsbDriver, 8>) -> ! {
    use embassy_futures::select::{Either, select};

    loop {
        // Wait for USB host to enumerate the device.
        writer.ready().await;

        // L+R handshake — same as tud_mount_cb in the C firmware.
        // The Switch uses this to transition from the controller-select screen.
        let mut handshake = SwitchGamepadDescriptor::neutral();
        handshake.buttons = SwitchButton::BtnL as u16 | SwitchButton::BtnR as u16;
        let _ = writer.write(handshake.as_bytes()).await;
        Timer::after(Duration::from_millis(120)).await;
        let _ = writer.write(SwitchGamepadDescriptor::neutral().as_bytes()).await;

        // The Switch drops the controller if it stops receiving reports.
        // Send the current state every 8 ms, exactly like the C firmware's 5 ms loop.
        let mut report = SwitchGamepadDescriptor::neutral();
        loop {
            match select(KEY_CHANNEL.receive(), Timer::after(Duration::from_millis(8))).await {
                Either::First(event) => {
                    let (button, hold_ms) = match event {
                        KeyEvent::Tap(btn) => (btn, 50u64),
                        KeyEvent::Hold(btn, ms) => (btn, ms),
                    };
                    report.buttons = button as u16;
                    // Send pressed reports at 8 ms intervals for the full hold duration.
                    let steps = (hold_ms / 8).max(1);
                    for _ in 0..steps {
                        if writer.write(report.as_bytes()).await.is_err() {
                            break;
                        }
                        Timer::after(Duration::from_millis(8)).await;
                    }
                    report = SwitchGamepadDescriptor::neutral();
                    let _ = writer.write(report.as_bytes()).await;
                }
                Either::Second(_) => {
                    // Keep-alive tick — send current state.
                    if writer.write(report.as_bytes()).await.is_err() {
                        // USB disconnected; break to outer loop to re-wait at ready().
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
                get(|| async {
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

    // ---------- USB HID setup ----------
    let usb_driver = embassy_rp::usb::Driver::new(p.USB, Irqs);

    // Must match the HORI Pokken Controller identity that the Switch whitelists.
    let mut usb_config = UsbConfig::new(0x0F0D, 0x0092);
    usb_config.manufacturer = Some("HORI CO.,LTD.");
    usb_config.product = Some("POKKEN CONTROLLER");
    usb_config.serial_number = None;
    usb_config.max_power = 250;
    usb_config.max_packet_size_0 = 64;

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
        request_handler: None,
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

    // ---------- CYW43 / WiFi setup ----------
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

    // StackResources: 2 internal (DNS + DHCP) + WEB_TASK_POOL_SIZE TCP sockets.
    static RESOURCES: StaticCell<StackResources<{ WEB_TASK_POOL_SIZE + 2 }>> = StaticCell::new();
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

    static APP: StaticCell<AppRouter<App>> = StaticCell::new();
    let app = APP.init(App.build_app());

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
