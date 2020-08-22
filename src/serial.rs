//! Serial

use core::fmt;
use core::marker::PhantomData;
use core::ptr;

use embedded_hal::blocking::serial as serial_block;
use embedded_hal::prelude::*;
use embedded_hal::serial;
use nb::block;

use crate::stm32;
use crate::stm32::rcc::{d2ccip2r, d3ccipr};
use crate::stm32::usart1::cr1::{M0_A as M0, PCE_A as PCE, PS_A as PS};
use stm32h7::Variant::Val;

use crate::stm32::LPUART1;
use crate::stm32::{UART4, UART5, UART7, UART8};
use crate::stm32::{USART1, USART2, USART3, USART6};

use crate::gpio::gpioa::{
    PA0, PA1, PA10, PA11, PA12, PA15, PA2, PA3, PA4, PA8, PA9,
};
use crate::gpio::gpiob::{
    PB10, PB11, PB12, PB13, PB14, PB15, PB3, PB4, PB5, PB6, PB7, PB8, PB9,
};
use crate::gpio::gpioc::{PC10, PC11, PC12, PC6, PC7, PC8};
use crate::gpio::gpiod::{PD0, PD1, PD10, PD2, PD5, PD6, PD7, PD8, PD9};
use crate::gpio::gpioe::{PE0, PE1, PE7, PE8};
use crate::gpio::gpiof::{PF6, PF7};
use crate::gpio::gpiog::{PG14, PG7, PG9};
use crate::gpio::gpioh::{PH13, PH14};
use crate::gpio::gpioi::PI9;
use crate::gpio::gpioj::{PJ8, PJ9};

use crate::gpio::{Alternate, AF11, AF14, AF3, AF4, AF6, AF7, AF8};
use crate::rcc::{rec, CoreClocks, ResetEnable};
use crate::time::Hertz;

use crate::Never;

/// Serial error
#[derive(Debug)]
pub enum Error {
    /// Framing error
    Framing,
    /// Noise error
    Noise,
    /// RX buffer overrun
    Overrun,
    /// Parity check error
    Parity,
    #[doc(hidden)]
    _Extensible,
}

/// Interrupt event
pub enum Event {
    /// New data has been received
    Rxne,
    /// New data can be sent
    Txe,
    /// Idle line state detected
    Idle,
}

pub mod config {
    use crate::time::Hertz;

    pub enum WordLength {
        DataBits8,
        DataBits9,
    }

    pub enum Parity {
        ParityNone,
        ParityEven,
        ParityOdd,
    }

    pub enum StopBits {
        #[doc = "1 stop bit"]
        STOP1,
        #[doc = "0.5 stop bits"]
        STOP0P5,
        #[doc = "2 stop bits"]
        STOP2,
        #[doc = "1.5 stop bits"]
        STOP1P5,
    }

    pub struct Config {
        pub baudrate: Hertz,
        pub wordlength: WordLength,
        pub parity: Parity,
        pub stopbits: StopBits,
    }

    impl Config {
        pub fn baudrate(mut self, baudrate: impl Into<Hertz>) -> Self {
            self.baudrate = baudrate.into();
            self
        }

        pub fn parity_none(mut self) -> Self {
            self.parity = Parity::ParityNone;
            self
        }

        pub fn parity_even(mut self) -> Self {
            self.parity = Parity::ParityEven;
            self
        }

        pub fn parity_odd(mut self) -> Self {
            self.parity = Parity::ParityOdd;
            self
        }

        pub fn wordlength_8(mut self) -> Self {
            self.wordlength = WordLength::DataBits8;
            self
        }

        pub fn wordlength_9(mut self) -> Self {
            self.wordlength = WordLength::DataBits9;
            self
        }

        pub fn stopbits(mut self, stopbits: StopBits) -> Self {
            self.stopbits = stopbits;
            self
        }
    }

    #[derive(Debug)]
    pub struct InvalidConfig;

    impl Default for Config {
        fn default() -> Config {
            Config {
                baudrate: Hertz(19_200), // 19k2 baud
                wordlength: WordLength::DataBits8,
                parity: Parity::ParityNone,
                stopbits: StopBits::STOP1,
            }
        }
    }

    impl<T: Into<Hertz>> From<T> for Config {
        fn from(f: T) -> Config {
            Config {
                baudrate: f.into(),
                ..Default::default()
            }
        }
    }
}

