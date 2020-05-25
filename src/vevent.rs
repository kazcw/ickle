use chrono::{NaiveDate, NaiveDateTime};
use crate::{ContentLine, Property, IanaProperty, IanaParam};
use std::str::FromStr;
use log::debug;

#[derive(Debug)]
pub enum DateTime {
    Utc(NaiveDateTime),
    Local(NaiveDateTime, String),
    Floating(NaiveDateTime),
}

#[derive(Debug)]
pub enum When {
    Date(NaiveDate),
    DateTime(DateTime),
}
impl From<NaiveDate> for When {
    fn from(d: NaiveDate) -> Self {
        When::Date(d)
    }
}
impl From<DateTime> for When {
    fn from(d: DateTime) -> Self {
        When::DateTime(d)
    }
}

#[derive(Debug)]
pub enum VEventProperty {
    Dtstart(When),
    Dtend(When),
    Summary(String),
    Unknown,
    Extended(ContentLine),
}

#[derive(Debug)]
enum DataType {
    Date,
    DateTime,
}

#[derive(Debug)]
enum Bad {
    Value { datatype: DataType, invalid: String },
    Condition { error: String },
}
#[derive(Debug)]
pub struct Error {
    bad: Bad,
    line: usize,
}
pub type Result<T> = std::result::Result<T, Error>;

fn parse_date(s: &str) -> std::result::Result<NaiveDate, Bad> {
    let err = || Bad::Value { datatype: DataType::Date, invalid: s.to_owned() };
    if s.len() != 8 { return Err(err()); }
    let err = |_| Bad::Value { datatype: DataType::Date, invalid: s.to_owned() };
    let yy = i32::from_str(&s[0..4]).map_err(err)?;
    let mm = u32::from_str(&s[4..6]).map_err(err)?;
    let dd = u32::from_str(&s[6..8]).map_err(err)?;
    Ok(NaiveDate::from_ymd(yy, mm, dd))
}

fn parse_datetime(s: &str, tzid: Option<&str>) -> std::result::Result<DateTime, Bad> {
    let mut parts = s.splitn(2, 'T');
    let date = parts.next().unwrap();
    let date = parse_date(date)?;
    let err = || Bad::Value { datatype: DataType::DateTime, invalid: s.to_owned() };
    let time = parts.next().ok_or_else(err)?;
    if time.len() < 6 { return Err(err()); }
    let (value, z) = time.split_at(6);
    let err = |_| Bad::Value { datatype: DataType::DateTime, invalid: s.to_owned() };
    let hh = u32::from_str(&value[0..2]).map_err(err)?;
    let mm = u32::from_str(&value[2..4]).map_err(err)?;
    let ss = u32::from_str(&value[4..6]).map_err(err)?;
    let datetime = date.and_hms(hh, mm, ss);
    let err = || Bad::Value { datatype: DataType::DateTime, invalid: s.to_owned() };
    Ok(match (tzid, z) {
        (None, "Z") => DateTime::Utc(datetime),
        (None, "") => DateTime::Floating(datetime),
        (Some(tzid), "") => DateTime::Local(datetime, tzid.to_owned()),
        (Some(_), "Z") => return Err(Bad::Condition{ error: "UTC time must not have TZID".to_owned() }),
        _ =>  return Err(err()),
    })
}

fn parse_when(coli: &ContentLine) -> std::result::Result<When, Bad> {
    Ok(match coli.value_of(IanaParam::Value) {
        Some("DATE") => parse_date(coli.value())?.into(),
        _ => {
            let tzid = coli.value_of(IanaParam::Tzid);
            parse_datetime(coli.value(), tzid)?.into()
        }
    })
}

pub fn parse_property(coli: &ContentLine) -> Result<Option<VEventProperty>> {
    use VEventProperty::*;
    let line = coli.line();
    let iana = match coli.name() {
        Property::Iana(iana) => iana,
        Property::Extended(_) => return Ok(Some(Extended(coli.clone()))),
        Property::End => return Ok(None),
        Property::Begin => todo!(),
    };
    Ok(Some(match iana {
        IanaProperty::Dtstart => Dtstart(parse_when(coli).map_err(|bad| Error { bad, line } )?),
        IanaProperty::Dtend => Dtend(parse_when(coli).map_err(|bad| Error { bad, line } )?),
        IanaProperty::Summary => Summary(coli.value().to_owned()),
        IanaProperty::Dtstamp |
        IanaProperty::Uid |
        IanaProperty::Class |
        IanaProperty::Created |
        IanaProperty::Description |
        IanaProperty::Geo |
        IanaProperty::LastModified |
        IanaProperty::Location |
        IanaProperty::Organizer |
        IanaProperty::Priority |
        IanaProperty::Sequence |
        IanaProperty::Status |
        IanaProperty::Transp |
        IanaProperty::Url |
        IanaProperty::RecurrenceId |
        IanaProperty::Rrule |
        IanaProperty::Duration |
        IanaProperty::Attach |
        IanaProperty::Attendee |
        IanaProperty::Categories |
        IanaProperty::Comment |
        IanaProperty::Contact |
        IanaProperty::Exdate |
        IanaProperty::RequestStatus |
        IanaProperty::RelatedTo |
        IanaProperty::Resources |
        IanaProperty::Rdate => {
            debug!("VEVENT property not implemented: {}", iana.as_str());
            Unknown
        }
        _ => Unknown
    }))
}
