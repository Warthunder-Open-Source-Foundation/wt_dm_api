use std::path::{Path, PathBuf};

#[derive(
	Debug,
	strum::Display,
	strum::EnumString,
	strum::VariantArray,
	Eq,
	PartialEq,
	Hash,
	Clone,
	Copy,
	strum::AsRefStr,
	strum::IntoStaticStr,
)]
pub enum VromfType {
	#[strum(serialize = "aces.vromfs.bin")]
	Aces,
	#[strum(serialize = "char.vromfs.bin")]
	Char,
	#[strum(serialize = "game.vromfs.bin")]
	Game,
	#[strum(serialize = "gui.vromfs.bin")]
	Gui,
	#[strum(serialize = "lang.vromfs.bin")]
	Lang,
	#[strum(serialize = "mis.vromfs.bin")]
	Mis,
	#[strum(serialize = "regional.vromfs.bin")]
	Regional,
	#[strum(serialize = "wwdata.vromfs.bin")]
	Wwdata,
}

impl From<VromfType> for PathBuf {
	fn from(value: VromfType) -> Self {
		Into::<&Path>::into(value).to_path_buf()
	}
}

impl From<&VromfType> for PathBuf {
	fn from(value: &VromfType) -> Self {
		Into::<&Path>::into(value).to_path_buf()
	}
}

impl From<VromfType> for &Path {
	fn from(value: VromfType) -> Self {
		Path::new(Into::<&'static str>::into(value))
	}
}

impl From<&VromfType> for &Path {
	fn from(value: &VromfType) -> Self {
		Path::new(Into::<&'static str>::into(value))
	}
}
