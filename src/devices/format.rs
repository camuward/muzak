use bitflags::bitflags;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleFormat {
    Float64,
    Float32,
    Signed32,
    Unsigned32,
    Signed24,
    Unsigned24,
    Signed24Packed,
    Unsigned24Packed,
    Signed16,
    Unsigned16,
    Signed8,
    Unsigned8,
    Dsd,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelSpec {
    Bitmask(Channels),
    Count(u16),
}

impl ChannelSpec {
    pub fn count(self) -> u16 {
        match self {
            ChannelSpec::Bitmask(channels) => channels.count(),
            ChannelSpec::Count(count) => count,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferSize {
    /// Inclusive range of supported buffer sizes.
    Range(u32, u32),
    Fixed(u32),
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatInfo {
    pub originating_provider: &'static str,
    pub sample_type: SampleFormat,
    pub sample_rate: u32,
    pub buffer_size: BufferSize,
    pub channels: ChannelSpec,
    /// The number of channels for the sample rate.
    ///
    /// On some implementations the sample rate is the device's fixed sample rate; on others it is
    /// the sample rate of the current stream. `rate_channel_ratio` is used to determine the number
    /// of channels for the current sample rate, if the number of channels is fixed.
    pub rate_channel_ratio: Option<u16>,
}
pub struct SupportedFormat {
    pub originating_provider: &'static str,
    pub sample_type: SampleFormat,
    /// Lowest and highest supported sample rates.
    pub sample_rates: (u32, u32),
    pub buffer_size: BufferSize,
    pub channels: ChannelSpec,
}

bitflags! {
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct Channels: u32 {
        const FRONT_LEFT            = 0x1;
        const FRONT_RIGHT           = 0x2;
        const FRONT_CENTER          = 0x4;
        const LOW_FREQUENCY         = 0x8;
        const BACK_LEFT             = 0x10;
        const BACK_RIGHT            = 0x20;
        const FRONT_LEFT_OF_CENTER  = 0x40;
        const FRONT_RIGHT_OF_CENTER = 0x80;
        const BACK_CENTER           = 0x100;
        const SIDE_LEFT             = 0x200;
        const SIDE_RIGHT            = 0x400;
        const TOP_CENTER            = 0x800;
        const TOP_FRONT_LEFT        = 0x1000;
        const TOP_FRONT_CENTER      = 0x2000;
        const TOP_FRONT_RIGHT       = 0x4000;
        const TOP_BACK_LEFT         = 0x8000;
        const TOP_BACK_CENTER       = 0x10000;
        const TOP_BACK_RIGHT        = 0x20000;
    }
}

impl Channels {
    pub fn count(self) -> u16 {
        self.bits().count_ones().try_into().expect("infallible")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Mono,
    Stereo,
    TwoOne,
    FiveOne,
    SevenOne,
}

impl Layout {
    pub fn channels(self) -> Channels {
        match self {
            Layout::Mono => Channels::FRONT_LEFT,
            Layout::Stereo => Channels::FRONT_LEFT | Channels::FRONT_RIGHT,
            Layout::TwoOne => {
                Channels::FRONT_LEFT | Channels::FRONT_RIGHT | Channels::LOW_FREQUENCY
            }
            Layout::FiveOne => {
                Channels::FRONT_LEFT
                    | Channels::FRONT_RIGHT
                    | Channels::BACK_LEFT
                    | Channels::BACK_RIGHT
                    | Channels::LOW_FREQUENCY
            }
            Layout::SevenOne => {
                Channels::FRONT_LEFT
                    | Channels::FRONT_RIGHT
                    | Channels::SIDE_LEFT
                    | Channels::SIDE_RIGHT
                    | Channels::BACK_LEFT
                    | Channels::BACK_RIGHT
                    | Channels::LOW_FREQUENCY
            }
        }
    }
}
