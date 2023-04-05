#![allow(non_snake_case)]
#![allow(unused_imports)]

extern crate native_windows_derive as nwd;
extern crate native_windows_gui as nwg;

use std::{env, fs, thread};
use std::cell::Cell;
use std::cmp::min;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::ops::{BitAnd, Deref};
use std::ptr::eq;
use std::rc::Rc;
use std::sync::{Arc, Mutex, RwLock};
use std::thread::Thread;
use std::time::Duration;

use nwd::NwgUi;
use nwg::{Event, EventHandler, fatal_message, NativeUi, Window, WindowBuilder, WindowFlags};
use nwg::MessageChoice::Retry;
use nwg::stretch::{geometry::{Rect, Size}, style::{AlignSelf, Dimension as D, FlexDirection}};

use config::WindowBox;

use crate::basic_app_ui::BasicAppUi;

mod cursed;
mod config;
mod ui;

#[derive(Debug, Clone)]
struct WindowInfo {
	winBox: WindowBox,
	destroyed: bool,
}

impl WindowInfo {
	fn new(winBox: WindowBox) -> Self {
		Self {
			winBox: winBox,
			destroyed: false,
		}
	}
}


#[derive(Default, NwgUi)]
pub struct BasicApp {
	#[nwg_control(title: "Rexplorer", flags: "WINDOW|MINIMIZE_BOX|MAXIMIZE_BOX|RESIZABLE")]
	// #[nwg_events(OnWindowClose: [BasicApp::say_goodbye])]
	window: Window,
	
	#[nwg_layout(parent: window, flex_direction: FlexDirection::Column)]
	grid: nwg::FlexboxLayout,
	
	#[nwg_control(text: "Heisenberg", focus: true)]
	#[nwg_layout_item(layout: grid, flex_grow: 0.0, min_size: Size { width: D::Points(60.0), height: D::Points(60.0)})]
	name_edit: nwg::TextInput,
	
	#[nwg_control(text: "Say my name")]
	#[nwg_layout_item(layout: grid, flex_grow: 1.0)]
	#[nwg_events(OnButtonClick: [BasicApp::say_hello])]
	hello_button: nwg::Button,
}

impl BasicApp {
	fn say_hello(&self) {
		nwg::modal_info_message(&self.window, "Hello", &format!("Hello {}", self.name_edit.text()));
	}
	
	fn say_goodbye(&self) {
		nwg::stop_thread_dispatch();
	}
}


fn main() {
	nwg::init().expect("Failed to init Native Windows GUI");
	nwg::Font::set_global_family("Segoe UI").expect("Failed to set default font");
	
	let orgState = config::readWindowBox().filter(|s| {
		return s.width as i32 + s.x >= 0 &&
			s.height as i32 + s.y >= 0;
	});
	
	let res = ui::makeMain().map_err(|err| {
		println!("Failed to make ui; {err}");
	});
	if res.is_err() { return; }
	let window = res.unwrap();
	
	match orgState {
		None => {
			window.setSize(800, 600);
			ui::centerWindow(&window);
		}
		Some(state) => {
			window.setSize(state.width, state.height);
			window.setPosition(state.x, state.y);
		}
	}
	
	
	let info = WindowInfo::new(WindowBox::new(&window));
	let info = Arc::new(Mutex::new(info));
	
	
	let window = Rc::new(window);
	let fun;
	{
		let info = info.clone();
		let window = window.clone();
		fun = move |e, _data, _handle| {
			match e {
				Event::OnResize | Event::OnMove => {
					match window.getWindowPlacementMode().unwrap_or(cursed::PlacementMode::MAXIMIZED)
					{
						cursed::PlacementMode::REGULAR => {}
						cursed::PlacementMode::MINIMIZED | cursed::PlacementMode::MAXIMIZED => {
							return;
						}
					}
					
					let b = WindowBox::new(&window);
					if min(b.width, b.height) <= 0 { return; }
					
					info.lock().unwrap().winBox = b;
				}
				_ => {}
			}
		};
	}
	
	window.setVisible(true);
	let handler = nwg::full_bind_event_handler(&window.window.handle, fun);
	watchState(&info, orgState);
	
	nwg::dispatch_thread_events();
	nwg::unbind_event_handler(&handler);
	
	let mut info = info.lock().unwrap();
	config::writeWindowBox(&info.winBox);
	info.destroyed = true;
}

fn watchState(window: &Arc<Mutex<WindowInfo>>, orgState: Option<WindowBox>) {
	let window = window.clone();
	thread::spawn(move || {
		let mut orgState = orgState.clone();
		while true {
			thread::sleep(Duration::from_secs(1));
			
			let state: WindowBox;
			{
				let window = window.lock().unwrap();
				if window.destroyed {
					return;
				}
				state = window.winBox.clone();
			}
			
			
			if orgState.filter(|s| { s.eq(&state) }).is_some() {
				continue;
			}
			
			//println!("Change:\n{:?}\n{:?}", Some(state), orgState);
			
			orgState = Some(state);
			
			config::writeWindowBox(&state);
		}
	});
}
