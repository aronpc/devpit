//! Host profiles: the machines a project's terminals can run on, over SSH.

use serde::{Deserialize, Serialize};
use specta::Type;

/// What a host is for. Production is marked wherever a host shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum HostEnvironment {
    Dev,
    Staging,
    Production,
}

/// A host profile as the window edits it: no id for a new one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HostDraft {
    pub id: Option<String>,
    pub name: String,
    /// An ssh config alias or a host name.
    pub target: String,
    pub user: Option<String>,
    pub port: Option<u16>,
    /// The identity file's path, never its contents.
    pub identity_file: Option<String>,
    pub work_root: String,
    pub environment: HostEnvironment,
}

/// One saved host profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Host {
    pub id: String,
    pub name: String,
    pub target: String,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<String>,
    pub work_root: String,
    pub environment: HostEnvironment,
}

/// A list, so tomorrow's field has somewhere to go.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Hosts {
    pub hosts: Vec<Host>,
}

/// One line of a host's check: what was checked, whether it passed, and what
/// was found or why not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HostCheckRow {
    pub what: String,
    pub ok: bool,
    pub said: String,
}

/// A host's check, ssh first: when it fails, nothing else was asked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HostCheck {
    pub rows: Vec<HostCheckRow>,
}

/// Where a project is on one host, with the host's name and environment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HostLink {
    pub host_id: String,
    pub host_name: String,
    pub environment: HostEnvironment,
    pub path: String,
}

/// A project's links to hosts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HostLinks {
    pub links: Vec<HostLink>,
}
