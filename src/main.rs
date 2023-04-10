#![allow(non_snake_case)]
#![allow(dead_code)]

use std::{env, fmt, fs, thread};
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::ops::Deref;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex, RwLock};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::sleep;
use std::time::{Duration, SystemTime};

use rand::Rng;
use slint::{Image, Model, ModelRc, PhysicalPosition, PhysicalSize, SharedString, SharedVector, Timer, TimerMode, WindowPosition, WindowSize};
use slint::private_unstable_api::re_exports::SharedVectorModel;

use config::WindowBox;

use crate::config::WindowInfo;
use crate::icon::{FileLoaderAction, GlobalIcons};
use crate::rgba_img::RgbImg;

mod config;
mod rgba_img;
mod work;
mod icon;

slint::include_modules!();

enum LoadStage<T> {
	Loading,
	Loaded(SystemTime, T),
}

struct DirectoryReader {
	directory: UIDirectoryInfo,
	pathIndex: HashMap<String, usize>,
	receiver: Receiver<FileLoaderAction>,
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
	
	let timer = Timer::default();
	windowPersistence(app.clone(), &timer);
	
	let globalIcon = Arc::new(RwLock::new(GlobalIcons::read()));
	icon::startIconGC(globalIcon.clone());
	
	let dirReader = Rc::new(Mutex::new(None));
	
	{
		let tmpApp = app.clone();
		let app = app.clone();
		let globalIcon = globalIcon.clone();
		let dirReader = dirReader.clone();
		
		tmpApp.on_onFileOpen(move |f| {
			let f = f.as_str();
			match fetchInfo(globalIcon.clone(), f) {
				PathInfo::Fail(d) => { println!("{}: {}", f, d); }
				PathInfo::Dir(d) => {
					app.set_data(d.directory.clone());
					*dirReader.lock().unwrap() = Some(d);
				}
				PathInfo::File => {
					if let Err(err) = open::that(f) { println!("{}", err); }
				}
			}
			app.window().request_redraw();
		});
	}
	
	
	// if let Some(path) = home::home_dir() {
	// 	let mut iconCache = HashMap::new();
	// 	match fetchInfo(&mut iconCache, format!("{}", path.display()).as_str()) {
	// 		PathInfo::Fail(d) => { app.set_data(d) }
	// 		PathInfo::Dir(d) => { app.set_data(d) }
	// 		PathInfo::File => {}
	// 	}
	// }
	let homePath = "C:\\Users\\LapisSea\\Desktop";
	match fetchInfo(globalIcon.clone(), homePath) {
		PathInfo::Fail(d) => {
			app.set_data(UIDirectoryInfo {
				files: Default::default(),
				fullPath: SharedString::from(homePath),
				status: SharedString::from(d),
			})
		}
		PathInfo::Dir(d) => {
			app.set_data(d.directory.clone());
			*dirReader.lock().unwrap() = Some(d);
		}
		PathInfo::File => {}
	}
	
	let defaultIcon = Rc::new(globalIcon.read().unwrap().default.asImage());
	
