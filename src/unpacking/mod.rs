mod binary_vromfs;

use std::sync::Arc;

use octocrab::{Octocrab, OctocrabBuilder};
use tokio::sync::Mutex;

use crate::{
	app_state::AppState,
	endpoints::{
		files::{FileRequest, UnpackedVromfs},
		get_vromfs::VromfCache,
	},
	error::ApiError,
};

/// Super high level struct exposing all API dedicated functionality
pub struct Vromfs {
	// Contains binary VROMFs requested from github
	pub vromf_cache:     VromfCache,
	pub octocrab:        Mutex<Octocrab>,
	// Initialized unpackers per VROMF
	pub unpacked_vromfs: UnpackedVromfs,
}

impl Vromfs {
	pub fn new(octocrab: Octocrab) -> Self {
		Self {
			vromf_cache:     Default::default(),
			octocrab:        Mutex::new(octocrab),
			unpacked_vromfs: Default::default(),
		}
	}
}
