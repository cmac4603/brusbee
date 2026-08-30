#![no_std]
#![no_main]

use core::{net::Ipv4Addr, str::FromStr};

use brusbee::web::{AppProps, WEB_TASK_POOL_SIZE, web_task};
use embassy_executor::Spawner;
use embassy_net::{Ipv4Cidr, Runner, Stack, StackResources, StaticConfigV4};
use embassy_time::{Delay, Duration, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_sdmmc::SdCard;
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::gpio::{Level, Output, OutputConfig, Pull};
use esp_hal::time::Rate;
use esp_hal::{Async, spi};
use esp_hal::{
    clock::CpuClock, interrupt::software::SoftwareInterruptControl, ram, rng::Rng,
    spi::master::Spi, timer::timg::TimerGroup,
};
use esp_println::println;
use esp_radio::wifi::{Config, ControllerConfig, Interface, WifiController, ap::AccessPointConfig};
use picoserve::{AppBuilder, AppRouter, make_static};

esp_bootloader_esp_idf::esp_app_desc!();

const GW_IP_ADDR_ENV: &str = "1.2.3.4";
const SSID: &str = "Brusbee";

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    esp_println::logger::init_logger_from_env();
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[ram(reclaimed)] size: 64 * 1024);
    esp_alloc::heap_allocator!(size: 36 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_int = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_int.software_interrupt0);

    let spi_bus = Spi::new(
        peripherals.SPI2,
        spi::master::Config::default()
            .with_frequency(Rate::from_khz(400))
            .with_mode(spi::Mode::_0),
    )
    .unwrap()
    .with_sck(peripherals.GPIO12) // clk
    .with_mosi(peripherals.GPIO16) // cmd
    .with_miso(peripherals.GPIO14) // d0
    .into_async();

    let sd_cs = Output::new(
        peripherals.GPIO18, // d3
        Level::High,
        OutputConfig::default().with_pull(Pull::Up),
    );
    let spi_dev = ExclusiveDevice::new(spi_bus, sd_cs, Delay).unwrap();
    let sdcard = SdCard::new(spi_dev, Delay);

    println!("Init SD card controller and retrieve card size...");
    let sd_size = sdcard.num_bytes().unwrap();
    println!("Card size is {} bytes", sd_size);

    let access_point_config = Config::AccessPoint(AccessPointConfig::default().with_ssid(SSID));

    println!("Starting wifi");
    let device = esp_radio::wifi::Interface::access_point();
    let controller = esp_radio::wifi::WifiController::new(
        peripherals.WIFI,
        ControllerConfig::default().with_initial_config(access_point_config),
    )
    .expect("wifi should start up fine");
    println!("Wifi started!");

    let gw_ip_addr = Ipv4Addr::from_str(GW_IP_ADDR_ENV).expect("failed to parse gateway ip");

    let config = embassy_net::Config::ipv4_static(StaticConfigV4 {
        address: Ipv4Cidr::new(gw_ip_addr, 24),
        gateway: Some(gw_ip_addr),
        dns_servers: Default::default(),
    });

    let rng = Rng::new();
    let seed = (rng.random() as u64) << 32 | rng.random() as u64;

    // initialize network stack
    let (stack, runner) = embassy_net::new(
        device,
        config,
        // 1 socket for dns, 1 for dhcp and 2 for the webserver
        // 1 + 1 + 2 = 4 :bicep:
        make_static!(StackResources<4>, StackResources::<4>::new()),
        seed,
    );

    spawner.spawn(
        connection(controller, sdcard)
            .expect("station connection events to access point event should work"),
    );
    spawner.spawn(net_task(runner).expect("wifi network stack should run"));
    spawner.spawn(
        run_dhcp(stack, GW_IP_ADDR_ENV)
            .expect("dhcp server should work"),
    );

    println!("Connect to the AP `{SSID}` and point your browser to http://{GW_IP_ADDR_ENV}:8080/");
    println!("DHCP is enabled so there's no need to configure a static IP, just in case:");

    stack.wait_config_up().await;

    stack
        .config_v4()
        .inspect(|c| println!("ipv4 config: {c:?}"));

    let app = make_static!(AppRouter<AppProps>, AppProps.build_app());

    for task_id in 0..WEB_TASK_POOL_SIZE {
        spawner.spawn(web_task(task_id, stack, app).unwrap());
    }
}

#[embassy_executor::task]
async fn run_dhcp(stack: Stack<'static>, gw_ip_addr: &'static str) {
    use core::net::{Ipv4Addr, SocketAddrV4};

    use edge_dhcp::{
        io::{self, DEFAULT_SERVER_PORT},
        server::{Server, ServerOptions},
    };
    use edge_nal::UdpBind;
    use edge_nal_embassy::{Udp, UdpBuffers};

    let ip = Ipv4Addr::from_str(gw_ip_addr).expect("dhcp task failed to parse gw ip");

    let mut buf = [0u8; 1500];

    let mut gw_buf = [Ipv4Addr::UNSPECIFIED];

    let buffers = UdpBuffers::<3, 1024, 1024, 10>::new();
    let unbound_socket = Udp::new(stack, &buffers);
    let mut bound_socket = unbound_socket
        .bind(core::net::SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::UNSPECIFIED,
            DEFAULT_SERVER_PORT,
        )))
        .await
        .unwrap();

    loop {
        _ = io::server::run(
            &mut Server::<_, 64>::new_with_et(ip),
            &ServerOptions::new(ip, Some(&mut gw_buf)),
            &mut bound_socket,
            &mut buf,
        )
        .await
        .inspect_err(|e| log::warn!("DHCP server error: {e:?}"));
        Timer::after(Duration::from_millis(500)).await;
    }
}

#[embassy_executor::task]
async fn connection(
    controller: WifiController<'static>,
    sdcard: SdCard<ExclusiveDevice<Spi<'static, Async>, Output<'static>, Delay>, Delay>,
) {
    println!("Start connection task");
    loop {
        let ev = controller
            .wait_for_access_point_connected_event_async()
            .await;
        match ev {
            Ok(esp_radio::wifi::ap::EventInfo::Connected(info)) => {
                println!("Station connected: {:?}", info);
            }
            Ok(esp_radio::wifi::ap::EventInfo::Disconnected(info)) => {
                println!("Station disconnected: {:?}", info);
            }
            _ => (),
        }
        Timer::after(Duration::from_millis(5000)).await
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, Interface>) {
    runner.run().await
}
