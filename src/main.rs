#![no_std]
#![no_main]

extern crate alloc;

mod display_line_buffer_providers;

use alloc::{boxed::Box, rc::Rc};
use display_line_buffer_providers::DrawBuffer;
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    delay::Delay,
    gpio::{self, Output},
    main,
    spi::master::Spi,
    time::RateExtU32,
    timer::systimer::SystemTimer,
};
use gc9a01::{
    mode::DisplayConfiguration,
    prelude::{DisplayResolution240x240, DisplayRotation},
    Gc9a01, SPIDisplayInterface,
};
use slint::platform::software_renderer::MinimalSoftwareWindow;

slint::include_modules!();

#[main]
fn main() -> ! {
    esp_alloc::heap_allocator!(size: 72 * 1024);

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::default());
    let peripherals = esp_hal::init(config);

    let mut delay = Delay::new();

    let sck = peripherals.GPIO10; // GP 10 (LCD_CLK) - Clock signal used to synchronize the timing of data transfer between LCD controller and ESP32
    let mosi = peripherals.GPIO11; // GP 11 (LCD_DIN) - LCD MOSI - Master Out Slave In is the data line that transmits data from ESP32 to LCD (slave)
    let cs = peripherals.GPIO9; // GP 9  (LCD_CS) - LCD Chip Selection - Used in SPI to select which device the ESP32 is communicating with
    let dc = peripherals.GPIO8; // GP 8  (LCD_DC) - LCD Command/Data Selection - This is used to tell the LCD controller whether the data being sent is a command or data for the display.
    let reset = peripherals.GPIO12; // GP 12 (LCD_RST) - LCD Reset - A pin where signal used to reset the LCD Display Controller, ensuring it starts in a known state
    let backlight = peripherals.GPIO40; // GP 40 (LCD_BL) - LCD Backlight Control - A pin that throws signal which controls whether LCD backlight is on or off.

    // ---- Initialization ----

    // Setting the Reset pin voltage to zero (0)
    let mut led_reset = Output::new(reset, gpio::Level::Low);

    // Setting the Chip Selection pin voltage to zero (0)
    let cs_output = Output::new(cs, gpio::Level::Low);

    // Setting the Command/Data Selection pin voltage to zero (0)
    let dc_output = Output::new(dc, gpio::Level::Low);

    // Turning on the Backlight after initialization
    Output::new(backlight, gpio::Level::High);

    // Initializing SPI API of esp_hal through peripherals.SPI2 (ESP32 hardware SPI interface)
    // Typically used when you're talking to devices such as LCD Displays, SD cards, sensors, flash memory chips.
    let spi = Spi::new(
        peripherals.SPI2,
        // Create config for spi peripheral: Clock Polarity = 0, phase = 0
        esp_hal::spi::master::Config::default()
            .with_mode(esp_hal::spi::Mode::_0) // use SPI mode 0 where we idle clock to low
            .with_frequency(20_u32.MHz()), // Set SPI clock to 20 MHz (handy extension method to turn 20 into 20_000_000 Hz.)
    )
    .unwrap() // If it fails, it panicks!
    .with_mosi(mosi) // Setting GPIO (Master Out Slave In)
    .with_sck(sck); // Set clock line

    // Assets the CS (Pull it low) when communication starts, then transfer data
    // over the SPI, and de-asserts CS (Sets it high) when finished
    // No artificial delay between steps, and unwrap or panic if failed
    // Used when we dont want to manually control CS pin
    let spi_dev = ExclusiveDevice::new_no_delay(spi, cs_output).unwrap();

    // This generates generic interface between SPI Peripheral and Gc9a01 (Integrated Display)
    // This creates a transport layer that knows how to communicate with your display over SPI
    let interface = SPIDisplayInterface::new(spi_dev, dc_output);

    // Initialize Display for Gc9a01
    let mut display = Gc9a01::new(
        interface,
        DisplayResolution240x240, // GC9A01 panels are typically 240x240 pixels (square, round display).
        DisplayRotation::Rotate180, // This sets the initial rotation of the display buffer.
    );

    // -- Getting the screen ready to draw --
    display.clear_fit().unwrap(); // This clears the screen with the current display rotation and resolution in mind.
    display.reset(&mut led_reset, &mut delay).unwrap(); // This triggers a hardware reset of the display using the reset (RST) pin.
    display.init(&mut delay).unwrap(); // This sends the full initialization command sequence to the GC9A01 controller over SPI.

    log::info!("Driver configured!");

    // Slint
    // Let's start drawing something using Slint.

    // This creates a minimal off-screen framebuffer window.
    // This uses Slint to draw UI pixels in RAM.
    let window = MinimalSoftwareWindow::new(Default::default());
    window.set_size(slint::PhysicalSize::new(240 as _, 240 as _));

    // Registers rendering backend with Slint.
    slint::platform::set_platform(Box::new(EspBackend {
        window: window.clone(),
    }))
    .unwrap();

    // Initialize the Slint module (slint::include_modules!)
    let _app_window = AppWindow::new().unwrap();

    // Allocates one line (row) of RGB565 pixels.
    // Used as a scratch buffer to hold each rendered scanline before writing it to the screen.
    let mut line_buffer = [slint::platform::software_renderer::Rgb565Pixel(0); 240 as usize];

    loop {
        // This updates internal Slint timers, animations, and transitions.
        slint::platform::update_timers_and_animations();

        // Only re-renders if something changed (dirty region, animation, etc.).
        window.draw_if_needed(|renderer| {
            renderer.render_by_line(DrawBuffer {
                display: &mut display,
                line_buffer: &mut line_buffer,
            });
        });

        // This checks if Slint has any animations running.
        // If there are no animations, you might choose to sleep, yield, or idle the CPU.
        if window.has_active_animations() {
            continue;
        }
    }
}

// A custom backend that implements the Slint Platform trait.
struct EspBackend {
    window: Rc<MinimalSoftwareWindow>,
}

impl slint::platform::Platform for EspBackend {
    // "Hey, use this struct as your platform implementation."
    // This method is called when Slint wants to create a window (usually just one in embedded).
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }

    // This tells Slint how much time has passed since the application started.
    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(
            esp_hal::time::now().ticks() / (SystemTimer::ticks_per_second() / 1000),
        )
    }
}
