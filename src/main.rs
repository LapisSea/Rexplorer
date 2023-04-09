#![allow(non_snake_case)]
#![allow(dead_code)]

use std::{env, fmt, fs, thread};
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry;
use std::fmt::Display;
use std::ops::Deref;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::sleep;
use std::time::{Duration, SystemTime};

use rand::Rng;
use slint::{Image, Model, ModelRc, PhysicalPosition, PhysicalSize, SharedString, SharedVector, Timer, TimerMode, WindowPosition, WindowSize};
use slint::private_unstable_api::re_exports::SharedVectorModel;

use config::WindowBox;

use crate::config::WindowInfo;
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
	loader: Arc<UIDirectoryInfoLoader>,
	receiver: Receiver<FileLoaderAction>,
}

struct State {
	iconCache: HashMap<String, LoadStage<Arc<RgbImg>>>,
	folderIco: Arc<RgbImg>,
	defaultIco: Arc<RgbImg>,
	loaderId: u32,
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
	windowPersistence(app.clone(),&timer);
	
	let state = Arc::new(Mutex::new(State {
		iconCache: HashMap::new(),
		folderIco: Arc::new(RgbImg::read(">>win-folder.png").unwrap()),
		defaultIco: Arc::new(RgbImg::read(">>default.png").unwrap()),
		loaderId: 1,
	}));
	gcIcons(state.clone());
	
	
	let dirReader = Rc::new(Mutex::new(None));
	
