use std::path::{Path, PathBuf};

#[derive(Debug, strum::Display, strum::EnumString, strum::VariantArray, Eq, PartialEq, Hash, Clone, Copy, strum::AsRefStr, strum::IntoStaticStr)]
pub enum VromfType {
	#[strum(to_string = "aces.vromfs.bin", serialize = "aces.vromfs.bin")]
	Aces,
	#[strum(to_string = "char.vromfs.bin", serialize = "char.vromfs.bin")]
	Char,
	#[strum(to_string = "game.vromfs.bin", serialize = "game.vromfs.bin")]
	Game,
	#[strum(to_string = "gui.vromfs.bin", serialize = "gui.vromfs.bin")]
	Gui,
	#[strum(to_string = "lang.vromfs.bin", serialize = "lang.vromfs.bin")]
	Lang,
	#[strum(to_string = "mis.vromfs.bin", serialize = "mis.vromfs.bin")]
	Mis,
	#[strum(to_string = "regional.vromfs.bin", serialize = "regional.vromfs.bin")]
	Regional,
	#[strum(to_string = "wwdata.vromfs.bin", serialize = "wwdata.vromfs.bin")]
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
