#![allow(non_snake_case)]
#![allow(dead_code)]

use std::{env, fmt, fs, thread};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::Display;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use slint;
use slint::{ModelRc, PhysicalPosition, PhysicalSize, SharedString, SharedVector, WindowPosition, WindowSize};
use slint::private_unstable_api::re_exports::SharedVectorModel;

use config::WindowBox;

use crate::rgba_img::RgbImg;

mod config;
mod rgba_img;
mod work;

slint::include_modules!();

#[derive(Debug, Clone)]
struct WindowInfo {
	winBox: WindowBox,
	destroyed: bool,
}

impl WindowInfo {
	fn new(winBox: WindowBox) -> Self {
		Self {
			winBox,
			destroyed: false,
		}
	}
}

enum LoadStage<T> {
	Loading,
	Loaded(T),
}

struct State {
	iconCache: HashMap<String, LoadStage<Arc<RgbImg>>>,
	folderIco: Arc<RgbImg>,
	defaultIco: Arc<RgbImg>,
	
}

impl Display for UIFile {
	fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
		fmt.write_str(self.name.as_str())?;
		fmt.write_str(" -> '")?;
		fmt.write_str(self.fullPath.as_str())?;
		fmt.write_str("'")?;
		Ok(())
	}
}

fn main() {
	let app = Rc::new(HomeApp::new().unwrap());
	let win = app.window();
	
	let state = Arc::new(Mutex::new(State {
		iconCache: HashMap::new(),
		folderIco: Arc::new(RgbImg::read(">>win-folder.png").unwrap()),
		defaultIco: Arc::new(RgbImg::read(">>default.png").unwrap()),
	}));
	
	{
		let eApp = app.clone();
		let state = state.clone();
		app.on_onFileOpen(move |f| {
			let f = f.as_str();
			match fetchInfo(state.clone(), f) {
				PathInfo::Fail(d) => {
					println!("{}: {}", d.fullPath.as_str(), d.status.as_str())
				}
				PathInfo::Dir(d) => { eApp.set_data(d); }
				PathInfo::File => {
					if let Err(err) = open::that(f) {
						println!("{}", err);
					}
				}
			}
			eApp.window().request_redraw();
		});
	}
	
	match fetchInfo(state, "C:\\Users\\LapisSea\\Desktop") {
		PathInfo::Fail(d) => { app.set_data(d) }
		PathInfo::Dir(d) => { app.set_data(d) }
		PathInfo::File => {}
	}
	
	// if let Some(path) = home::home_dir() {
	// 	let mut iconCache = HashMap::new();
	// 	match fetchInfo(&mut iconCache, format!("{}", path.display()).as_str()) {
	// 		PathInfo::Fail(d) => { app.set_data(d) }
	// 		PathInfo::Dir(d) => { app.set_data(d) }
	// 		PathInfo::File => {}
	// 	}
	// }
	
	let s = config::readWindowBox().unwrap_or_else(|| WindowBox::new(100, 100, 800, 600));
	
	win.set_size(WindowSize::Physical(PhysicalSize::new(s.width, s.height)));
	win.set_position(WindowPosition::Physical(PhysicalPosition::new(s.x, s.y)));
	
	app.run().unwrap();
}


enum PathInfo {
	Fail(UIDirectoryInfo),
	Dir(UIDirectoryInfo),
	File,
}

fn loadFromPath(state: Arc<Mutex<State>>, path: PathBuf) -> Arc<RgbImg> {
	match fs::metadata(path.clone()).ok() {
		None => {
			return state.lock().unwrap().defaultIco.clone();
		}
		Some(meta) => {
			if meta.is_dir() {
				return state.lock().unwrap().folderIco.clone();
			}
		}
	}
	
	let mut icon = None;
	if path.extension().and_then(|s| s.to_str()).filter(|s| ["jpg", "png"].contains(s)).is_some() {
		icon = RgbImg::read(path.as_os_str().to_str().unwrap()).ok();
	}
	
	icon.map(|u| Arc::new(u))
	    .unwrap_or_else(|| state.lock().unwrap().defaultIco.clone())
}

fn fetchInfo(state: Arc<Mutex<State>>, path: &str) -> PathInfo {
	return match fs::read_dir(path) {
		Ok(rd) => {
			let paths: Vec<PathBuf> = rd.into_iter().filter_map(|p| p.ok()).map(|p| p.path()).collect();
			
			for path in paths.clone() {
				let state = state.clone();
				{
					let mut state = state.lock().unwrap();
					let strPath = path.to_str().unwrap();
					match state.iconCache.entry(strPath.to_string()) {
						Entry::Occupied(e) => {
							if let LoadStage::Loaded(_) = e.get() {
								continue;
							}
						}
						Entry::Vacant(e) => {
							e.insert(LoadStage::Loading);
						}
					};
				}
				
				work::execute(move || {
					let strPath = path.to_str().unwrap();
					println!("Async loading {}", strPath);
					let img = loadFromPath(state.clone(), path.clone());
					let mut state = state.lock().unwrap();
					state.iconCache.insert(strPath.to_string(), LoadStage::Loaded(img));
					println!("Async loaded {}", strPath);
				});
			}
			
			let mut data: SharedVector<UIFile> = Default::default();
			for path in paths {
				let icon;
				let strPath = path.to_str().unwrap();
				loop {
					let state = state.lock().unwrap();
					match state.iconCache.get(strPath).unwrap() {
						LoadStage::Loading => {
							drop(state);
							thread::sleep(Duration::from_millis(5));
							continue;
						}
						LoadStage::Loaded(r) => {
							icon = r.clone();
							// println!("Loaded {}", strPath);
							break;
						}
					};
				}
				
				data.push(UIFile {
					fullPath: SharedString::from(path.to_str().unwrap()),
					icon: icon.asImage(),
					name: SharedString::from(path.file_name().unwrap().to_str().unwrap()),
				});
			}
			
			
			return PathInfo::Dir(UIDirectoryInfo {
				files: ModelRc::new(SharedVectorModel::from(data)),
				fullPath: SharedString::from(path),
				status: SharedString::from("Nothing here"),
			});
		}
		Err(err) => {
			if let Some(meta) = fs::metadata(path).ok() {
				if meta.is_file() {
					return PathInfo::File;
				}
			}
			PathInfo::Fail(UIDirectoryInfo {
				files: Default::default(),
				fullPath: SharedString::from(path),
				status: SharedString::from(format!("{}", err)),
			})
		}
	};
}

fn watchState(window: &Arc<Mutex<WindowInfo>>, orgState: Option<WindowBox>) {
	let window = window.clone();
	thread::spawn(move || {
		let mut orgState = orgState;
		loop {
			thread::sleep(Duration::from_secs(1));
			
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
			
			config::writeWindowBox(&state);
		}
	});
}