	{
		let tmpApp = app.clone();
		let app = app.clone();
		let state = state.clone();
		let dirReader = dirReader.clone();
		tmpApp.on_onFileOpen(move |f| {
			let f = f.as_str();
			match fetchInfo(state.clone(), f) {
				PathInfo::Fail(d) => {
					let d = d.loader;
					
					println!("{}: {}", d.fullPath.as_str(), d.status.as_str())
				}
				PathInfo::Dir(d) => {
					let mut model = app.get_data();
					model.fullPath = SharedString::from(&d.loader.fullPath);
					app.set_data(model);
					
					*dirReader.lock().unwrap() = Some(d);
				}
				PathInfo::File => {
					if let Err(err) = open::that(f) {
						println!("{}", err);
					}
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
	match fetchInfo(state.clone(), "C:\\Users\\LapisSea\\Desktop") {
		PathInfo::Fail(d) => { *dirReader.lock().unwrap() = Some(d); }
		PathInfo::Dir(d) => {
			let mut model = app.get_data();
			model.fullPath = SharedString::from(&d.loader.fullPath);
			app.set_data(model);
			
			*dirReader.lock().unwrap() = Some(d);
		}
		PathInfo::File => {}
	}
	
	let defaultIcon = { state.lock().unwrap().defaultIco.asImage() };
	let defaultIcon = Rc::new(defaultIcon);
	
	let tApp = app.clone();
	let timer = Timer::default();
	timer.start(TimerMode::Repeated, Duration::from_millis(30), move || {
		let mut discard = false;
		{
			let directory = dirReader.lock().unwrap();
			if let Some(tup) = directory.deref() {
				let dir = &tup.loader;
				let rec = &tup.receiver;
				
				let mut model = tApp.get_data();
				let mut dirty = false;
				
				let mut dirtyPos = HashSet::new();
				
				while let Ok(action) = rec.try_recv() {
					// println!("{}", action);
					match action {
						FileLoaderAction::MakeUI => {
							model = dir.makeUI();
							dirtyPos.reserve(model.files.row_count());
							(0..model.files.row_count()).for_each(|i| { dirtyPos.insert(i); });
							dirty = true;
						}
						FileLoaderAction::UpdateFile(idx) => {
							dirty = true;
							dirtyPos.insert(idx);
							// if dirtyPos.len() >= 50 { break; }
						}
						FileLoaderAction::End => {
							discard = true;
						}
					}
				}
				
				if dirty {
					fn makeImg(default: &Rc<Image>, file: &mut UIFile, fileLoader: &UIFileLoader) {
						let rgb = fileLoader.icon.lock().unwrap();
						
						file.icon = if rgb.isDefault() {
							default.deref().clone()
						} else {
							rgb.asImage()
						}
					}
					
					match dirtyPos.len() {
						0 => {}
						1..=5 => {
							for pos in dirtyPos {
								let mut file = model.files.row_data(pos).unwrap();
								
								let fileLoader = dir.files.get(pos).unwrap();
								
								makeImg(&defaultIcon, &mut file, fileLoader);
								
								model.files.set_row_data(pos, file);
							}
						}
						_ => {
							let mut fileVec: SharedVector<UIFile> = Default::default();
							for x in model.files.iter() {
								fileVec.push(x);
							}
							
							let slice = fileVec.make_mut_slice();
							for idx in dirtyPos {
								let fileLoader = dir.files.get(idx).unwrap();
								makeImg(&defaultIcon, &mut slice[idx], fileLoader);
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

fn gcIcons(state: Arc<Mutex<State>>) {
	thread::spawn(move || {
		let mut rng = rand::thread_rng();
		loop {
			sleep(Duration::from_millis(500));
			
			let mut state = state.lock().unwrap();
			let cache = &mut state.iconCache;
			
			let now = SystemTime::now();
			
			cache.retain(|k, val| {
				match val {
					LoadStage::Loading => {}
					LoadStage::Loaded(t, _) => {
						let age = now.duration_since(*t).unwrap().as_secs_f64();
						
						let minAge = 5.0;
						let maxAge = 120.0;
						if age > minAge {
							let fac = 1.0_f64.min((age - minAge) / (maxAge - minAge));
							let fac = fac.powi(4);
							if rng.gen_bool(fac) {
								// println!("Yeet {k} \t {age} with probability of {fac}");
								return false;
							}
						}
					}
				}
				true
			});
		}
	});
}

enum FileLoaderAction {
	MakeUI,
	UpdateFile(usize),
	End,
}

struct UIFileLoader {
	name: String,
	fullPath: String,
	icon: Arc<Mutex<Arc<RgbImg>>>,
}

impl UIFileLoader {
	fn makeUI(&self) -> UIFile {
		UIFile {
			fullPath: SharedString::from(&self.fullPath),
			name: SharedString::from(&self.name),
			icon: Default::default(),
		}
	}
}

struct UIDirectoryInfoLoader {
	loaderId: u32,
	files: Arc<Vec<UIFileLoader>>,
	fullPath: String,
	status: String,
}

impl UIDirectoryInfoLoader {
	fn makeUI(&self) -> UIDirectoryInfo {
		let mut data: SharedVector<UIFile> = Default::default();
		
		{
			for f in self.files.iter() {
				data.push(f.makeUI());
			}
		}
		
		UIDirectoryInfo {
			files: ModelRc::new(SharedVectorModel::from(data)),
			fullPath: SharedString::from(&self.fullPath),
			status: SharedString::from(&self.status),
		}
	}
}

enum PathInfo {
	Fail(DirectoryReader),
	Dir(DirectoryReader),
	File,
}

impl Display for FileLoaderAction {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			FileLoaderAction::MakeUI => {
				f.write_str("MakeUI")?;
			}
			FileLoaderAction::UpdateFile(idx) => {
				f.write_str("UpdateFile{@")?;
				f.write_str(&idx.to_string())?;
				f.write_str("}")?;
			}
			FileLoaderAction::End => {
				f.write_str("END")?;
			}
		}
		Ok(())
	}
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
	
	icon.map(Arc::new)
	    .unwrap_or_else(|| state.lock().unwrap().defaultIco.clone())
}

fn checkID(state: Arc<Mutex<State>>, dest: Arc<UIDirectoryInfoLoader>, send: Sender<FileLoaderAction>) -> bool {
	let state = state.lock().unwrap();
	if dest.loaderId != state.loaderId {
		let _ = send.send(FileLoaderAction::End);
		return true;
	}
	false
}

fn resolveInfoAsync(state: Arc<Mutex<State>>, paths: Vec<PathBuf>, dest: Arc<UIDirectoryInfoLoader>, send: Sender<FileLoaderAction>) {
	thread::spawn(move || {
		if checkID(state.clone(), dest.clone(), send.clone()) { return; }
		
		let mut loadingIdx = vec![];
		
		//Spawn tasks
		for (i, path) in paths.clone().into_iter().enumerate() {
			let state = state.clone();
			{
				let mut state = state.lock().unwrap();
				let strPath = path.to_str().unwrap();
				match state.iconCache.entry(strPath.to_string()) {
					Entry::Occupied(mut e) => {
						if let LoadStage::Loaded(t, loaded) = e.get_mut() {
							*t = SystemTime::now();
							*dest.files[i].icon.lock().unwrap() = loaded.clone();
							continue;
						}
					}
					Entry::Vacant(e) => {
						e.insert(LoadStage::Loading);
					}
				};
			}
			loadingIdx.push(i);
			
			let dest = dest.clone();
			if checkID(state.clone(), dest.clone(), send.clone()) { return; }
			
			let send = send.clone();
			work::execute(move || {
				if checkID(state.clone(), dest.clone(), send.clone()) { return; }
				let strPath = path.to_str().unwrap();
				// println!("Async loading {}", strPath);
				let img = loadFromPath(state.clone(), path.clone());
				if checkID(state.clone(), dest.clone(), send.clone()) { return; }
				{
					let state = state.clone();
					let mut state = state.lock().unwrap();
					state.iconCache.insert(strPath.to_string(), LoadStage::Loaded(SystemTime::now(), img.clone()));
				}
				// println!("Async loaded {}", strPath);
				
				if !img.isDefault() {
					if checkID(state.clone(), dest.clone(), send.clone()) { return; }
					
					let file = dest.files.get(i).unwrap();
					*file.icon.lock().unwrap() = img;
					let _ = send.send(FileLoaderAction::UpdateFile(i));
				}
			});
		}
		
		//Collect results
		for i in loadingIdx {
			let path = &paths[i];
			let icon;
			let strPath = path.to_str().unwrap();
			loop {
				// if checkID(state.clone(), dest.clone()) { return; }
				
				let state = state.lock().unwrap();
				match state.iconCache.get(strPath).unwrap() {
					LoadStage::Loading => {
						drop(state);
						sleep(Duration::from_millis(50));
						continue;
					}
					LoadStage::Loaded(t, r) => {
						icon = r.clone();
						// println!("Loaded {}", strPath);
						break;
					}
				};
			}
			
			if !icon.isDefault() {
				if checkID(state.clone(), dest.clone(), send.clone()) { return; }
				
				let file = dest.files.get(i).unwrap();
				*file.icon.lock().unwrap() = icon;
				let _ = send.send(FileLoaderAction::UpdateFile(i));
			}
		}
		let _ = send.send(FileLoaderAction::End);
	});
}

fn fetchInfo(state: Arc<Mutex<State>>, path: &str) -> PathInfo {
	for x in ["", ".", "./"] {
		if x.eq(path) {
			return failPathInfo(path, "Invalid path".to_string());
		}
	}
	
	match fs::read_dir(path) {
		Ok(rd) => {
			let loaderId;
			{
				let mut state = state.lock().unwrap();
				state.loaderId += 1;
				loaderId = state.loaderId;
			}
			
			//Collect
			let paths: Vec<PathBuf> = rd.into_iter().filter_map(|p| p.ok()).map(|p| p.path()).collect();
			
			let mut files = vec![];
			files.reserve_exact(paths.len());
			
			let icon = {
				let state = state.lock().unwrap();
				state.defaultIco.clone()
			};
			
			let (send, receive) = channel();
			send.send(FileLoaderAction::MakeUI).unwrap();
			
			//Initial populate
			for path in paths.clone() {
				files.push(UIFileLoader {
					name: path.file_name().and_then(|o| o.to_str())
					          .map(|s| s.to_string()).unwrap_or("".to_string()),
					fullPath: path.to_str().unwrap_or("").to_string(),
					icon: Arc::new(Mutex::new(icon.clone())),
				});
			}
			
			let res = Arc::new(UIDirectoryInfoLoader {
				loaderId,
				files: Arc::new(files),
				fullPath: path.to_string(),
				status: "Loading... please wait".to_string(),
			});
			resolveInfoAsync(state, paths, res.clone(), send);
			PathInfo::Dir(DirectoryReader {
				loader: res,
				receiver: receive,
			})
		}
		Err(err) => {
			if let Ok(meta) = fs::metadata(path) {
				if meta.is_file() {
					return PathInfo::File;
				}
			}
			
			failPathInfo(path, format!("{}", err))
		}
	}
}

fn failPathInfo(path: &str, err: String) -> PathInfo {
	let path = path.to_string();
	let (send, receive) = channel();
	send.send(FileLoaderAction::MakeUI).unwrap();
	
	PathInfo::Fail(DirectoryReader {
		loader: Arc::new(UIDirectoryInfoLoader {
			loaderId: 0,
			files: Default::default(),
			fullPath: path,
			status: err,
		}),
		receiver: receive,
	})
}