pub trait Pins<USART> {}

pub trait PinTx<USART> {}

pub trait PinRx<USART> {}

pub trait PinCk<USART> {}

impl<USART, TX, RX> Pins<USART> for (TX, RX)
where
    TX: PinTx<USART>,
    RX: PinRx<USART>,
{
}

/// A filler type for when the Tx pin is unnecessary
pub struct NoTx;

/// A filler type for when the Rx pin is unnecessary
pub struct NoRx;

/// A filler type for when the Ck pin is unnecessary
pub struct NoCk;

macro_rules! usart_pins {
    ($($USARTX:ty: TX: [$($TX:ty),*] RX: [$($RX:ty),*] CK: [$($CK:ty),*])+) => {
        $(
            $(
                impl PinTx<$USARTX> for $TX {}
            )*
            $(
                impl PinRx<$USARTX> for $RX {}
            )*
            $(
                impl PinCk<$USARTX> for $CK {}
            )*
        )+
    }
}
macro_rules! uart_pins {
    ($($UARTX:ty: TX: [$($TX:ty),*] RX: [$($RX:ty),*])+) => {
        $(
            $(
                impl PinTx<$UARTX> for $TX {}
            )*
            $(
                impl PinRx<$UARTX> for $RX {}
            )*
        )+
    }
}

usart_pins! {
    USART1:
        TX: [
            NoTx,
            PA9<Alternate<AF7>>,
            PB6<Alternate<AF7>>,
            PB14<Alternate<AF4>>
        ]
        RX: [
            NoRx,
            PA10<Alternate<AF7>>,
            PB7<Alternate<AF7>>,
            PB15<Alternate<AF4>>
        ]
        CK: [
            NoCk,
            PA8<Alternate<AF7>>
        ]
    USART2:
        TX: [
            NoTx,
            PA2<Alternate<AF7>>,
            PD5<Alternate<AF7>>
        ]
        RX: [
            NoRx,
            PA3<Alternate<AF7>>,
            PD6<Alternate<AF7>>
        ]
        CK: [
            NoCk,
            PA4<Alternate<AF7>>,
            PD7<Alternate<AF7>>
        ]
    USART3:
        TX: [
            NoTx,
            PB10<Alternate<AF7>>,
            PC10<Alternate<AF7>>,
            PD8<Alternate<AF7>>
        ]
        RX: [
            NoRx,
            PB11<Alternate<AF7>>,
            PC11<Alternate<AF7>>,
            PD9<Alternate<AF7>>
        ]
        CK: [
            NoCk,
            PB12<Alternate<AF7>>,
            PC12<Alternate<AF7>>,
            PD10<Alternate<AF7>>
        ]
    USART6:
        TX: [
            NoTx,
            PC6<Alternate<AF7>>,
            PG14<Alternate<AF7>>
        ]
        RX: [
            NoRx,
            PC7<Alternate<AF7>>,
            PG9<Alternate<AF7>>
        ]
        CK: [
            NoCk,
            PC8<Alternate<AF7>>,
            PG7<Alternate<AF7>>
        ]
}
uart_pins! {
    UART4:
        TX: [
            NoTx,
            PA0<Alternate<AF8>>,
            PA12<Alternate<AF6>>,
            PB9<Alternate<AF8>>,
            PC10<Alternate<AF8>>,
            PD1<Alternate<AF8>>,
            PH13<Alternate<AF8>>
        ]
        RX: [
            NoRx,
            PA1<Alternate<AF8>>,
            PA11<Alternate<AF6>>,
            PB8<Alternate<AF8>>,
            PC11<Alternate<AF8>>,
            PD0<Alternate<AF8>>,
            PH14<Alternate<AF8>>,
            PI9<Alternate<AF8>>
        ]
    UART5:
        TX: [
            NoTx,
            PB6<Alternate<AF14>>,
            PB13<Alternate<AF14>>,
            PC12<Alternate<AF8>>
        ]
        RX: [
            NoRx,
            PB5<Alternate<AF14>>,
            PB12<Alternate<AF14>>,
            PD2<Alternate<AF8>>
        ]
    UART7:
        TX: [
            NoTx,
            PA15<Alternate<AF11>>,
            PB4<Alternate<AF11>>,
            PE8<Alternate<AF7>>,
            PF7<Alternate<AF7>>
        ]
        RX: [
            NoRx,
            PA8<Alternate<AF11>>,
            PB3<Alternate<AF11>>,
            PE7<Alternate<AF7>>,
            PF6<Alternate<AF7>>
        ]
    UART8:
        TX: [
            NoTx,
            PE1<Alternate<AF8>>,
            PJ8<Alternate<AF8>>
        ]
        RX: [
            NoRx,
            PE0<Alternate<AF8>>,
            PJ9<Alternate<AF8>>
        ]

    LPUART1:
        TX: [
            NoTx,
            PA9<Alternate<AF3>>,
            PB6<Alternate<AF8>>
        ]
        RX: [
            NoRx,
            PA10<Alternate<AF3>>,
            PB7<Alternate<AF8>>
        ]
}

