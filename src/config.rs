use std::fs;
use std::ptr::eq;
use nwg::Window;

#[derive(serde_derive::Deserialize, serde_derive::Serialize, Debug, Eq, Clone, Copy)]
pub struct WindowBox {
	pub x: i32,
	pub y: i32,
	pub width: u32,
	pub height: u32,
}

impl WindowBox {
	pub fn new(window: &Window) -> Self {
		let (x, y) = window.position();
		let (width, height) = window.size();
		Self {
			x,
			y,
			width,
			height,
		}
	}
}

impl PartialEq for WindowBox {
	fn eq(&self, other: &WindowBox) -> bool {
		return
			self.x == other.x &&
				self.y == other.y &&
				self.width == other.width &&
				self.height == other.height;
	}
}

pub fn readWindowBox() -> Option<WindowBox> {
	return fs::read_to_string("./WindowState.json")
		.map_err(|err| {
			println!("Config failed to read: {}", err);
		})
		.and_then(|data| serde_json::from_str(&data).map_err(|err| {
			println!("Config malformed: {}", err);
		})).ok();
}

pub fn writeWindowBox(wbox: &WindowBox) {
	fs::write("./WindowState.json", serde_json::to_string_pretty(wbox).unwrap()).map_err(|err| {
		println!("Failed to save config: {}", err);
	}).ok();
}
