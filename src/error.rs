use std::fmt;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub enum Error {
    AlreadySaved(String),
    BadCoordinates(String),
    BadFile(PathBuf, serde_json::Error),
    BadVersion(String),
    File(PathBuf, io::Error),
    LastArea(String),
    MissingData(&'static str),
    MissingHome,
    Network(Box<ureq::Error>),
    NoElevation,
    NoResults(String),
    Output(io::Error),
    ResultOutOfRange { chosen: usize, available: usize },
    UnexpectedData(serde_json::Error),
    UnknownArea(String),
    UnknownWeatherCode(u8),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::AlreadySaved(name) => write!(f, "there is already an area called {name}"),
            Error::BadCoordinates(text) => write!(f, "cannot read the coordinates: {text}"),
            Error::BadFile(path, source) => write!(f, "cannot read {}: {source}", path.display()),
            Error::BadVersion(text) => write!(f, "cannot read the version {text}"),
            Error::File(path, source) => write!(f, "cannot access {}: {source}", path.display()),
            Error::LastArea(name) => write!(
                f,
                "{name} is the only area left: add another one before removing it"
            ),
            Error::MissingData(name) => write!(f, "the forecast has no {name}"),
            Error::MissingHome => write!(f, "the HOME variable is not set"),
            Error::Network(source) => write!(f, "the request failed: {source}"),
            Error::NoElevation => write!(f, "no elevation found for this place"),
            Error::NoResults(query) => write!(f, "no places found for {query}"),
            Error::Output(source) => write!(f, "cannot print the result: {source}"),
            Error::ResultOutOfRange { chosen, available } => {
                write!(f, "result {chosen} does not exist, choose 1 to {available}")
            }
            Error::UnexpectedData(source) => write!(f, "unexpected data: {source}"),
            Error::UnknownArea(name) => write!(
                f,
                "unknown area: {name}. Add it with: peakcast add \"{name}\""
            ),
            Error::UnknownWeatherCode(code) => write!(f, "unknown weather code: {code}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::BadFile(_, source) | Error::UnexpectedData(source) => Some(source),
            Error::File(_, source) | Error::Output(source) => Some(source),
            Error::Network(source) => Some(source),
            Error::AlreadySaved(_)
            | Error::BadCoordinates(_)
            | Error::BadVersion(_)
            | Error::LastArea(_)
            | Error::MissingData(_)
            | Error::MissingHome
            | Error::NoElevation
            | Error::NoResults(_)
            | Error::ResultOutOfRange { .. }
            | Error::UnknownArea(_)
            | Error::UnknownWeatherCode(_) => None,
        }
    }
}

impl From<ureq::Error> for Error {
    fn from(source: ureq::Error) -> Self {
        Error::Network(Box::new(source))
    }
}

impl From<io::Error> for Error {
    fn from(source: io::Error) -> Self {
        Error::Output(source)
    }
}