/// Serial abstraction
pub struct Serial<USART> {
    usart: USART,
}

/// Serial receiver
pub struct Rx<USART> {
    _usart: PhantomData<USART>,
}

/// Serial transmitter
pub struct Tx<USART> {
    _usart: PhantomData<USART>,
}

pub trait SerialExt<USART>: Sized {
    type Rec: ResetEnable;

    fn serial(
        self,
        _pins: impl Pins<USART>,
        config: impl Into<config::Config>,
        prec: Self::Rec,
        clocks: &CoreClocks,
    ) -> Result<Serial<USART>, config::InvalidConfig>;

    fn serial_unchecked(
        self,
        config: impl Into<config::Config>,
        prec: Self::Rec,
        clocks: &CoreClocks,
    ) -> Result<Serial<USART>, config::InvalidConfig>;

    #[deprecated(since = "0.7.0", note = "Deprecated in favour of .serial(..)")]
    fn usart(
        self,
        pins: impl Pins<USART>,
        config: impl Into<config::Config>,
        prec: Self::Rec,
        clocks: &CoreClocks,
    ) -> Result<Serial<USART>, config::InvalidConfig> {
        self.serial(pins, config, prec, clocks)
    }

    #[deprecated(
        since = "0.7.0",
        note = "Deprecated in favour of .serial_unchecked(..)"
    )]
    fn usart_unchecked(
        self,
        config: impl Into<config::Config>,
        prec: Self::Rec,
        clocks: &CoreClocks,
    ) -> Result<Serial<USART>, config::InvalidConfig> {
        self.serial_unchecked(config, prec, clocks)
    }
}

