//! Parsing and ordering of Java version strings.

use crate::error::{Error, Result};
use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

/// A parsed Java version.
///
/// Handles both the legacy `1.8.0_412-b08` scheme and the JEP 223 scheme
/// (`11.0.22+7`, `17.0.10`, `21`, `22-ea+15`).
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct JavaVersion {
    /// Feature/major release (8, 11, 17, 21, ...).
    pub major: u32,
    /// Interim/minor release.
    pub minor: u32,
    /// Update/security release.
    pub patch: u32,
    /// Build number, when present (`+7`, `-b08`).
    pub build: Option<u32>,
    /// Pre-release qualifier such as `ea`.
    pub pre: Option<String>,
    /// The original, unmodified string.
    pub raw: String,
}

impl JavaVersion {
    /// Parse a version string such as `1.8.0_412`, `17.0.10+7` or `21`.
    pub fn parse(input: &str) -> Result<Self> {
        let raw = input.trim();
        if raw.is_empty() {
            return Err(Error::InvalidVersion(input.to_string()));
        }

        // Split off build metadata: `+7`, `-b08`, `_412` is handled separately.
        let (core, mut build, mut pre) = split_qualifiers(raw)?;

        let mut nums = core.split('.');
        let first: u32 = parse_num(nums.next().unwrap_or_default(), raw)?;

        let (major, minor, patch) = if first == 1 {
            // Legacy: 1.8.0_412 -> major 8
            let major = parse_num(nums.next().unwrap_or("0"), raw)?;
            let patch = nums.next().map(|n| parse_num(n, raw)).transpose()?;
            (major, 0, patch.unwrap_or(0))
        } else {
            let minor = nums.next().map(|n| parse_num(n, raw)).transpose()?;
            let patch = nums.next().map(|n| parse_num(n, raw)).transpose()?;
            (first, minor.unwrap_or(0), patch.unwrap_or(0))
        };

        if major == 0 {
            return Err(Error::InvalidVersion(input.to_string()));
        }

        // Legacy update suffix `_412` is really the patch level for Java 8.
        let (major, minor, patch) = if let Some(rest) = raw.split('_').nth(1) {
            let update = rest
                .split(|c: char| !c.is_ascii_digit())
                .find(|s| !s.is_empty())
                .map(|n| parse_num(n, raw))
                .transpose()?;
            (major, minor, update.unwrap_or(patch))
        } else {
            (major, minor, patch)
        };

        if pre.as_deref() == Some("") {
            pre = None;
        }
        if build == Some(u32::MAX) {
            build = None;
        }

        Ok(JavaVersion {
            major,
            minor,
            patch,
            build,
            pre,
            raw: raw.to_string(),
        })
    }

    /// `true` when this is an early-access / pre-release build.
    pub fn is_prerelease(&self) -> bool {
        self.pre.is_some()
    }

    /// Ordering key ignoring the raw string.
    fn key(&self) -> (u32, u32, u32, u8, u32) {
        (
            self.major,
            self.minor,
            self.patch,
            // GA sorts above pre-release.
            u8::from(self.pre.is_none()),
            self.build.unwrap_or(0),
        )
    }
}

fn parse_num(s: &str, raw: &str) -> Result<u32> {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return Err(Error::InvalidVersion(raw.to_string()));
    }
    digits
        .parse()
        .map_err(|_| Error::InvalidVersion(raw.to_string()))
}

/// Returns `(numeric core, build, pre-release)`.
fn split_qualifiers(raw: &str) -> Result<(String, Option<u32>, Option<String>)> {
    let mut build = None;
    let mut pre = None;

    // `+NN` build metadata.
    let (head, plus) = match raw.split_once('+') {
        Some((h, b)) => (h, Some(b)),
        None => (raw, None),
    };
    if let Some(b) = plus {
        build = b
            .split(|c: char| !c.is_ascii_digit())
            .find(|s| !s.is_empty())
            .and_then(|n| n.parse().ok());
    }

    // `-ea`, `-b08`, `-LTS` qualifiers.
    let head = match head.split_once('-') {
        Some((h, q)) => {
            let q = q.trim();
            if let Some(rest) = q.strip_prefix('b') {
                if let Ok(n) = rest.parse::<u32>() {
                    build = Some(n);
                }
            } else if !q.is_empty() {
                pre = Some(q.to_ascii_lowercase());
            }
            h
        }
        None => head,
    };

    // Legacy `_412` update suffix is stripped from the numeric core.
    let core = head.split('_').next().unwrap_or(head);
    if core.is_empty() {
        return Err(Error::InvalidVersion(raw.to_string()));
    }
    Ok((core.to_string(), build, pre))
}

impl Ord for JavaVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key().cmp(&other.key())
    }
}

impl PartialOrd for JavaVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl FromStr for JavaVersion {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        JavaVersion::parse(s)
    }
}

impl fmt::Display for JavaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}
