use crate::define_identifier_set;
use chrono::{NaiveDate, NaiveDateTime};
use crate::{ContentLine, Property, IanaProperty, IanaParam};
use std::str::FromStr;
#[allow(unused)]
use log::{debug, warn};

// XXX: move TZID to wrapping type?
#[derive(Debug)]
pub enum DateTime {
    Utc(NaiveDateTime),
    Local(NaiveDateTime, String),
    Floating(NaiveDateTime),
}
impl DateTime {
    fn with_tzid(self, tzid: String) -> Maybe<Self> {
        match self {
            DateTime::Utc(_) => return Err(Bad::Condition{ error: "UTC time must not have TZID".to_owned() }),
            DateTime::Floating(dt) => Ok(DateTime::Local(dt, tzid)),
            DateTime::Local(..) => panic!("set tzid twice on the same DateTime?"),
        }
    }
}
#[derive(Debug)]
pub struct UtcDate(NaiveDateTime);

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
    Rrule(Rrule),
    Uid(String),
    Description(String),
    Comment(String),
    Status(Status),
    RecurrenceId(When),
    Location(String),
    Sequence(u32),
    Transp(Transp),
    Dtstamp(UtcDate),
    Created(UtcDate),
    LastModified(UtcDate),
    Exdate(When),
}

#[derive(Debug)]
enum DataType {
    Date,
    DateTime,
    Rrule,
    Status,
    Int,
    Transp,
    UtcDate,
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
type Maybe<T> = std::result::Result<T, Bad>;
pub type Result<T> = std::result::Result<T, Error>;

fn parse_date(s: &str) -> Maybe<NaiveDate> {
    let err = || Bad::Value { datatype: DataType::Date, invalid: s.to_owned() };
    if s.len() != 8 { return Err(err()); }
    let err = |_| Bad::Value { datatype: DataType::Date, invalid: s.to_owned() };
    let yy = i32::from_str(&s[0..4]).map_err(err)?;
    let mm = u32::from_str(&s[4..6]).map_err(err)?;
    let dd = u32::from_str(&s[6..8]).map_err(err)?;
    Ok(NaiveDate::from_ymd(yy, mm, dd))
}

fn parse_datetime(s: &str) -> Maybe<DateTime> {
    let err = || Bad::Value { datatype: DataType::DateTime, invalid: s.to_owned() };
    let mut parts = s.splitn(2, 'T');
    let date = parts.next().unwrap();
    let date = parse_date(date).map_err(|_| err())?;
    let time = parts.next().ok_or_else(err)?;
    if time.len() < 6 { return Err(err()); }
    let (value, z) = time.split_at(6);
    let err = |_| Bad::Value { datatype: DataType::DateTime, invalid: s.to_owned() };
    let hh = u32::from_str(&value[0..2]).map_err(err)?;
    let mm = u32::from_str(&value[2..4]).map_err(err)?;
    let ss = u32::from_str(&value[4..6]).map_err(err)?;
    let datetime = date.and_hms(hh, mm, ss);
    let err = || Bad::Value { datatype: DataType::DateTime, invalid: s.to_owned() };
    Ok(match z {
        "Z" => DateTime::Utc(datetime),
        "" => DateTime::Floating(datetime),
        _ =>  return Err(err()),
    })
}

fn parse_utc(s: &str) -> Maybe<UtcDate> {
    let t = parse_datetime(s)?;
    if let DateTime::Utc(utc) = t {
        Ok(UtcDate(utc))
    } else {
        Err(Bad::Value { datatype: DataType::UtcDate, invalid: s.to_owned() })
    }
}

fn parse_when(coli: &ContentLine) -> Maybe<When> {
    Ok(match coli.value_of(IanaParam::Value) {
        Some("DATE") => parse_date(coli.value())?.into(),
        _ => {
            let tzid = coli.value_of(IanaParam::Tzid);
            let mut dt = parse_datetime(coli.value())?;
            if let Some(tzid) = tzid {
                dt = dt.with_tzid(tzid.to_owned())?;
            }
            dt.into()
        }
    })
}

define_identifier_set!(Transp,
    Transparent, b"TRANSPARENT",
    Opaque,      b"OPAQUE",
);

define_identifier_set!(Status,
    Tentative, b"TENTATIVE",
    Confirmed, b"CONFIRMED",
    Cancelled, b"CANCELLED",
);

define_identifier_set!(Freq,
    Secondly, b"SECONDLY",
    Minutely, b"MINUTELY",
    Hourly,   b"HOURLY" ,
    Daily,    b"DAILY" ,
    Weekly,   b"WEEKLY" ,
    Monthly,  b"MONTHLY" ,
    Yearly,   b"YEARLY",
);
define_identifier_set!(Weekday,
    Mo, b"MO",
    Tu, b"TU",
    We, b"WE" ,
    Th, b"TH" ,
    Fr, b"FR" ,
    Sa, b"SA" ,
    Su, b"SU",
);
#[derive(Debug)]
enum Until {
    Date(NaiveDate),
    DateTime(DateTime),
}
#[derive(Debug)]
enum Stop {
    Until(Until),
    Count(u32),
}
#[derive(Debug)]
struct WeekdayNum {
    wday: Weekday,
    num: Option<i8>,
}
impl FromStr for WeekdayNum {
    type Err = ();
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if s.len() < 2 {
            return Err(());
        }
        let (num, wday) = s.split_at(s.len()-2);
        let wday = wday.parse()?;
        let num = if s.len() == 2 {
            None
        } else {
            Some(num.parse().map_err(|_| ())?)
        };
        Ok(WeekdayNum { wday, num })
    }
}
#[derive(Debug)]
pub struct Rrule {
    freq: Freq,
    stop: Option<Stop>,
    interval: Option<u32>,
    wkst: Option<Weekday>,
    bysecond: Option<Vec<u8>>,
    byminute: Option<Vec<u8>>,
    byhour: Option<Vec<u8>>,
    bymonth: Option<Vec<u8>>,
    byyearday: Option<Vec<i32>>,
    bymonthday: Option<Vec<i32>>,
    byweekno: Option<Vec<i32>>,
    bysetpos: Option<Vec<u32>>,
    byday: Option<Vec<WeekdayNum>>,
}