macro_rules! usart {
    ($(
        $USARTX:ident: ($usartX:ident, $Rec:ident, $pclkX:ident),
    )+) => {
        $(
            /// Configures a USART peripheral to provide serial
            /// communication
            impl Serial<$USARTX> {
                pub fn $usartX(
                    usart: $USARTX,
                    config: impl Into<config::Config>,
                    prec: rec::$Rec,
                    clocks: &CoreClocks
                ) -> Result<Self, config::InvalidConfig>
                {
                    use crate::stm32::usart1::cr2::STOP_A as STOP;
                    use self::config::*;

                    let config = config.into();

                    // Enable clock for USART and reset
                    prec.enable().reset();

                    // Get kernel clock
	                let usart_ker_ck = match Self::kernel_clk(clocks) {
                        Some(ker_hz) => ker_hz.0,
                        _ => panic!("$USARTX kernel clock not running!")
                    };

                    // Prescaler not used for now
                    let usart_ker_ck_presc = usart_ker_ck;
                    usart.presc.reset();

                    // Calculate baudrate divisor
                    let usartdiv = usart_ker_ck_presc / config.baudrate.0;
                    assert!(usartdiv <= 65_536);

                    // 16 times oversampling, OVER8 = 0
                    let brr = usartdiv as u16;
                    usart.brr.write(|w| { w.brr().bits(brr) });

                    // disable hardware flow control
                    // TODO enable DMA
                    // usart.cr3.write(|w| w.rtse().clear_bit().ctse().clear_bit());

                    // Reset registers to disable advanced USART features
                    usart.cr2.reset();
                    usart.cr3.reset();

                    // Set stop bits
                    usart.cr2.write(|w| {
                        w.stop().variant(match config.stopbits {
                            StopBits::STOP0P5 => STOP::STOP0P5,
                            StopBits::STOP1 => STOP::STOP1,
                            StopBits::STOP1P5 => STOP::STOP1P5,
                            StopBits::STOP2 => STOP::STOP2,
                        })
                    });

                    // Enable transmission and receiving
                    // and configure frame
                    usart.cr1.write(|w| {
                        w.fifoen()
                            .set_bit() // FIFO mode enabled
                            .over8()
                            .oversampling16() // Oversampling by 16
                            .ue()
                            .enabled()
                            .te()
                            .enabled()
                            .re()
                            .enabled()
                            .m1()
                            .clear_bit()
                            .m0()
                            .variant(match config.wordlength {
                                WordLength::DataBits8 => M0::BIT8,
                                WordLength::DataBits9 => M0::BIT9,
                            }).pce()
                            .variant(match config.parity {
                                Parity::ParityNone => PCE::DISABLED,
                                _ => PCE::ENABLED,
                            }).ps()
                            .variant(match config.parity {
                                Parity::ParityOdd => PS::EVEN,
                                _ => PS::ODD,
                            })
                    });

                    Ok(Serial { usart })
                }

                /// Starts listening for an interrupt event
                pub fn listen(&mut self, event: Event) {
                    match event {
                        Event::Rxne => {
                            self.usart.cr1.modify(|_, w| w.rxneie().enabled())
                        },
                        Event::Txe => {
                            self.usart.cr1.modify(|_, w| w.txeie().enabled())
                        },
                        Event::Idle => {
                            self.usart.cr1.modify(|_, w| w.idleie().enabled())
                        },
                    }
                }

                /// Stop listening for an interrupt event
                pub fn unlisten(&mut self, event: Event) {
                    match event {
                        Event::Rxne => {
                            self.usart.cr1.modify(|_, w| w.rxneie().disabled())
                        },
                        Event::Txe => {
                            self.usart.cr1.modify(|_, w| w.txeie().disabled())
                        },
                        Event::Idle => {
                            self.usart.cr1.modify(|_, w| w.idleie().disabled())
                        },
                    }
                }

                /// Return true if the line idle status is set
                pub fn is_idle(& self) -> bool {
                    unsafe { (*$USARTX::ptr()).isr.read().idle().bit_is_set() }
                }

                /// Return true if the tx register is empty (and can accept data)
                pub fn is_txe(& self) -> bool {
                    unsafe { (*$USARTX::ptr()).isr.read().txe().bit_is_set() }
                }

                /// Return true if the rx register is not empty (and can be read)
                pub fn is_rxne(& self) -> bool {
                    unsafe { (*$USARTX::ptr()).isr.read().rxne().bit_is_set() }
                }

                pub fn split(self) -> (Tx<$USARTX>, Rx<$USARTX>) {
                    (
                        Tx {
                            _usart: PhantomData,
                        },
                        Rx {
                            _usart: PhantomData,
                        },
                    )
                }
                /// Releases the USART peripheral
                pub fn release(self) -> $USARTX {
                    // Wait until both TXFIFO and shift register are empty
                    while self.usart.isr.read().tc().bit_is_clear() {}

                    self.usart
                }
            }

            impl SerialExt<$USARTX> for $USARTX {
                type Rec = rec::$Rec;

                fn serial(self,
                         _pins: impl Pins<$USARTX>,
                         config: impl Into<config::Config>,
                         prec: rec::$Rec,
                         clocks: &CoreClocks
                ) -> Result<Serial<$USARTX>, config::InvalidConfig>
                {
                    Serial::$usartX(self, config, prec, clocks)
                }

                fn serial_unchecked(self,
                                   config: impl Into<config::Config>,
                                   prec: rec::$Rec,
                                   clocks: &CoreClocks
                ) -> Result<Serial<$USARTX>, config::InvalidConfig>
                {
                    Serial::$usartX(self, config, prec, clocks)
                }
            }

            impl serial::Read<u8> for Serial<$USARTX> {
                type Error = Error;

                fn read(&mut self) -> nb::Result<u8, Error> {
                    let mut rx: Rx<$USARTX> = Rx {
                        _usart: PhantomData,
                    };
                    rx.read()
                }
            }

            impl serial::Read<u8> for Rx<$USARTX> {
                type Error = Error;

                fn read(&mut self) -> nb::Result<u8, Error> {
                    // NOTE(unsafe) atomic read with no side effects
                    let isr = unsafe { (*$USARTX::ptr()).isr.read() };

                    Err(if isr.pe().bit_is_set() {
                        unsafe { (*$USARTX::ptr()).icr.write(|w| w.pecf().clear() );};
                        nb::Error::Other(Error::Parity)
                    } else if isr.fe().bit_is_set() {
                        unsafe { (*$USARTX::ptr()).icr.write(|w| w.fecf().clear() );};
                        nb::Error::Other(Error::Framing)
                    } else if isr.nf().bit_is_set() {
                        unsafe { (*$USARTX::ptr()).icr.write(|w| w.ncf().clear() );};
                        nb::Error::Other(Error::Noise)
                    } else if isr.ore().bit_is_set() {
                        unsafe { (*$USARTX::ptr()).icr.write(|w| w.orecf().clear() );};
                        nb::Error::Other(Error::Overrun)
                    } else if isr.rxne().bit_is_set() {
                        // NOTE(read_volatile) see `write_volatile` below
                        return Ok(unsafe {
                            ptr::read_volatile(&(*$USARTX::ptr()).rdr as *const _ as *const _)
                        });
                    } else {
                        nb::Error::WouldBlock
                    })
                }
            }

            impl Rx<$USARTX> {
                /// Start listening for `Rxne` event
                pub fn listen(&mut self) {
                    // unsafe: rxneie bit accessed by Rx part only
                    unsafe { &*$USARTX::ptr() }.cr1.modify(|_, w| w.rxneie().enabled());
                }

                /// Stop listening for `Rxne` event
                pub fn unlisten(&mut self) {
                    // unsafe: rxneie bit accessed by Rx part only
                    unsafe { &*$USARTX::ptr() }.cr1.modify(|_, w| w.rxneie().disabled());
                }
            }

            impl serial::Write<u8> for Serial<$USARTX> {
                type Error = Never;

                fn flush(&mut self) -> nb::Result<(), Never> {
                    let mut tx: Tx<$USARTX> = Tx {
                        _usart: PhantomData,
                    };
                    tx.flush()
                }

                fn write(&mut self, byte: u8) -> nb::Result<(), Never> {
                    let mut tx: Tx<$USARTX> = Tx {
                        _usart: PhantomData,
                    };
                    tx.write(byte)
                }
            }

            impl serial_block::write::Default<u8> for Serial<$USARTX> {
                //implement marker trait to opt-in to default blocking write implementation
            }

            impl serial::Write<u8> for Tx<$USARTX> {
                // NOTE(Void) See section "29.7 USART interrupts"; the
                // only possible errors during transmission are: clear
                // to send (which is disabled in this case) errors and
                // framing errors (which only occur in SmartCard
                // mode); neither of these apply to our hardware
                // configuration
                type Error = Never;

                fn flush(&mut self) -> nb::Result<(), Never> {
                    // NOTE(unsafe) atomic read with no side effects
                    let isr = unsafe { (*$USARTX::ptr()).isr.read() };

                    if isr.tc().bit_is_set() {
                        Ok(())
                    } else {
                        Err(nb::Error::WouldBlock)
                    }
                }

                fn write(&mut self, byte: u8) -> nb::Result<(), Never> {
                    // NOTE(unsafe) atomic read with no side effects
                    let isr = unsafe { (*$USARTX::ptr()).isr.read() };

                    if isr.txe().bit_is_set() {
                        // NOTE(unsafe) atomic write to stateless register
                        // NOTE(write_volatile) 8-bit write that's not
                        // possible through the svd2rust API
                        unsafe {
                            ptr::write_volatile(
                                &(*$USARTX::ptr()).tdr as *const _ as *mut _, byte)
                        }
                        Ok(())
                    } else {
                        Err(nb::Error::WouldBlock)
                    }
                }
            }

            impl Tx<$USARTX> {
                /// Start listening for `Txe` event
                pub fn listen(&mut self) {
                    // unsafe: txeie bit accessed by Tx part only
                    unsafe { &*$USARTX::ptr() }.cr1.modify(|_, w| w.txeie().enabled());
                }

                /// Stop listening for `Txe` event
                pub fn unlisten(&mut self) {
                    // unsafe: txeie bit accessed by Tx part only
                    unsafe { &*$USARTX::ptr() }.cr1.modify(|_, w| w.txeie().disabled());
                }
            }
        )+
    }
}

