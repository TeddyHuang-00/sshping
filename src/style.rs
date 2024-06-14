use clap::ValueEnum;
use tabled::{settings::Style, Table};

#[derive(ValueEnum, Clone, PartialEq, Eq, Debug)]
pub enum TableStyle {
    Empty,
    Blank,
    ASCII,
    PSQL,
    Markdown,
    Modern,
    Sharp,
    Extended,
    Dots,
    RST,
    Rounded,
    ASCIIRounded,
    ModernRounded,
}

impl TableStyle {
    pub fn stylize<'a>(&self, table: &'a mut Table) -> &'a mut Table {
        match self {
            TableStyle::Empty => table.with(Style::empty()),
            TableStyle::Blank => table.with(Style::blank()),
            TableStyle::ASCII => table.with(Style::ascii()),
            TableStyle::PSQL => table.with(Style::psql()),
            TableStyle::Markdown => table.with(Style::markdown()),
            TableStyle::Modern => table.with(Style::modern()),
            TableStyle::Sharp => table.with(Style::sharp()),
            TableStyle::Extended => table.with(Style::extended()),
            TableStyle::Dots => table.with(Style::dots()),
            TableStyle::RST => table.with(Style::re_structured_text()),
            TableStyle::Rounded => table.with(Style::rounded()),
            TableStyle::ASCIIRounded => table.with(Style::ascii_rounded()),
            TableStyle::ModernRounded => table.with(Style::modern_rounded()),
        }
    }
}
