use std::collections::HashMap;

use dashmap::DashMap;
use wt_version::Version;

use crate::vromf_enum::VromfType;

pub struct BinaryVromfs {
	binaries:     DashMap<Version, HashMap<VromfType, Vec<u8>>>,
	commit_pages: DashMap<Version, String>,
}