/// Configures a LPUART peripheral to provide serial communication
impl Serial<LPUART1> {
    pub fn lpuart1(
        lpuart: LPUART1,
        config: config::Config,
        prec: rec::Lpuart1,
        clocks: &CoreClocks,
    ) -> Result<Self, config::InvalidConfig> {
        use self::config::*;

        // Enable clock for USART and reset
        prec.enable().reset();

        // Get kernel clock
        let usart_ker_ck = match Self::kernel_clk(clocks) {
            Some(ker_hz) => ker_hz.0,
            _ => panic!("LPUART1 kernel clock not running!"),
        };

        // Prescaler not used for now
        let usart_ker_ck_presc = usart_ker_ck;
        lpuart.presc.reset();

        // Calculate baudrate divisor
        let usartdiv = usart_ker_ck_presc / config.baudrate.0;
        assert!(usartdiv <= 65_536);

        // 16 times oversampling, OVER8 = 0
        let brr = usartdiv as u32;
        lpuart.brr.write(|w| unsafe { w.brr().bits(brr) });

        // disable hardware flow control
        // TODO enable DMA
        // usart.cr3.write(|w| w.rtse().clear_bit().ctse().clear_bit());

        // Reset registers to disable advanced USART features
        lpuart.cr2.reset();
        lpuart.cr3.reset();

        // Set stop bits
        lpuart.cr2.write(|w| unsafe {
            w.stop().bits(match config.stopbits {
                StopBits::STOP1 => 0,
                StopBits::STOP2 => 1,
                _ => panic!("unsupported stopbits, must be 1 or 2"),
            })
        });

        // Enable transmission and receiving
        // and configure frame
        lpuart.cr1.write(|w| {
            w.fifoen()
                .set_bit() // FIFO mode enabled
                .ue()
                .set_bit()
                .te()
                .set_bit()
                .re()
                .set_bit()
                .m1()
                .clear_bit()
                .m0()
                .bit(match config.wordlength {
                    WordLength::DataBits8 => false,
                    WordLength::DataBits9 => true,
                })
                .pce()
                .bit(match config.parity {
                    Parity::ParityNone => false,
                    _ => true,
                })
                .ps()
                .bit(match config.parity {
                    Parity::ParityOdd => false,
                    _ => true,
                })
        });

        Ok(Serial { usart: lpuart })
    }

