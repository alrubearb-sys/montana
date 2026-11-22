use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum Error {
    AlreadySaved(String),
    BadCoordinates(String),
    BadFile(PathBuf, serde_json::Error),
    File(PathBuf, std::io::Error),
    MissingData(&'static str),
    MissingHome,
    Network(Box<ureq::Error>),
    NoElevation,
    NoResults(String),
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
            Error::File(path, source) => write!(f, "cannot access {}: {source}", path.display()),
            Error::MissingData(name) => write!(f, "the forecast has no {name}"),
            Error::MissingHome => write!(f, "the HOME variable is not set"),
            Error::Network(source) => write!(f, "the request failed: {source}"),
            Error::NoElevation => write!(f, "no elevation found for this place"),
            Error::NoResults(query) => write!(f, "no places found for {query}"),
            Error::ResultOutOfRange { chosen, available } => {
                write!(f, "result {chosen} does not exist, choose 1 to {available}")
            }
            Error::UnexpectedData(source) => write!(f, "unexpected data: {source}"),
            Error::UnknownArea(name) => write!(
                f,
                "unknown area: {name}. Add it with: montana add \"{name}\""
            ),
            Error::UnknownWeatherCode(code) => write!(f, "unknown weather code: {code}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::BadFile(_, source) | Error::UnexpectedData(source) => Some(source),
            Error::File(_, source) => Some(source),
            Error::Network(source) => Some(source),
            Error::AlreadySaved(_)
            | Error::BadCoordinates(_)
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
