//! Ephemeral whole-file OpenWiki checks, never native-record verification.
//! Contract: langchain-ai/openwiki@0b1f07cc622156ac0efe8e28437029bc9427e826,
//! src/claims/evidence/repository/{resource,resolver}.ts.
use serde::{Deserialize, Serialize};

pub const QUALIFICATION: &str = "A match means the same bytes at check time only; no test execution, runtime proof, or documentary authority. Changed or unknown reports require a current-source check or permitted targeted inspection before resolving a consequential disagreement.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessStatus {
    MatchesCheckout,
    ChangedSinceSnapshot,
    UnknownOrUnchecked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessReason {
    InspectionNotAuthorized,
    CheckoutUnavailable,
    UnsupportedLocator,
    UnsupportedVersion,
    InvalidPath,
    PathUnavailableOrExcluded,
    BudgetExhausted,
    InspectionFailed,
    ContentChanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckoutFreshness {
    pub evidence_id: String,
    pub resource: String,
    pub version: String,
    pub status: FreshnessStatus,
    pub reason: Option<FreshnessReason>,
    /// Exact checkout-relative path; present only for a successfully read file.
    pub checked_path: Option<String>,
    pub content_hash: Option<String>,
    pub observation_id: Option<String>,
    pub qualification: String,
}

pub(super) fn whole_file_digest(version: &str) -> Option<&str> {
    let digest = version.strip_prefix("repo-file-v1:")?;
    let hex = digest.strip_prefix("sha256:")?;
    (hex.len() == 64
        && hex
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)))
    .then_some(digest)
}

pub(super) fn whole_file_path(resource: &str) -> Result<String, FreshnessReason> {
    let path = resource
        .strip_prefix("repo://")
        .filter(|p| !p.contains('#'))
        .ok_or(FreshnessReason::UnsupportedLocator)?;
    // Only canonical persisted whole-file locators are supported. Decode once;
    // never normalize traversal or interpret a URI host as a repository name.
    for (i, byte) in path.bytes().enumerate() {
        if byte == b'%'
            && !path
                .as_bytes()
                .get(i + 1..i + 3)
                .is_some_and(|s| s.iter().all(u8::is_ascii_hexdigit))
        {
            return Err(FreshnessReason::InvalidPath);
        }
    }
    let decoded = percent_encoding::percent_decode_str(path)
        .decode_utf8()
        .map_err(|_| FreshnessReason::InvalidPath)?;
    if !super::inspection::valid_relative(&decoded)
        || decoded
            .split('/')
            .next()
            .is_some_and(|p| p.eq_ignore_ascii_case("openwiki"))
    {
        return Err(FreshnessReason::InvalidPath);
    }
    Ok(decoded.into_owned())
}