    /// Starts listening for an interrupt event
    pub fn listen(&mut self, event: Event) {
        match event {
            Event::Rxne => self.usart.cr1.modify(|_, w| w.rxneie().set_bit()),
            Event::Txe => self.usart.cr1.modify(|_, w| w.txeie().set_bit()),
            Event::Idle => self.usart.cr1.modify(|_, w| w.idleie().set_bit()),
        }
    }

    /// Stop listening for an interrupt event
    pub fn unlisten(&mut self, event: Event) {
        match event {
            Event::Rxne => self.usart.cr1.modify(|_, w| w.rxneie().clear_bit()),
            Event::Txe => self.usart.cr1.modify(|_, w| w.txeie().clear_bit()),
            Event::Idle => self.usart.cr1.modify(|_, w| w.idleie().clear_bit()),
        }
    }

    /// Return true if the line idle status is set
    pub fn is_idle(&self) -> bool {
        unsafe { (*LPUART1::ptr()).isr.read().idle().bit_is_set() }
    }

    /// Return true if the tx register is empty (and can accept data)
    pub fn is_txe(&self) -> bool {
        unsafe { (*LPUART1::ptr()).isr.read().txe().bit_is_set() }
    }

    /// Return true if the rx register is not empty (and can be read)
    pub fn is_rxne(&self) -> bool {
        unsafe { (*LPUART1::ptr()).isr.read().rxne().bit_is_set() }
    }

    pub fn split(self) -> (Tx<LPUART1>, Rx<LPUART1>) {
        (
            Tx {
                _usart: PhantomData,
            },
            Rx {
                _usart: PhantomData,
            },
        )
    }
    /// Releases the USART peripheral
    pub fn release(self) -> LPUART1 {
        // Wait until both TXFIFO and shift register are empty
        while self.usart.isr.read().tc().bit_is_clear() {}

        self.usart
    }
}