fn parse_until(s: &str) -> Maybe<Until> {
    Ok(if s.len() == 8 {
        Until::Date(parse_date(s)?)
    } else {
        Until::DateTime(parse_datetime(s)?)
    })
}

fn parse_comma_list<X: FromStr>(s: &str) -> std::result::Result<Vec<X>, X::Err> {
    s.split(',').map(|v| FromStr::from_str(v)).collect()
}

fn parse_rrule(coli: &ContentLine) -> Maybe<Rrule> {
    let spec = coli.value();
    let err = || Bad::Value { datatype: DataType::Rrule, invalid: spec.to_owned() };
    let mut freq = None;
    let mut stop = None;
    let mut interval = None;
    let mut wkst = None;
    let mut bysecond = None;
    let mut byminute = None;
    let mut byhour = None;
    let mut bymonth = None;
    let mut byyearday = None;
    let mut bymonthday = None;
    let mut byweekno = None;
    let mut bysetpos = None;
    let mut byday = None;
    for part in spec.split(';') {
        let mut parts = part.splitn(2, '=');
        let type_ = parts.next().unwrap();
        let value = parts.next().ok_or_else(err)?;
        match type_ {
            "FREQ" => freq = Some(Freq::from_str(value).map_err(|_| err())?),
            "UNTIL" => stop = Some(Stop::Until(parse_until(value).map_err(|_| err())?)),
            "COUNT" => stop = Some(Stop::Count(u32::from_str(value).map_err(|_| err())?)),
            "INTERVAL" => interval = Some(u32::from_str(value).map_err(|_| err())?),
            "WKST" => wkst = Some(Weekday::from_str(value).map_err(|_| err())?),
            "BYSECOND" => bysecond = Some(parse_comma_list(value).map_err(|_| err())?),
            "BYMINUTE" => byminute = Some(parse_comma_list(value).map_err(|_| err())?),
            "BYHOUR" => byhour = Some(parse_comma_list(value).map_err(|_| err())?),
            "BYMONTH" => bymonth = Some(parse_comma_list(value).map_err(|_| err())?),
            "BYYEARDAY" => byyearday = Some(parse_comma_list(value).map_err(|_| err())?),
            "BYMONTHDAY" => bymonthday = Some(parse_comma_list(value).map_err(|_| err())?),
            "BYWEEKNO" => byweekno = Some(parse_comma_list(value).map_err(|_| err())?),
            "BYSETPOS" => bysetpos = Some(parse_comma_list(value).map_err(|_| err())?),
            "BYDAY" => byday = Some(parse_comma_list(value).map_err(|_| err())?),
            _ => return Err(err()),
        }
    }
    Ok(Rrule {
        freq: freq.unwrap(),
        stop,
        interval,
        wkst,
        bysecond,
        byminute,
        byhour,
        bymonth,
        byyearday,
        bymonthday,
        byweekno,
        bysetpos,
        byday,
    })
}

fn parse_data<X: FromStr>(s: &str, datatype: DataType) -> Maybe<X> {
    X::from_str(s).map_err(|_| Bad::Value { datatype, invalid: s.to_owned() })
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
        IanaProperty::Rrule => Rrule(parse_rrule(coli).map_err(|bad| Error { bad, line } )?),
        IanaProperty::Uid => Uid(coli.value().to_owned()),
        IanaProperty::Description => Description(coli.value().to_owned()),
        IanaProperty::Comment => Comment(coli.value().to_owned()),
        IanaProperty::Status => Status(parse_data(coli.value(), DataType::Status).map_err(|bad| Error { bad, line } )?),
        IanaProperty::RecurrenceId  => RecurrenceId(parse_when(coli).map_err(|bad| Error { bad, line } )?),
        IanaProperty::Location => Location(coli.value().to_owned()),
        IanaProperty::Sequence => Sequence(parse_data(coli.value(), DataType::Int).map_err(|bad| Error { bad, line } )?),
        IanaProperty::Transp => Transp(parse_data(coli.value(), DataType::Transp).map_err(|bad| Error { bad, line } )?),
        IanaProperty::Dtstamp => Dtstamp(parse_utc(coli.value()).map_err(|bad| Error { bad, line } )?),
        IanaProperty::LastModified => LastModified(parse_utc(coli.value()).map_err(|bad| Error { bad, line } )?),
        IanaProperty::Created => Created(parse_utc(coli.value()).map_err(|bad| Error { bad, line } )?),
        IanaProperty::Exdate => Exdate(parse_when(coli).map_err(|bad| Error { bad, line } )?),
        IanaProperty::Class |
        IanaProperty::Geo |
        IanaProperty::Organizer |
        IanaProperty::Priority |
        IanaProperty::Url |
        IanaProperty::Duration |
        IanaProperty::Attach |
        IanaProperty::Attendee |
        IanaProperty::Categories |
        IanaProperty::Contact |
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