	let tApp = app.clone();
	let timer = Timer::default();
	timer.start(TimerMode::Repeated, Duration::from_millis(30), move || {
		let mut discard = false;
		{
			let directory = dirReader.lock().unwrap();
			if let Some(tup) = directory.deref() {
				let dir = &tup.directory;
				let rec = &tup.receiver;
				let pathIndex = &tup.pathIndex;
				
				let mut model = tApp.get_data();
				let mut dirty = false;
				
				let mut dirtyPos = HashMap::new();
				
				while let Ok(action) = rec.try_recv() {
					// println!("{}", action);
					match action {
						FileLoaderAction::MakeUI => {
							model = dir.clone();
							dirty = true;
						}
						FileLoaderAction::UpdateFile(data) => {
							let path = data.path;
							let img = data.image;
							
							if let Some(index) = pathIndex.get(&path) {
								dirty = true;
								dirtyPos.insert(*index, img);
								// if dirtyPos.len() >= 50 { break; }
							}
						}
						FileLoaderAction::End => {
							discard = true;
						}
					}
				}
				
				if dirty {
					fn makeImg(default: &Rc<Image>, file: &mut UIFile, rgb: Arc<RgbImg>) {
						file.icon = if rgb.isDefault() {
							default.deref().clone()
						} else {
							rgb.asImage()
						}
					}
					
					match dirtyPos.len() {
						0 => {}
						1..=5 => {
							for (pos, image) in dirtyPos {
								let mut file = model.files.row_data(pos).unwrap();
								
								makeImg(&defaultIcon, &mut file, image);
								
								model.files.set_row_data(pos, file);
							}
						}
						_ => {
							let mut fileVec: SharedVector<UIFile> = Default::default();
							for x in model.files.iter() {
								fileVec.push(x);
							}
							
							let slice = fileVec.make_mut_slice();
							for (pos, image) in dirtyPos {
								makeImg(&defaultIcon, &mut slice[pos], image);
							}
							
							model = UIDirectoryInfo {
								files: ModelRc::new(SharedVectorModel::from(fileVec)),
								fullPath: model.fullPath,
								status: model.status,
							};
						}
					}
					
					tApp.set_data(model);
					tApp.window().request_redraw();
				}
			}
		}
		if discard {
			*dirReader.lock().unwrap() = None;
			println!("Done updating.");
		}
	});
	
	app.run().unwrap();
}

fn windowPersistence(app: Rc<HomeApp>, timer: &Timer){
	let windowState;
	{
		let org = config::readWindowBox();
		let s = org.unwrap_or_else(|| WindowBox::new(100, 100, 800, 600));
		
		let win = app.window();
		win.set_size(WindowSize::Physical(PhysicalSize::new(s.width, s.height)));
		win.set_position(WindowPosition::Physical(PhysicalPosition::new(s.x, s.y)));
		
		windowState = Arc::new(Mutex::new(WindowInfo::new(s)));
		config::watchState(windowState.clone(), org);
	}
	
	let tApp = app.clone();
	timer.start(TimerMode::Repeated, Duration::from_secs(1), move || {
		let mut info = windowState.lock().unwrap();
		info.winBox = WindowBox::fromWindow(tApp.window());
	});
}

enum PathInfo {
	Fail(String),
	Dir(DirectoryReader),
	File,
}

fn fetchInfo(state: Arc<RwLock<GlobalIcons>>, path: &str) -> PathInfo {
	for x in ["", ".", "./"] {
		if x.eq(path) {
			return PathInfo::Fail("Invalid path".to_string());
		}
	}
	
	match fs::read_dir(path) {
		Ok(rd) => {
			let start = SystemTime::now();
			
			//Collect
			let paths: Vec<PathBuf> = rd.into_iter().filter_map(|p| p.ok()).map(|p| p.path()).collect();
			
			let (send, receive) = channel();
			send.send(FileLoaderAction::MakeUI).unwrap();
			
			icon::loadAsyncIcons(state.clone(), paths.clone(), send);
			
			let icon = { state.read().unwrap().default.clone() }.asImage();
			
			//Initial populate
			let fileVec: SharedVector<UIFile> = paths.into_iter().map(|path| UIFile {
				name: SharedString::from(path.file_name().and_then(|o| o.to_str())
				                             .map(|s| s.to_string()).unwrap_or("".to_string())),
				fullPath: SharedString::from(path.to_str().unwrap_or("")),
				icon: icon.clone(),
			}).collect();
			
			let mut pathIndex = HashMap::new();
			for (i, f) in fileVec.iter().enumerate() {
				pathIndex.insert(f.fullPath.to_string(), i);
			}
			let directory = UIDirectoryInfo {
				files: ModelRc::new(SharedVectorModel::from(fileVec)),
				fullPath: SharedString::from(path),
				status: SharedString::from("Loading... please wait"),
			};
			
			let end = SystemTime::now();
			
			println!("read_dir blocking time: {:?}", end.duration_since(start).unwrap());
			
			PathInfo::Dir(DirectoryReader {
				directory,
				pathIndex,
				receiver: receive,
			})
		}
		Err(err) => {
			if let Ok(meta) = fs::metadata(path) {
				if meta.is_file() {
					return PathInfo::File;
				}
			}
			
			PathInfo::Fail(format!("{}", err))
		}
	}
}