impl SerialExt<LPUART1> for LPUART1 {
    type Rec = rec::Lpuart1;

    fn serial(
        self,
        _pins: impl Pins<LPUART1>,
        config: impl Into<config::Config>,
        prec: rec::Lpuart1,
        clocks: &CoreClocks,
    ) -> Result<Serial<LPUART1>, config::InvalidConfig> {
        Serial::lpuart1(self, config.into(), prec, clocks)
    }

    fn serial_unchecked(
        self,
        config: impl Into<config::Config>,
        prec: rec::Lpuart1,
        clocks: &CoreClocks,
    ) -> Result<Serial<LPUART1>, config::InvalidConfig> {
        Serial::lpuart1(self, config.into(), prec, clocks)
    }
}

impl serial::Read<u8> for Serial<LPUART1> {
    type Error = Error;

    fn read(&mut self) -> nb::Result<u8, Error> {
        let mut rx: Rx<LPUART1> = Rx {
            _usart: PhantomData,
        };
        rx.read()
    }
}

impl serial::Read<u8> for Rx<LPUART1> {
    type Error = Error;

    fn read(&mut self) -> nb::Result<u8, Error> {
        // NOTE(unsafe) atomic read with no side effects
        let isr = unsafe { (*LPUART1::ptr()).isr.read() };

        Err(if isr.pe().bit_is_set() {
            unsafe {
                (*LPUART1::ptr()).icr.write(|w| w.pecf().clear_bit());
            };
            nb::Error::Other(Error::Parity)
        } else if isr.fe().bit_is_set() {
            unsafe {
                (*LPUART1::ptr()).icr.write(|w| w.fecf().clear_bit());
            };
            nb::Error::Other(Error::Framing)
        } else if isr.ore().bit_is_set() {
            unsafe {
                (*LPUART1::ptr()).icr.write(|w| w.orecf().clear_bit());
            };
            nb::Error::Other(Error::Overrun)
        } else if isr.rxne().bit_is_set() {
            // NOTE(read_volatile) see `write_volatile` below
            return Ok(unsafe {
                ptr::read_volatile(
                    &(*LPUART1::ptr()).rdr as *const _ as *const _,
                )
            });
        } else {
            nb::Error::WouldBlock
        })
    }
}

impl serial::Write<u8> for Serial<LPUART1> {
    type Error = Never;

    fn flush(&mut self) -> nb::Result<(), Never> {
        let mut tx: Tx<LPUART1> = Tx {
            _usart: PhantomData,
        };
        tx.flush()
    }

    fn write(&mut self, byte: u8) -> nb::Result<(), Never> {
        let mut tx: Tx<LPUART1> = Tx {
            _usart: PhantomData,
        };
        tx.write(byte)
    }
}

impl serial_block::write::Default<u8> for Serial<LPUART1> {
    //implement marker trait to opt-in to default blocking write implementation
}

impl serial::Write<u8> for Tx<LPUART1> {
    // NOTE(Void) See section "29.7 USART interrupts"; the
    // only possible errors during transmission are: clear
    // to send (which is disabled in this case) errors and
    // framing errors (which only occur in SmartCard
    // mode); neither of these apply to our hardware
    // configuration
    type Error = Never;

    fn flush(&mut self) -> nb::Result<(), Never> {
        // NOTE(unsafe) atomic read with no side effects
        let isr = unsafe { (*LPUART1::ptr()).isr.read() };

        if isr.tc().bit_is_set() {
            Ok(())
        } else {
            Err(nb::Error::WouldBlock)
        }
    }

    fn write(&mut self, byte: u8) -> nb::Result<(), Never> {
        // NOTE(unsafe) atomic read with no side effects
        let isr = unsafe { (*LPUART1::ptr()).isr.read() };

        if isr.txe().bit_is_set() {
            // NOTE(unsafe) atomic write to stateless register
            // NOTE(write_volatile) 8-bit write that's not
            // possible through the svd2rust API
            unsafe {
                ptr::write_volatile(
                    &(*LPUART1::ptr()).tdr as *const _ as *mut _,
                    byte,
                )
            }
            Ok(())
        } else {
            Err(nb::Error::WouldBlock)
        }
    }
}

