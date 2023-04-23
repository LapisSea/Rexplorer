#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![windows_subsystem = "windows"]

use std::{env, fmt, fs, thread};
use std::collections::HashMap;
use std::fmt::Display;
use std::ops::Deref;
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex, RwLock};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, SystemTime};

use normpath::PathExt;
use slint::{Image, Model, ModelRc, PhysicalPosition, PhysicalSize, SharedString, SharedVector, Timer, TimerMode, WindowPosition, WindowSize};
use slint::platform::SetPlatformError;
use slint::private_unstable_api::re_exports::SharedVectorModel;

use config::WindowBox;

use crate::config::WindowInfo;
use crate::icon::{FileLoaderAction, GlobalIcons};
use crate::rgba_img::ImageSequence;

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

impl DirectoryReader {
	fn make(path: &str, status: &str, files: SharedVector<UIFile>, receiver: Receiver<FileLoaderAction>) -> Self {
		let mut pathIndex = HashMap::new();
		for (i, f) in files.iter().enumerate() {
			pathIndex.insert(f.fullPath.to_string(), i);
		}
		
		Self {
			directory: UIDirectoryInfo {
				files: ModelRc::new(SharedVectorModel::from(files)),
				fullPath: SharedString::from(path),
				status: SharedString::from(status),
			},
			pathIndex,
			receiver,
		}
	}
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
	let start = SystemTime::now();
	
	let globalIcon = thread::spawn(move || {
		let gi = Arc::new(RwLock::new(GlobalIcons::read()));
		icon::startIconGC(gi.clone());
		gi
	});
	
	println!("Starting GUI...");
	let app = Rc::new(HomeApp::new().expect("Failed to load UI"));
	initLogic(&app.global());
	println!("HomeApp: {:?}", start.elapsed().unwrap());
	
	let timer = Timer::default();
	windowPersistence(app.clone(), &timer);
	
	let dirReader = Rc::new(RwLock::new(None));
	
	let timer = Timer::default();
	playLoadingAnimation(&timer, app.clone(), dirReader.clone());
	
	println!("playLoadingAnimation: {:?}", start.elapsed().unwrap());
	
	let globalIcon = globalIcon.join().unwrap();
	println!("globalIcon join: {:?}", start.elapsed().unwrap());
	
	registerFileOpen(app.clone(), dirReader.clone(), globalIcon.clone());
	
	openStartingPath(&app);
	
	println!("since start: {:?}", start.elapsed().unwrap());
	
	let tApp = app.clone();
	let timer = Timer::default();
	timer.start(TimerMode::Repeated, Duration::from_secs_f32(1.0 / 15.0), move || {
		poolMediaChanges(&dirReader, &globalIcon, &tApp)
	});
	
	println!("since start: {:?}", start.elapsed().unwrap());
	let start = SystemTime::now();
	println!("Starting events...");
	app.show().unwrap();
	
	println!("{:?}", start.elapsed().unwrap());
	println!("Running loop...");
	slint::run_event_loop().unwrap();
	println!("closing...");
	app.hide().unwrap();
}

fn openStartingPath(app: &HomeApp) {
	let homePath = home::home_dir().or_else(|| {
		fs::canonicalize(".").ok().map(|p| {
			let mut f = p.as_path();
			while let Some(parent) = f.parent() { f = parent; }
			PathBuf::from(f)
		})
	}).and_then(|p| p.to_str().map(|s| s.to_string()));
	
	match homePath {
		None => {
			app.set_data(UIDirectoryInfo {
				files: Default::default(),
				fullPath: SharedString::from(""),
				status: SharedString::from("Unable to find home or default path"),
			})
		}
		Some(homePath) => {
			app.invoke_onFileOpen(SharedString::from(homePath));
		}
	}
}

fn registerFileOpen(app: Rc<HomeApp>, dirReader: Rc<RwLock<Option<DirectoryReader>>>, globalIcon: Arc<RwLock<GlobalIcons>>) {
	app.clone().on_onFileOpen(move |f| {
		let f = f.as_str();
		match fetchInfo(globalIcon.clone(), f) {
			PathInfo::Fail(d) => { println!("{}: {}", f, d); }
			PathInfo::Dir(d) => {
				setDir(&app, &dirReader, &globalIcon, d);
			}
			PathInfo::File => {
				if let Err(err) = open::that(f) { println!("{}", err); }
			}
		}
		app.window().request_redraw();
	});
}

