/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use buck2_core::fs::fs_util;
use serde::Deserialize;

const XCODE_SELECT_SYMLINK: &str = "/var/db/xcode_select_link";

/// Only fields we care about from Xcode Info.plist.
#[derive(Deserialize)]
#[allow(non_snake_case)]
struct XcodeInfoPlist {
    CFBundleShortVersionString: String,
}

/// Versioning information for the currently selected Xcode on the host machine.
#[derive(Debug, Default, PartialEq)]
pub struct XcodeVersionInfo {
    /// e.g. "14.0.1"
    pub version_string: Option<String>,
    /// The "14" in "14.0.1"
    pub major_version: Option<String>,
    /// The "0" in "14.0.1"
    pub minor_version: Option<String>,
    /// The "1" in "14.0.1"
    pub patch_version: Option<String>,
    /// Xcode-specific build number like "14A309"
    pub build_number: Option<String>,
}

impl XcodeVersionInfo {
    // Construct from Info.plist in root of Xcode install dir.
    pub fn new() -> anyhow::Result<Self> {
        let resolved_xcode_path = fs_util::canonicalize(&PathBuf::from(XCODE_SELECT_SYMLINK))
            .context("resolve selected xcode link")?;
        let plist_path = resolved_xcode_path
            .parent()
            .map(|base| base.join("Info.plist"))
            .ok_or_else(|| anyhow::anyhow!("unable to construct path to Xcode Info.plist"))?;
        Self::from_info_plist(&plist_path)
    }

    pub(crate) fn from_info_plist(plist_path: &Path) -> anyhow::Result<Self> {
        let plist: XcodeInfoPlist =
            plist::from_file(plist_path).context("deserializing Xcode Info.plist")?;

        let version_parts = &mut plist.CFBundleShortVersionString.split('.');
        let major = version_parts.next().map(|v| v.to_owned());
        let minor = version_parts.next().map(|v| v.to_owned());
        let patch = version_parts
            .next()
            .map(|v| v.to_owned())
            .or_else(|| Some("0".to_owned()));

        Ok(Self {
            version_string: Some(plist.CFBundleShortVersionString),
            major_version: major,
            minor_version: minor,
            patch_version: patch,
            // Build Identifier isn't actually stored in Info.plist. Weird.
            build_number: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn write_plist(plist_content: &str) -> (tempfile::TempDir, PathBuf) {
        let workspace = tempfile::tempdir().expect("failed to create tempdir");
        let fake_xcode_dir = workspace.path().join("xcode_foo_bar.app").join("Contents");
        fs::DirBuilder::new()
            .recursive(true)
            .create(&fake_xcode_dir)
            .unwrap();
        let plist_path = fake_xcode_dir.join("Info.plist");
        fs::write(&plist_path, plist_content).expect("failed to write plist");
        (workspace, plist_path)
    }

    #[test]
    fn test_resolves_version_from_plist() {
        let (_t, plist) = write_plist(
            r#"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIconName</key>
    <string>Xcode</string>
    <key>CFBundleShortVersionString</key>
    <string>14.0.1</string>
</dict>
</plist>
        "#,
        );

        let got = XcodeVersionInfo::from_info_plist(&plist).expect("failed to parse version info");
        let want = XcodeVersionInfo {
            version_string: Some("14.0.1".to_owned()),
            major_version: Some("14".to_owned()),
            minor_version: Some("0".to_owned()),
            patch_version: Some("1".to_owned()),
            build_number: None,
        };
        assert_eq!(want, got);
    }

    #[test]
    fn test_resolves_version_from_plist_no_patch_version() {
        let (_t, plist) = write_plist(
            r#"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIconName</key>
    <string>Xcode</string>
    <key>CFBundleShortVersionString</key>
    <string>14.0</string>
</dict>
</plist>
        "#,
        );

        let got = XcodeVersionInfo::from_info_plist(&plist).expect("failed to parse version info");
        let want = XcodeVersionInfo {
            version_string: Some("14.0".to_owned()),
            major_version: Some("14".to_owned()),
            minor_version: Some("0".to_owned()),
            patch_version: Some("0".to_owned()),
            build_number: None,
        };
        assert_eq!(want, got);
    }

    #[test]
    fn test_resolves_version_from_plist_beta() {
        let (_t, plist) = write_plist(
            r#"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIconName</key>
    <string>XcodeBeta</string>
    <key>CFBundleShortVersionString</key>
    <string>14.0</string>
</dict>
</plist>
        "#,
        );

        let got = XcodeVersionInfo::from_info_plist(&plist).expect("failed to parse version info");
        let want = XcodeVersionInfo {
            version_string: Some("14.0".to_owned()),
            major_version: Some("14".to_owned()),
            minor_version: Some("0".to_owned()),
            patch_version: Some("0".to_owned()),
            build_number: None,
        };
        assert_eq!(want, got);
    }
}
