use crate::media::playback::{GetInnerSamples, PlaybackFrame};

use super::{
    errors::{
        CloseError, FindError, InfoError, InitializationError, ListError, OpenError,
        SubmissionError,
    },
    format::{FormatInfo, SupportedFormat},
};

pub trait DeviceProvider {
    fn initialize(&mut self) -> Result<(), InitializationError>;
    fn get_devices(&mut self) -> Result<Vec<Box<dyn Device>>, ListError>;
    fn get_default_device(&mut self) -> Result<Box<dyn Device>, FindError>;
    fn get_device_by_uid(&mut self, id: &String) -> Result<Box<dyn Device>, FindError>;
}

pub trait Device {
    fn open_device(&mut self, format: FormatInfo) -> Result<Box<dyn OutputStream>, OpenError>;

    fn get_supported_formats(&self) -> Result<Vec<SupportedFormat>, InfoError>;
    fn get_default_format(&self) -> Result<FormatInfo, InfoError>;
    fn get_name(&self) -> Result<String, InfoError>;
    fn get_uid(&self) -> Result<String, InfoError>;
    fn requires_matching_format(&self) -> bool;
}

pub trait OutputStream {
    fn submit_frame(&mut self, frame: PlaybackFrame) -> Result<(), SubmissionError>;
    fn close_stream(&mut self) -> Result<(), CloseError>;
    fn needs_input(&self) -> bool;
    fn get_current_format(&self) -> Result<&FormatInfo, InfoError>;
}
