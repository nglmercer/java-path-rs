//! Parsing and ordering of Java version strings.

use crate::error::{Error, Result};
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

/// A parsed Java version.
///
/// Handles the legacy `1.8.0_412-b08` scheme and JEP 322 version numbers,
/// which are sequences of arbitrarily many numeric elements — `17.0.10.1` is
/// a distinct, later release than `17.0.10`.
///
/// `Eq`, `Hash` and `Ord` all agree, and all ignore the original string, so
/// `17` and `17.0.0` are the same version in a `BTreeSet` and a `HashSet`
/// alike. Use [`JavaVersion::raw`] to recover exactly what was parsed.
///
/// With the `serde` feature a version is represented as its version *string*
/// and read back through [`JavaVersion::parse`]. Deriving the impls would let
/// deserialization construct component vectors the parser can never produce
/// (`[17, 0, 0]`), which compare equal to `17` but hash differently — exactly
/// the `Eq`/`Hash` contract violation the normalisation above prevents.
#[derive(Debug, Clone)]
pub struct JavaVersion {
    /// Numeric elements with trailing zeros removed, so `17.0.0` and `17`
    /// share one representation.
    components: Vec<u32>,
    /// Pre-release qualifier such as `ea`.
    pre: Option<String>,
    /// Build number, when present (`+7`, `-b08`).
    build: Option<u32>,
    /// The original, unmodified string.
    raw: String,
}

impl JavaVersion {
    /// Parse a version string such as `1.8.0_412`, `17.0.10.1+7` or `21`.
    pub fn parse(input: &str) -> Result<Self> {
        let raw = input.trim();
        if raw.is_empty() {
            return Err(Error::InvalidVersion(input.to_string()));
        }

        let (core, build, pre) = split_qualifiers(raw)?;

        let mut components = Vec::new();
        for element in core.split('.') {
            components.push(parse_num(element, raw)?);
        }

        // Legacy `1.N...` numbering: drop the leading 1 so the feature
        // version is first, matching modern numbering.
        if components.first() == Some(&1) && components.len() > 1 {
            components.remove(0);
        }

        // The legacy `_412` update suffix is a further numeric element.
        if let Some(rest) = raw.split('_').nth(1) {
            if let Some(update) = rest
                .split(|c: char| !c.is_ascii_digit())
                .find(|s| !s.is_empty())
            {
                let update = parse_num(update, raw)?;
                while components.len() < 2 {
                    components.push(0);
                }
                components.truncate(2);
                components.push(update);
            }
        }

        if components.first().copied().unwrap_or(0) == 0 {
            return Err(Error::InvalidVersion(input.to_string()));
        }

        // Normalise so 17 and 17.0.0 are one version.
        while components.len() > 1 && components.last() == Some(&0) {
            components.pop();
        }

        Ok(JavaVersion {
            components,
            pre,
            build,
            raw: raw.to_string(),
        })
    }

    /// Feature/major release (8, 11, 17, 21, ...).
    pub fn major(&self) -> u32 {
        self.components.first().copied().unwrap_or(0)
    }

    /// Interim/minor release.
    pub fn minor(&self) -> u32 {
        self.component(1)
    }

    /// Update release.
    pub fn patch(&self) -> u32 {
        self.component(2)
    }

    /// The `n`th numeric element, counting from zero. Missing elements read
    /// as `0`, so `17` and `17.0.0` behave identically.
    pub fn component(&self, n: usize) -> u32 {
        self.components.get(n).copied().unwrap_or(0)
    }

    /// All numeric elements, trailing zeros removed.
    pub fn components(&self) -> &[u32] {
        &self.components
    }

    /// Build number, when the version carried one.
    pub fn build(&self) -> Option<u32> {
        self.build
    }

    /// Pre-release qualifier, lowercased.
    pub fn pre(&self) -> Option<&str> {
        self.pre.as_deref()
    }

    /// The string this version was parsed from.
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// `true` when this is an early-access / pre-release build.
    pub fn is_prerelease(&self) -> bool {
        self.pre.is_some()
    }
}

/// Compare numeric elements, treating missing trailing elements as zero.
fn cmp_components(a: &[u32], b: &[u32]) -> Ordering {
    let len = a.len().max(b.len());
    for i in 0..len {
        let left = a.get(i).copied().unwrap_or(0);
        let right = b.get(i).copied().unwrap_or(0);
        match left.cmp(&right) {
            Ordering::Equal => {}
            other => return other,
        }
    }
    Ordering::Equal
}

impl Ord for JavaVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        cmp_components(&self.components, &other.components)
            // A general-availability build sorts above a pre-release.
            .then_with(|| self.pre.is_none().cmp(&other.pre.is_none()))
            .then_with(|| self.pre.cmp(&other.pre))
            .then_with(|| self.build.cmp(&other.build))
    }
}

impl PartialOrd for JavaVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// Eq and Hash are defined in terms of Ord so the three never disagree.
impl PartialEq for JavaVersion {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for JavaVersion {}

impl Hash for JavaVersion {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Must hash exactly what `eq` compares: the normalised elements with
        // trailing zeros already removed, never `raw`.
        self.components.hash(state);
        self.pre.hash(state);
        self.build.hash(state);
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

    // The legacy `_412` update suffix is handled separately by the caller.
    let core = head.split('_').next().unwrap_or(head);
    if core.is_empty() {
        return Err(Error::InvalidVersion(raw.to_string()));
    }
    Ok((core.to_string(), build, pre))
}

impl FromStr for JavaVersion {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        JavaVersion::parse(s)
    }
}

impl fmt::Display for JavaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for JavaVersion {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.raw)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for JavaVersion {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct Visitor;

        impl serde::de::Visitor<'_> for Visitor {
            type Value = JavaVersion;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a java version string such as \"21.0.3+9\"")
            }

            fn visit_str<E: serde::de::Error>(
                self,
                value: &str,
            ) -> std::result::Result<JavaVersion, E> {
                // The parser is the only way to build a JavaVersion, here as
                // everywhere else, so its invariants cannot be bypassed.
                JavaVersion::parse(value).map_err(serde::de::Error::custom)
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}
