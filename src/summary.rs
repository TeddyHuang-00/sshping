use crate::util::Formatter;
use std::time::Duration;
use tabled::Tabled;

pub struct EchoTestSummary {
    pub char_count: usize,
    pub char_sent: usize,
    pub avg_latency: Duration,
    pub std_latency: Duration,
    pub med_latency: Duration,
    pub min_latency: Duration,
    pub max_latency: Duration,
}

impl EchoTestSummary {
    pub fn from_latencies(latencies: &Vec<u128>, char_count: usize) -> Self {
        let char_sent = latencies.len();
        let avg_latency = latencies.iter().sum::<u128>() / (char_sent as u128);
        let std_latency = Duration::from_nanos(
            ((latencies
                .iter()
                .map(|&latency| ((latency as i128) - (avg_latency as i128)).pow(2))
                .sum::<i128>() as f64)
                / (char_sent as f64))
                .sqrt() as u64,
        );
        let avg_latency = Duration::from_nanos(avg_latency as u64);
        let med_latency = Duration::from_nanos(
            (match char_sent % 2 {
                0 => (latencies[char_sent / 2 - 1] + latencies[char_sent / 2]) / 2,
                _ => latencies[char_sent / 2],
            }) as u64,
        );
        let min_latency = Duration::from_nanos(latencies.first().unwrap().to_owned() as u64);
        let max_latency = Duration::from_nanos(latencies.last().unwrap().to_owned() as u64);
        Self {
            char_count,
            char_sent,
            avg_latency,
            std_latency,
            med_latency,
            min_latency,
            max_latency,
        }
    }
    pub fn to_formatted_frame(&self, formatter: &Formatter) -> Vec<Record> {
        vec![
            Record::new(
                "Latency",
                "Average",
                formatter.format_duration(self.avg_latency),
            ),
            Record::new(
                "Latency",
                "Std deviation",
                formatter.format_duration(self.std_latency),
            ),
            Record::new(
                "Latency",
                "Median",
                formatter.format_duration(self.med_latency),
            ),
            Record::new(
                "Latency",
                "Minimum",
                formatter.format_duration(self.min_latency),
            ),
            Record::new(
                "Latency",
                "Maximum",
                formatter.format_duration(self.max_latency),
            ),
        ]
    }
}

pub struct SpeedTestResult {
    pub size: u64,
    pub time: Duration,
}

impl SpeedTestResult {
    pub fn speed(&self) -> u64 {
        ((self.size as f64) / self.time.as_secs_f64()) as u64
    }
}

pub struct SpeedTestSummary {
    pub upload: SpeedTestResult,
    pub download: SpeedTestResult,
}

impl SpeedTestSummary {
    pub fn to_formatted_frame(&self, formatter: &Formatter) -> Vec<Record> {
        vec![
            Record::new(
                "Speed",
                "Upload",
                formatter.format_size(self.upload.speed()) + "/s",
            ),
            Record::new(
                "Speed",
                "Download",
                formatter.format_size(self.download.speed()) + "/s",
            ),
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
    pub fn new(test: &'static str, metric: &'static str, result: String) -> Self {
        Self {
            test,
            metric,
            result,
        }
    }
}