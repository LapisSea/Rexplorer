use std::{fs, thread};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

use slint::Window;

#[derive(Debug, Clone)]
pub struct WindowInfo {
	pub winBox: WindowBox,
	pub destroyed: bool,
}

impl WindowInfo {
	pub fn new(winBox: WindowBox) -> Self {
		Self {
			winBox,
			destroyed: false,
		}
	}
}

#[derive(serde_derive::Deserialize, serde_derive::Serialize, Debug, Eq, Clone, Copy)]
pub struct WindowBox {
	pub x: i32,
	pub y: i32,
	pub width: u32,
	pub height: u32,
}

impl WindowBox {
	pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
		Self {
			x,
			y,
			width,
			height,
		}
	}
	
	pub fn fromWindow(window: &Window) -> Self {
		let pos = window.position();
		let siz = window.size();
		Self {
			x: pos.x,
			y: pos.y,
			width: siz.width,
			height: siz.height,
		}
	}
}

impl PartialEq for WindowBox {
	fn eq(&self, other: &WindowBox) -> bool {
			self.x == other.x &&
				self.y == other.y &&
				self.width == other.width &&
				self.height == other.height
	}
}

pub fn readWindowBox() -> Option<WindowBox> {
	fs::read_to_string("./WindowState.json")
		.map_err(|err| {
			println!("Config failed to read: {}", err);
		})
		.and_then(|data| serde_json::from_str(&data).map_err(|err| {
			println!("Config malformed: {}", err);
		}))
		.ok()
		.filter(validateData)
}

fn validateData(s: &WindowBox) -> bool {
	s.width as i32 + s.x >= 0 && s.height as i32 + s.y >= 0
}

pub fn writeWindowBox(wbox: &WindowBox) {
	fs::write("./WindowState.json", serde_json::to_string_pretty(wbox).unwrap()).map_err(|err| {
		println!("Failed to save config: {}", err);
	}).ok();
}


pub fn watchState(window: Arc<Mutex<WindowInfo>>, orgState: Option<WindowBox>) {
	thread::spawn(move || {
		let mut orgState = orgState;
		loop {
			sleep(Duration::from_secs(2));
			
			let state: WindowBox;
			{
				let window = window.lock().unwrap();
				if window.destroyed {
					return;
				}
				state = window.winBox;
			}
			
			
			if orgState.filter(|s| { s.eq(&state) }).is_some() {
				continue;
			}
			
			//println!("Change:\n{:?}\n{:?}", Some(state), orgState);
			
			orgState = Some(state);
			
			writeWindowBox(&state);
		}
	});
}
