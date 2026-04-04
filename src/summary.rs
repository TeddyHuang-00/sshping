use std::time::Duration;

use serde::{Deserialize, Serialize};
use tabled::Tabled;

use crate::util::Formatter;

#[derive(Deserialize, Serialize)]
pub struct EchoTestSummary {
    pub char_sent: usize,
    pub avg_latency: String,
    pub std_latency: String,
    pub med_latency: String,
    pub min_latency: String,
    pub max_latency: String,
}

impl EchoTestSummary {
    pub fn from_latencies(latencies: &[u128], formatter: &Formatter) -> Result<Self, String> {
        let char_sent = latencies.len();
        if char_sent == 0 {
            return Err("Unable to get any echos in given time".to_string());
        }
        let avg_latency = latencies.iter().sum::<u128>() / (char_sent as u128);
        let std_latency = formatter.format_duration(Duration::from_nanos(
            ((latencies
                .iter()
                .map(|&latency| ((latency as i128) - (avg_latency as i128)).pow(2))
                .sum::<i128>() as f64)
                / (char_sent as f64))
                .sqrt() as u64,
        ));
        let avg_latency = formatter.format_duration(Duration::from_nanos(avg_latency as u64));
        let med_latency = formatter.format_duration(Duration::from_nanos(
            (match char_sent % 2 {
                0 => (latencies[char_sent / 2 - 1] + latencies[char_sent / 2]) / 2,
                _ => latencies[char_sent / 2],
            }) as u64,
        ));
        let min_latency = formatter.format_duration(Duration::from_nanos(
            latencies.first().unwrap().to_owned() as u64,
        ));
        let max_latency = formatter.format_duration(Duration::from_nanos(
            latencies.last().unwrap().to_owned() as u64,
        ));
        Ok(Self {
            char_sent,
            avg_latency,
            std_latency,
            med_latency,
            min_latency,
            max_latency,
        })
    }
    pub fn to_formatted_frame(&self) -> Vec<Record> {
        vec![
            Record::new("Latency", "Average", self.avg_latency.clone()),
            Record::new("Latency", "Std deviation", self.std_latency.clone()),
            Record::new("Latency", "Median", self.med_latency.clone()),
            Record::new("Latency", "Minimum", self.min_latency.clone()),
            Record::new("Latency", "Maximum", self.max_latency.clone()),
        ]
    }
}

#[derive(Deserialize, Serialize)]
pub struct SpeedTestResult {
    pub size: String,
    pub time: String,
    pub speed: String,
}

impl SpeedTestResult {
    pub fn new(size: u64, time: Duration, formatter: &Formatter) -> Self {
        let time_nanos = time.as_nanos();
        let speed = if time_nanos == 0 {
            0
        } else {
            ((size as u128) * 1_000_000_000 / time_nanos) as u64
        };
        Self {
            size: formatter.format_size(size),
            time: formatter.format_duration(time),
            speed: formatter.format_size(speed) + "/s",
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct SpeedTestSummary {
    pub upload: SpeedTestResult,
    pub download: SpeedTestResult,
}

impl SpeedTestSummary {
    pub fn to_formatted_frame(&self) -> Vec<Record> {
        vec![
            Record::new("Speed", "Upload", self.upload.speed.clone()),
            Record::new("Speed", "Download", self.download.speed.clone()),
        ]
    }
}

#[derive(Tabled)]
pub struct Record {
    #[tabled(rename = "Test")]
    pub test: &'static str,
    #[tabled(rename = "Metric")]
    pub metric: &'static str,
    #[tabled(rename = "Result")]
    pub result: String,
}

impl Record {
    pub const fn new(test: &'static str, metric: &'static str, result: String) -> Self {
        Self {
            test,
            metric,
            result,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{EchoTestSummary, SpeedTestResult};
    use crate::util::Formatter;

    #[test]
    fn echo_summary_rejects_empty_latencies() {
        let formatter = Formatter::new(false, Some(','));
        let result = EchoTestSummary::from_latencies(&[], &formatter);
        assert!(result.is_err());
    }

    #[test]
    fn speed_result_handles_zero_duration() {
        let formatter = Formatter::new(false, Some(','));
        let result = SpeedTestResult::new(1024, Duration::from_nanos(0), &formatter);
        assert_eq!(result.speed, "0 B/s");
    }
}
