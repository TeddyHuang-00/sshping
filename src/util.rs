use std::time::Duration;

use num_format::{Buffer, CustomFormat};
use size::{Base, Size, Style};

pub struct Formatter {
    // Formatter style for large number
    // Only used when human_readable is false
    format: Option<CustomFormat>,
}

impl Formatter {
    pub fn new(human_readable: bool, delimit: Option<char>) -> Self {
        let format = (!human_readable).then(|| {
            CustomFormat::builder()
                .separator(delimit.map(|ch| ch.to_string()).unwrap_or_default())
                .build()
                .unwrap()
        });

        Self { format }
    }

    pub fn format_duration(&self, time: Duration) -> String {
        self.format.as_ref().map_or_else(
            || {
                let formatted = humantime::format_duration(time).to_string();
                let parts = formatted.split(" ").collect::<Vec<&str>>();
                if parts.len() > 2 {
                    parts[..2].join(" ")
                } else {
                    formatted
                }
            },
            |format| {
                let mut buffer = Buffer::new();
                buffer.write_formatted(&time.as_nanos(), format);
                buffer.as_str().to_string() + "ns"
            },
        )
    }

    pub fn format_size(&self, size: u64) -> String {
        self.format.as_ref().map_or_else(
            || {
                Size::from_bytes(size)
                    .format()
                    .with_base(Base::Base10)
                    .with_style(Style::Abbreviated)
                    .to_string()
            },
            |format| {
                let mut buffer = Buffer::new();
                buffer.write_formatted(&size, format);
                buffer.as_str().to_string() + " B"
            },
        )
    }
}
