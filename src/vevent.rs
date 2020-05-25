use chrono::{NaiveDate, NaiveDateTime};
use crate::{ContentLine, Property, ParamName};
use std::str::FromStr;
use log::debug;

pub enum DateTime {
    Utc(NaiveDateTime),
    Local(NaiveDateTime, String),
    Floating(NaiveDateTime),
}

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

pub enum VEventProperty {
    Dtstart(When),
    Dtend(When),
    Summary(String),
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

fn parse_when(coli: &mut ContentLine) -> std::result::Result<When, Bad> {
    Ok(match coli.value_of(ParamName::Value) {
        Some("DATE") => parse_date(coli.value())?.into(),
        _ => {
            let tzid = coli.value_of(ParamName::Tzid);
            parse_datetime(coli.value(), tzid)?.into()
        }
    })
}

pub fn parse_property(coli: &mut ContentLine) -> Result<Option<VEventProperty>> {
    use VEventProperty::*;
    let line = coli.line();
    Ok(Some(match coli.name() {
        Property::Dtstart => Dtstart(parse_when(coli).map_err(|bad| Error { bad, line } )?),
        Property::Dtend => Dtend(parse_when(coli).map_err(|bad| Error { bad, line } )?),
        Property::Summary => Summary(coli.value().to_owned()),
        Property::Dtstamp |
        Property::Uid |
        Property::Class |
        Property::Created |
        Property::Description |
        Property::Geo |
        Property::LastModified |
        Property::Location |
        Property::Organizer |
        Property::Priority |
        Property::Sequence |
        Property::Status |
        Property::Transp |
        Property::Url |
        Property::RecurrenceId |
        Property::Rrule |
        Property::Duration |
        Property::Attach |
        Property::Attendee |
        Property::Categories |
        Property::Comment |
        Property::Contact |
        Property::Exdate |
        Property::RequestStatus |
        Property::RelatedTo |
        Property::Resources |
        Property::Rdate => {
            debug!("VEVENT property not implemented: {}", coli.name().as_str());
            return Ok(None);
        }
        _ => return Ok(None)
    }))
}
