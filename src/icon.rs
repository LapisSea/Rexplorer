use std::{fmt, fs, thread};
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter, Write};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::sync::mpsc::Sender;
use std::thread::sleep;
use std::time::{Duration, SystemTime};

use rand::Rng;

use crate::rgba_img::RgbImg;
use crate::work;

type WorkID = u32;

enum LoadStage<T> {
	Loading,
	Loaded(SystemTime, T),
}

pub struct LoadedIcon {
	pub image: Arc<RgbImg>,
	pub path: String,
}

pub struct GlobalIcons {
	pub default: Arc<RgbImg>,
	pub folder: Arc<RgbImg>,
	
	iconCache: HashMap<String, LoadStage<Arc<RgbImg>>>,
	workerId: WorkID,
}

impl GlobalIcons {
	pub fn read() -> Self {
		let folder = thread::spawn(|| Arc::new(RgbImg::read(">>win-folder.png").unwrap()));
		let default = thread::spawn(|| Arc::new(RgbImg::read(">>default.png").unwrap()));
		Self {
			folder: folder.join().unwrap(),
			default: default.join().unwrap(),
			iconCache: Default::default(),
			workerId: Default::default(),
		}
	}
}

pub enum FileLoaderAction {
	MakeUI,
	UpdateFile(LoadedIcon),
	End,
}

impl Display for FileLoaderAction {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			FileLoaderAction::MakeUI => {
				f.write_str("MakeUI")?;
			}
			FileLoaderAction::UpdateFile(idx) => {
				f.write_str("UpdateFile{")?;
				f.write_str(&idx.path)?;
				f.write_str("}")?;
			}
			FileLoaderAction::End => {
				f.write_str("END")?;
			}
		}
		Ok(())
	}
}

impl GlobalIcons {
	fn newId(&mut self) -> WorkID {
		self.workerId += 1;
		self.workerId
	}
}

pub fn loadAsyncIcons(global: Arc<RwLock<GlobalIcons>>, paths: Vec<PathBuf>, send: Sender<FileLoaderAction>) {
	let workId = { global.write().unwrap().newId() };
	
	thread::spawn(move || {
		if checkID(global.clone(), workId, send.clone()) { return; }
		
		let mut toScan = HashSet::new();
		let mut toSpawn = vec![];
		
		for x in paths.iter() {
			let pathStr = x.to_str().unwrap();
			let mut global = global.write().unwrap();
			
			if let Some(stage) = global.iconCache.get_mut(pathStr) {
				let pathStr = pathStr.to_string();
				if let LoadStage::Loading = stage { toScan.insert(pathStr.clone()); }
				sendStage(&send, pathStr, stage);
				continue;
			}
			let pathStr = pathStr.to_string();
			toSpawn.push(x.clone());
			global.iconCache.insert(pathStr.clone(), LoadStage::Loading);
			toScan.insert(pathStr);
		}
		
		if checkID(global.clone(), workId, send.clone()) { return; }
		
		for path in toSpawn {
			let send = send.clone();
			let global = global.clone();
			work::execute(move || {
				let pathStr = path.to_str().unwrap().to_string();
				
				if checkID(global.clone(), workId, send.clone()) {
					let mut state = global.write().unwrap();
					state.iconCache.remove(&pathStr);
					return;
				}
				
				let img = loadFromPath(global.clone(), path.clone());
				{
					let state = global.clone();
					let mut state = state.write().unwrap();
					state.iconCache.insert(pathStr.clone(), LoadStage::Loaded(SystemTime::now(), img.clone()));
				}
				
				// println!("Async loaded {}", pathStr);
				
				if !img.isDefault() {
					if checkID(global.clone(), workId, send.clone()) { return; }
					let _ = send.send(FileLoaderAction::UpdateFile(LoadedIcon {
						image: img,
						path: pathStr,
					}));
				}
			});
		}
		
		while !toScan.is_empty() {
			sleep(Duration::from_millis(2));
			
			toScan.retain(|f| {
				match global.clone().write().unwrap().iconCache.get_mut(f) {
					None => {}
					Some(stage) => {
						sendStage(&send, f.clone(), stage);
						if let LoadStage::Loaded(_, _) = stage {
							return false;
						}
					}
				}
				true
			});
		}
		let _ = send.send(FileLoaderAction::End);
	});
}

fn sendStage(send: &Sender<FileLoaderAction>, pathStr: String, stage: &mut LoadStage<Arc<RgbImg>>) {
	match stage {
		LoadStage::Loading => {}
		LoadStage::Loaded(t, img) => {
			*t = SystemTime::now();
			let _ = send.send(FileLoaderAction::UpdateFile(LoadedIcon {
				image: img.clone(),
				path: pathStr,
			}));
		}
	}
}

fn loadFromPath(state: Arc<RwLock<GlobalIcons>>, path: PathBuf) -> Arc<RgbImg> {
	match fs::metadata(path.clone()).ok() {
		None => {
			return state.read().unwrap().default.clone();
		}
		Some(meta) => {
			if meta.is_dir() {
				return state.read().unwrap().folder.clone();
			}
		}
	}
	
	let mut icon = None;
	if path.extension().and_then(|s| s.to_str()).filter(|s| ["jpg", "png"].contains(s)).is_some() {
		icon = RgbImg::read(path.as_os_str().to_str().unwrap()).ok();
	}
	
	icon.map(Arc::new)
	    .unwrap_or_else(|| state.read().unwrap().default.clone())
}

fn checkID(global: Arc<RwLock<GlobalIcons>>, currentId: WorkID, send: Sender<FileLoaderAction>) -> bool {
	let workerId = { global.read().unwrap().workerId };
	
	if workerId != currentId {
		let _ = send.send(FileLoaderAction::End);
		return true;
	}
	false
}


pub fn startIconGC(state: Arc<RwLock<GlobalIcons>>) {
	thread::spawn(move || {
		let mut rng = rand::thread_rng();
		loop {
			sleep(Duration::from_millis(500));
			
			let mut state = state.write().unwrap();
			let cache = &mut state.iconCache;
			
			let now = SystemTime::now();
			
			cache.retain(|_, val| {
				match val {
					LoadStage::Loading => {}
					LoadStage::Loaded(t, _ico) => {
						let age = now.duration_since(*t).unwrap().as_secs_f64();
						
						let minAge = 5.0;
						let maxAge = 120.0;
						if age > minAge {
							let fac = 1.0_f64.min((age - minAge) / (maxAge - minAge));
							let fac = fac.powi(4);
							if rng.gen_bool(fac) {
								// if !_ico.isDefault() { println!("Yeet {k} \t {age} with probability of {fac}"); }
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