fn poolMediaChanges(dirReader: &RwLock<Option<DirectoryReader>>, globalIcon: &RwLock<GlobalIcons>, app: &HomeApp) {
	let mut discard = false;
	
	let directory = match dirReader.try_read() {
		Ok(l) => { l }
		Err(_) => { return; }
	};
	let dReader = match directory.deref() {
		None => { return; }
		Some(s) => { s }
	};
	
	let mut model = None;
	let mut dirty = false;
	
	let mut dirtyPos = HashMap::new();
	
	let mut defaultIcon: Option<Image> = None;
	
	while let Ok(action) = dReader.receiver.try_recv() {
		// println!("{}", action);
		match action {
			FileLoaderAction::MakeUI => {
				model = Some(dReader.directory.clone());
				dirty = true;
			}
			FileLoaderAction::UpdateFile(data) => {
				let path = data.path;
				let img = data.image;
				
				if let Some(index) = dReader.pathIndex.get(&path) {
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
	drop(directory);
	
	if dirty {
		let mut model = match model {
			None => { app.get_data() }
			Some(m) => { m }
		};
		
		match dirtyPos.len() {
			0 => {}
			1..=5 => {
				for (pos, image) in dirtyPos {
					let mut file = model.files.row_data(pos).unwrap();
					
					file.icon = image.asImageCached(globalIcon, &mut defaultIcon);
					
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
					slice[pos].icon = image.asImageCached(globalIcon, &mut defaultIcon);
				}
				
				model = UIDirectoryInfo {
					files: ModelRc::new(SharedVectorModel::from(fileVec)),
					fullPath: model.fullPath,
					status: model.status,
				};
			}
		}
		
		app.set_data(model);
	}
	
	if discard {
		*dirReader.write().unwrap() = None;
		// println!("Done updating");
	}
}

fn playLoadingAnimation(timer: &Timer, app: Rc<HomeApp>, dirReader: Rc<RwLock<Option<DirectoryReader>>>) {
	let start = SystemTime::now();
	let load = ImageSequence::read("loading icon sequence.zip", "frame-", "png", 1, 30.0)
		.map_err(|err| format!("Failed to load loading icon: {err}")).unwrap();
	
	// println!("loading icon sequence {:?}", start.elapsed().unwrap());
	
	let mut loadStart = SystemTime::now();
	
	let mut lastLoading = true;
	timer.start(TimerMode::Repeated, load.timePerFrame(), move || {
		let loading = match dirReader.try_read() {
			Ok(l) => { l.deref().is_some() }
			Err(_) => { true }
		};
		if !lastLoading && loading {
			loadStart = SystemTime::now();
		}
		let update = loading || lastLoading != loading;
		lastLoading = loading;
		
		if update {
			app.set_loadIcon(if loading && loadStart.elapsed().unwrap() > Duration::from_millis(200) {
				load.getFrame(start.elapsed().unwrap()).clone()
			} else {
				Default::default()
			});
		}
	});
}

fn setDir(app: &HomeApp, dirReader: &RwLock<Option<DirectoryReader>>, globalIcon: &RwLock<GlobalIcons>, d: DirectoryReader) {
	*dirReader.write().unwrap() = Some(d);
	poolMediaChanges(dirReader, globalIcon, app)
}

fn initLogic(logic: &Logic) {
	logic.on_makeComponents(|text| {
		let comps = Path::new(text.as_str()).components();
		let cc = comps.clone().count() > 1;
		
		let mut compsVec: SharedVector<UIPathComponent> = Default::default();
		let mut soFar = PathBuf::new();
		for c in comps {
			let str = c.as_os_str().to_str().unwrap();
			soFar.push(str);
			if cc && matches!(c, Component::RootDir) {
				continue;
			}
			compsVec.push(UIPathComponent {
				fullPath: SharedString::from(format!("{}{}", soFar.to_str().unwrap(), std::path::MAIN_SEPARATOR)),
				name: SharedString::from(str),
			})
		}
		ModelRc::new(SharedVectorModel::from(compsVec))
	});
	
	logic.on_separator(|| SharedString::from(std::path::MAIN_SEPARATOR));
}

fn windowPersistence(app: Rc<HomeApp>, timer: &Timer) {
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
	
	timer.start(TimerMode::Repeated, Duration::from_millis(100), move || {
		let mut info = windowState.lock().unwrap();
		info.winBox = WindowBox::fromWindow(app.window());
	});
}

enum PathInfo {
	Fail(String),
	Dir(DirectoryReader),
	File,
}

fn normalizePath(path: &str) -> String {
	if let Ok(norm) = Path::new(path).normalize_virtually() {
		let str = norm.as_os_str().to_str().unwrap().to_string();
		if fs::metadata(&norm).map(|m| m.is_dir()).unwrap_or(false) {
			return if str.ends_with(std::path::MAIN_SEPARATOR) {
				str
			} else {
				format!("{}{}", str, std::path::MAIN_SEPARATOR)
			}
		}
		return str;
		// let l = &*norm.localize_name();
		// let l = l.to_str().unwrap();
		// let l = l.to_string();
		// return l;
	}
	
	path.to_string()
}

fn fetchInfo(state: Arc<RwLock<GlobalIcons>>, path: &str) -> PathInfo {
	let path = &normalizePath(path);
	
	for x in ["", ".", "./"] {
		if x.eq(path) {
			return PathInfo::Fail("Invalid path".to_string());
		}
	}
	
	match fs::read_dir(path) {
		Ok(rd) => {
			//Collect
			let paths: Arc<Vec<PathBuf>> =Arc::new(rd.into_iter().filter_map(|p| p.ok()).map(|p| p.path()).collect());
			
			let (send, receiver) = channel();
			send.send(FileLoaderAction::MakeUI).unwrap();
			
			icon::loadAsyncIcons(state.clone(), paths.clone(), send);
			
			let icon = { state.read().unwrap().default.clone() }.asImage();
			
			let files: SharedVector<UIFile> = paths.deref().iter().map(|path| UIFile {
				name: SharedString::from(path.file_name().and_then(|o| o.to_str())
				                             .map(|s| s.to_string()).unwrap_or("".to_string())),
				fullPath: SharedString::from(path.to_str().unwrap_or("")),
				icon: icon.clone(),
			}).collect();
			
			let status = if files.is_empty() { "This folder is empty" } else { "Ok directory" };
			
			PathInfo::Dir(DirectoryReader::make(path, status, files, receiver))
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