macro_rules! usart16sel {
	($($USARTX:ident,)+) => {
	    $(
            impl Serial<$USARTX> {
                /// Returns the frequency of the current kernel clock
                /// for USART1 and 6
                fn kernel_clk(clocks: &CoreClocks) -> Option<Hertz> {
                    // unsafe: read only
                    let d2ccip2r = unsafe { (*stm32::RCC::ptr()).d2ccip2r.read() };

                    match d2ccip2r.usart16sel().variant() {
                        Val(d2ccip2r::USART16SEL_A::RCC_PCLK2) => Some(clocks.pclk2()),
                        Val(d2ccip2r::USART16SEL_A::PLL2_Q) => clocks.pll2_q_ck(),
                        Val(d2ccip2r::USART16SEL_A::PLL3_Q) => clocks.pll3_q_ck(),
                        Val(d2ccip2r::USART16SEL_A::HSI_KER) => clocks.hsi_ck(),
                        Val(d2ccip2r::USART16SEL_A::CSI_KER) => clocks.csi_ck(),
                        Val(d2ccip2r::USART16SEL_A::LSE) => unimplemented!(),
                        _ => unreachable!(),
                    }
                }
            }
        )+
    }
}
macro_rules! usart234578sel {
	($($USARTX:ident,)+) => {
	    $(
            impl Serial<$USARTX> {
                /// Returns the frequency of the current kernel clock
                /// for USART2/3, UART4/5/7/8
                fn kernel_clk(clocks: &CoreClocks) -> Option<Hertz> {
                    // unsafe: read only
                    let d2ccip2r = unsafe { (*stm32::RCC::ptr()).d2ccip2r.read() };

                    match d2ccip2r.usart234578sel().variant() {
                        Val(d2ccip2r::USART234578SEL_A::RCC_PCLK1) => Some(clocks.pclk1()),
                        Val(d2ccip2r::USART234578SEL_A::PLL2_Q) => clocks.pll2_q_ck(),
                        Val(d2ccip2r::USART234578SEL_A::PLL3_Q) => clocks.pll3_q_ck(),
                        Val(d2ccip2r::USART234578SEL_A::HSI_KER) => clocks.hsi_ck(),
                        Val(d2ccip2r::USART234578SEL_A::CSI_KER) => clocks.csi_ck(),
                        Val(d2ccip2r::USART234578SEL_A::LSE) => unimplemented!(),
                        _ => unreachable!(),
                    }
                }
            }
        )+
    }
}

usart! {
    USART1: (usart1, Usart1, pclk2),
    USART2: (usart2, Usart2, pclk1),
    USART3: (usart3, Usart3, pclk1),
    USART6: (usart6, Usart6, pclk2),

    UART4: (uart4, Uart4, pclk1),
    UART5: (uart5, Uart5, pclk1),
    UART7: (uart7, Uart7, pclk1),
    UART8: (uart8, Uart8, pclk1),
}

usart16sel! {
    USART1, USART6,
}
usart234578sel! {
    USART2, USART3, UART4, UART5, UART7, UART8,
}

impl Serial<LPUART1> {
    /// Returns the frequency of the current kernel clock
    /// for USART2/3, UART4/5/7/8
    fn kernel_clk(clocks: &CoreClocks) -> Option<Hertz> {
        // unsafe: read only
        let d3ccipr = unsafe { (*stm32::RCC::ptr()).d3ccipr.read() };

        match d3ccipr.lpuart1sel().variant() {
            Val(d3ccipr::LPUART1SEL_A::RCC_PCLK_D3) => Some(clocks.pclk3()),
            Val(d3ccipr::LPUART1SEL_A::PLL2_Q) => clocks.pll2_q_ck(),
            Val(d3ccipr::LPUART1SEL_A::PLL3_Q) => clocks.pll3_q_ck(),
            Val(d3ccipr::LPUART1SEL_A::HSI_KER) => clocks.hsi_ck(),
            Val(d3ccipr::LPUART1SEL_A::CSI_KER) => clocks.csi_ck(),
            Val(d3ccipr::LPUART1SEL_A::LSE) => unimplemented!(),
            _ => unreachable!(),
        }
    }
}

impl<USART> fmt::Write for Tx<USART>
where
    Tx<USART>: serial::Write<u8>,
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let _ = s.as_bytes().iter().map(|c| block!(self.write(*c))).last();
        Ok(())
    }
}
