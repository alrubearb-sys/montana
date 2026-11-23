use std::fmt;

use serde::Deserialize;

use crate::data::get;
use crate::error::Error;

const LATEST: &str = "https://api.github.com/repos/alrubearb-sys/peakcast/releases/latest";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(u64, u64, u64);

impl Version {
    // "0.3.0", or "v0.3.0" as the tags are called.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let invalid = || Error::BadVersion(text.to_string());
        let mut numbers = text.strip_prefix('v').unwrap_or(text).split('.');
        let mut next = || -> Result<u64, Error> {
            numbers
                .next()
                .ok_or_else(invalid)?
                .parse()
                .map_err(|_| invalid())
        };
        let version = Self(next()?, next()?, next()?);
        match numbers.next() {
            Some(_) => Err(invalid()),
            None => Ok(version),
        }
    }

    pub fn this() -> Result<Self, Error> {
        Self::parse(env!("CARGO_PKG_VERSION"))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
}

pub fn latest() -> Result<Version, Error> {
    parse(&get(LATEST, &[])?)
}

fn parse(json: &str) -> Result<Version, Error> {
    let release: Release = serde_json::from_str(json).map_err(Error::UnexpectedData)?;
    Version::parse(&release.tag_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_latest_release() {
        let version = parse(include_str!("../../tests/data/github_release.json")).unwrap();

        assert_eq!(version, Version(0, 2, 0));
    }

    #[test]
    fn compares_versions_by_number() {
        let older = Version::parse("0.9.0").unwrap();
        let newer = Version::parse("v0.10.0").unwrap();

        assert!(older < newer);
        assert_eq!(newer.to_string(), "0.10.0");
    }

    #[test]
    fn rejects_other_versions() {
        assert!(Version::parse("0.3").is_err());
        assert!(Version::parse("0.3.0.1").is_err());
        assert!(Version::parse("0.3.0-beta").is_err());
    }

    #[test]
    fn this_version_is_valid() {
        assert!(Version::this().is_ok());
    }
}
