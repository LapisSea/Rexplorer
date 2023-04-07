#![allow(non_snake_case)]
#![allow(unused_imports)]
#![allow(dead_code)]

use std::{default, env, fmt, fs, io, thread};
use std::cell::Cell;
use std::cmp::{max, min};
use std::collections::HashMap;
use std::error::Error;
use std::fmt::Display;
use std::fs::File;
use std::io::{BufReader, ErrorKind};
use std::io::Cursor;
use std::ops::{BitAnd, Deref};
use std::path::PathBuf;
use std::ptr::eq;
use std::rc::Rc;
use std::sync::{Arc, Mutex, RwLock};
use std::thread::Thread;
use std::time::Duration;

use image::{ColorType, ImageFormat, load, Pixel, Rgba};
use image::imageops::FilterType;
use image::io::Reader as ImageReader;
use platform::Platform;
use slint;
use slint::{Image, LogicalPosition, LogicalSize, Model, ModelRc, PhysicalPosition, PhysicalSize, platform, Rgb8Pixel, Rgba8Pixel, SharedPixelBuffer, SharedString, SharedVector, Window, WindowPosition, WindowSize};
use slint::private_unstable_api::re_exports::SharedVectorModel;
use slint::private_unstable_api::re_exports::StandardButtonKind::Retry;

use config::WindowBox;

mod config;
mod ui;

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
	let s = config::readWindowBox().unwrap_or_else(|| WindowBox::new(100, 100, 800, 600));
	let app = Rc::new(HomeApp::new().unwrap());
	let win = app.window();
	{
		let eApp = app.clone();
		app.on_onFileOpen(move |f| {
			let f = f.as_str();
			let mut iconCache = HashMap::new();
			match fetchInfo(&mut iconCache, f) {
				PathInfo::Fail(_) => {}
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
	
	if let Some(path) = home::home_dir() {
		let mut iconCache = HashMap::new();
		match fetchInfo(&mut iconCache, format!("{}", path.display()).as_str()) {
			PathInfo::Fail(d) => { app.set_data(d) }
			PathInfo::Dir(d) => { app.set_data(d) }
			PathInfo::File => {}
		}
	}
	
	
	win.set_size(WindowSize::Physical(PhysicalSize::new(s.width, s.height)));
	win.set_position(WindowPosition::Physical(PhysicalPosition::new(s.x, s.y)));
	
	app.run().unwrap();
}

fn loadIcon(iconCache: &mut HashMap<String, Image>, path: &str) -> Result<Image, String> {
	{
		let cached = iconCache.get(path);
		match cached {
			None => {}
			Some(i) => { return Ok(i.clone()); }
		}
	}
	
	let maxItems = 1024 * 1024 * 8 / (128 * 128 * 4);
	
	if iconCache.len() >= maxItems {
		let key = iconCache.keys().next().cloned().unwrap();
		iconCache.remove(&key);
	}
	
	let loaded = loadIconFs(path);
	
	match loaded {
		Ok(loaded) => {
			iconCache.insert(path.to_string(), loaded.clone());
			Ok(loaded)
		}
		Err(err) => { Err(err) }
	}
}

fn loadIconFs(path: &str) -> Result<Image, String> {
	println!("Loading: {:?}", path);
	
	let mut img =
		ImageReader::open(path)
			.and_then(|i| i.with_guessed_format())
			.map_err(|err| format!("{}", err))
			.and_then(|i| i.decode().map_err(|err| format!("{}", err)))?;
	
	let s = max(img.width(), img.height());
	let maxSiz = 256;
	
	if s > maxSiz {
		let fac = maxSiz as f32 / s as f32;
		img = img.resize(
			(img.width() as f32 * fac) as u32,
			(img.height() as f32 * fac) as u32,
			FilterType::CatmullRom,
		);
	}
	
	let mut buffer = SharedPixelBuffer::new(img.width(), img.height());
	let w = img.width();
	let h = img.height();
	let s: &mut [Rgba8Pixel] = buffer.make_mut_slice();
	for x in 0..w {
		for y in 0..h {
			let p: Rgba<u8> = match img.color() {
				ColorType::L8 => { img.as_luma8().unwrap().get_pixel(x, y).to_rgba() }
				ColorType::La8 => { img.as_luma_alpha8().unwrap().get_pixel(x, y).to_rgba() },
				ColorType::Rgb8 => { img.as_rgb8().unwrap().get_pixel(x, y).to_rgba() },
				ColorType::Rgba8 => { *img.as_rgba8().unwrap().get_pixel(x, y) },
				ColorType::L16 => { p16to8(img.as_luma16().unwrap().get_pixel(x, y).to_rgba()) },
				ColorType::La16 => { p16to8(img.as_luma_alpha16().unwrap().get_pixel(x, y).to_rgba()) },
				ColorType::Rgb16 => { p16to8(img.as_rgb16().unwrap().get_pixel(x, y).to_rgba()) },
				ColorType::Rgba16 => { p16to8(img.as_rgba16().unwrap().get_pixel(x, y).to_rgba()) },
				ColorType::Rgb32F => { pf32to8(img.as_rgb32f().unwrap().get_pixel(x, y).to_rgba()) },
				ColorType::Rgba32F => { pf32to8(img.as_rgba32f().unwrap().get_pixel(x, y).to_rgba()) },
				_ => todo!("Unimplemented format {:?}", img.color())
			};
			s[(x + y * w) as usize] = Rgba8Pixel::from([p[0], p[1], p[2], p[3]]);
		}
	}
	
	Ok(Image::from_rgba8(buffer))
}

fn p16to8(p: Rgba<u16>) -> Rgba<u8> {
	Rgba::from([p[0] as u8, p[1] as u8, p[2] as u8, p[3] as u8])
}

fn pf32to8(p: Rgba<f32>) -> Rgba<u8> {
	Rgba::from([p[0] as u8, p[1] as u8, p[2] as u8, p[3] as u8])
}

fn pathToUIFile(iconCache: &mut HashMap<String, Image>, path: PathBuf) -> Option<UIFile> {
	let meta = fs::metadata(path.clone()).ok()?;
	
	let icon = match meta.is_dir() {
		true => loadIcon(iconCache, "./../src/ui/win-folder.png")
			.map_err(|err| println!("Failed to load image: {}", err))
			.unwrap(),
		false => {
			let mut icon = None;
			if path.extension().and_then(|s| s.to_str()).filter(|s| ["jpg", "png"].contains(s)).is_some() {
				icon = loadIcon(iconCache, path.as_os_str().to_str().unwrap())
					.map_err(|err| println!("Failed to load image: {}", err)).ok()
			}
			if icon.is_none() {
				loadIcon(iconCache, "./../src/ui/default.png")
					.map_err(|err| println!("Failed to load image: {}", err))
					.unwrap()
			} else {
				icon.unwrap()
			}
		}
	};
	
	Some(UIFile {
		fullPath: SharedString::from(path.to_str().unwrap()),
		icon,
		name: SharedString::from(path.file_name().unwrap().to_str().unwrap()),
	})
}

enum PathInfo {
	Fail(UIDirectoryInfo),
	Dir(UIDirectoryInfo),
	File,
}

fn fetchInfo(iconCache: &mut HashMap<String, Image>, path: &str) -> PathInfo {
	return match fs::read_dir(path) {
		Ok(rd) => {
			let mut data: SharedVector<UIFile> = Default::default();
			
			for path in rd {
				// println!("{:?}", path);
				let path = path.ok().map(|p| p.path());
				if let Some(path) = path {
					if let Some(uip) = pathToUIFile(iconCache, path) {
						data.push(uip);
					}
				}
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
