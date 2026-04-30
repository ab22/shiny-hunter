#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]

use cyw43::{JoinOptions, aligned_bytes};
use cyw43_pio::{DEFAULT_CLOCK_DIVIDER, PioSpi};
use defmt::*;
use embassy_executor::Spawner;
use embassy_net::{Config, Stack, StackResources};
use embassy_rp::clocks::RoscRng;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{DMA_CH0, PIO0};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::{bind_interrupts, dma};
use picoserve::routing::get;
use picoserve::{AppBuilder, AppRouter};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>;
});

const WIFI_NETWORK: &str = "Fam. Alvarado 2F";
const WIFI_PASSWORD: &str = "Xchgeax,eax";

const WEB_TASK_POOL_SIZE: usize = 4;

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
        picoserve::Router::new().route("/", get(|| async { "Hello, World!" }))
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
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    let config = Config::dhcpv4(Default::default());
    let seed = rng.next_u64();

    // StackResources<5>` only allocates 5 socket slots total. Embassy-net internally
    // claims 2 of them at startup — 1 for the DNS socket and 1 for the DHCP socket —
    // leaving only 3 free for TCP. But 4 web tasks each try to create a `TcpSocket`,
    // so the 4th one calls into smoltcp which hard-panics with "adding a socket to a
    // full SocketSet". That panic was the cause of the LED staying on permanently.
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
            Err(err) => info!("join failed: {:?}", err),
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
        embassy_time::Timer::after(embassy_time::Duration::from_secs(3600)).await;
    }
}

async fn blink(control: &mut cyw43::Control<'_>, times: u32) {
    for _ in 0..times {
        control.gpio_set(0, true).await;
        embassy_time::Timer::after(embassy_time::Duration::from_millis(400)).await;
        control.gpio_set(0, false).await;
        embassy_time::Timer::after(embassy_time::Duration::from_millis(400)).await;
    }
    embassy_time::Timer::after(embassy_time::Duration::from_millis(800)).await;
}
