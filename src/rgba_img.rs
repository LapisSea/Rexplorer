use std::{fmt, thread};
use std::cmp::{max, min};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::io::{BufRead, Cursor, Read, Seek};
use std::ops::Deref;
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use std::sync::mpsc::channel;
use std::thread::scope;
use std::time::Duration;

use image::{ColorType, DynamicImage, Pixel, Rgba};
use image::imageops::FilterType;
use image::io::Reader as ImageReader;
use rust_embed::RustEmbed;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use zip::ZipArchive;
use crate::icon::GlobalIcons;

#[derive(RustEmbed)]
#[folder = "src/ui/"]
#[include = "*.png"]
#[include = "*.jpg"]
#[include = "*.zip"]
struct BuiltInAssets;

pub struct ImageSequence {
	frames: Vec<Image>,
	timePerFrame: Duration,
}

fn readFile(namePrefix: &str, extension: &str, zip: &mut ZipArchive<Cursor<&[u8]>>, i: usize) -> Result<(i32, RgbImg), String> {
	let mut file = zip.by_index(i).map_err(|e| e.to_string())?;
	let name = file.name();
	let num = match name.strip_prefix(namePrefix).and_then(|x| x.strip_suffix(&extension)) {
		None => { return Err(format!("Invalid name {name}, All file names must conform to: {namePrefix}<integer>{extension}")); }
		Some(s) => { s }
	};
	let num = match num.parse::<i32>() {
		Ok(n) => { n }
		Err(err) => {
			return Err(format!("Invalid frame number \"{err}\" for {name}"));
		}
	};
	
	let mut fileBytes = vec![0_u8; file.size() as usize];
	let mut pos = 0;
	
	let mut buf = [0_u8; 1024];
	loop {
		let read = file.read(&mut buf).unwrap();
		if read == 0 { break; }
		fileBytes[pos..(read + pos)].copy_from_slice(&buf[..read]);
		pos += read;
	}
	
	let image = imgFrom(Cursor::new(fileBytes))?;
	Ok((num, convert(image, "")))
}

impl ImageSequence {
	pub fn read(path: &str, namePrefix: &str, extension: &str, indexStart: i32, framerate: f32) -> Result<Self, String> {
		let extension = format!(".{}", extension);
		
		let (send, receive) = channel();
		
		let d = match BuiltInAssets::get(path) {
			None => { return Err(format!("{path} not embedded")); }
			Some(d) => { d }
		};
		
		let zip = ZipArchive::new(Cursor::new(d.data.deref()))
			.map_err(|err| format!("Failed to open {path}: {err}"))?;
		
		let count = zip.len();
		if count == 0 { return Err(format!("Empty zip: {path}")); }
		
		scope(|scope| {
			let cores = thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
			let usedCores = min(cores, count);
			let chunkSize = ceilDiv(count, usedCores);
			
			let mut tasks = vec![];
			
			for i in 0..usedCores {
				let start = i * chunkSize;
				let end = min((i + 1) * chunkSize, count);
				
				let pref = namePrefix.to_string();
				let send = send.clone();
				let mut zip = zip.clone();
				let extension = extension.clone();
				
				tasks.push(scope.spawn(move || {
					for i in start..end {
						let res = readFile(&pref, &extension, &mut zip, i).and_then(|(num, image)| {
							if num < indexStart {
								return Err(format!("{num} < {indexStart}"));
							}
							let index = (num - indexStart) as usize;
							Ok((index, image))
						});
						match res {
							Ok(ok) => { let _ = send.send(ok); }
							Err(err) => { return Some(err); }
						}
					}
					None
				}));
			}
			
			for x in tasks {
				match x.join() {
					Ok(ok) => {
						if let Some(err) = ok {
							return Err(err);
						}
					}
					Err(_) => {
						return Err("??".to_string());
					}
				}
			}
			
			let mut acum: HashMap<usize, RgbImg> = HashMap::new();
			let mut frames = vec![];
			frames.reserve_exact(count);
			
			for _ in 0..count {
				let (idx, img) = receive.recv().map_err(|err| err.to_string())?;
				if idx == frames.len() {
					frames.push(img.asImage());
					while let Some(img) = acum.remove(&frames.len()) {
						frames.push(img.asImage());
					}
					continue;
				}
				acum.insert(idx, img);
			}
			
			if !acum.is_empty() {
				return Err(format!("Missing frame {}", frames.len()));
			}
			
			Ok(ImageSequence {
				frames,
				timePerFrame: Duration::from_secs_f32(1.0 / framerate),
			})
		})
	}
	
	pub fn timePerFrame(&self) -> Duration { self.timePerFrame }
	
	pub fn getFrame(&self, timeSinceStart: Duration) -> &Image {
		let index = timeSinceStart.as_micros() / self.timePerFrame.as_micros();
		let wrapped = (index % self.frames.len() as u128) as usize;
		
		self.frames.get(wrapped).unwrap()
	}
}

pub struct RgbImg {
	pixels: Vec<Rgba8Pixel>,
	width: u32,
	height: u32,
	isDefault: bool,
}

impl Display for RgbImg {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		let mut rgb=&self.pixels[(self.width * self.height / 2) as usize];
		for p in &self.pixels {
			if p.r != 255 {
				rgb = p;
				break;
			}
		}
		
		f.write_str(&format!("RgbImg{{width= {}, height={} {}}}", self.width, self.height, rgb))?;
		Ok(())
	}
}

impl RgbImg {
	pub fn isDefault(&self) -> bool { self.isDefault }
	
	pub fn asImageCached(&self,globalIcon: &RwLock<GlobalIcons>, defaultCache: &mut Option<Image>) -> Image {
		if self.isDefault() {
			match defaultCache.clone() {
				Some(i) => { i }
				None => {
					let i = globalIcon.read().unwrap().default.asImage();
					*defaultCache = Some(i.clone());
					i
				}
			}
		}else {
			self.asImage()
		}
	}
	
	pub fn asImage(&self) -> Image {
		let mut buffer = SharedPixelBuffer::new(self.width, self.height);
		let buff = buffer.make_mut_slice();
		let px = &self.pixels;
		buff[..px.len()].copy_from_slice(px);
		Image::from_rgba8(buffer)
	}
	
	pub fn read(path: &str) -> Result<RgbImg, String> {
		RgbImg::readSizeLimited(path, Some(256))
	}
	pub fn readSizeLimited(path: &str, maxSize: Option<u32>) -> Result<RgbImg, String> {
		let mut img = None;
		
		if let Some(p) = path.strip_prefix(">>") {
			// println!("Loading embedded: {:?}", p);
			if let Some(asset) = BuiltInAssets::get(p) {
				// println!("{p} {}", asset.data.len());
				img = Some(imgFrom(Cursor::new(asset.data.deref()))?);
			}
		}
		
		let mut img = match img {
			None => {
				ImageReader::open(path)
					.and_then(|i| i.with_guessed_format())
					.map_err(|err| format!("{}", err))
					.and_then(|i| i.decode().map_err(|err| format!("{}", err)))?
			}
			Some(i) => { i }
		};
		
		if let Some(maxSize) = maxSize {
			let s = max(img.width(), img.height());
			if s > maxSize {
				let fac = maxSize as f32 / s as f32;
				img = img.resize(
					(img.width() as f32 * fac) as u32,
					(img.height() as f32 * fac) as u32,
					FilterType::Triangle,
				);
			}
		}
		
		Ok(convert(img, path))
	}
}

fn convert(img: DynamicImage, path: &str) -> RgbImg {
	let w = img.width();
	let h = img.height();
	let mut s = vec![Rgba8Pixel::new(0, 0, 0, 0); (w * h) as usize];
	
	match img.color() {
		ColorType::L8 => {
			let d = img.as_luma8().unwrap();
			for y in 0..h {
				for x in 0..w {
					s[(x + y * w) as usize] = make(d.get_pixel(x, y).to_rgba());
				}
			}
		}
		ColorType::La8 => {
			let d = img.as_luma_alpha8().unwrap();
			for y in 0..h {
				for x in 0..w {
					s[(x + y * w) as usize] = make(d.get_pixel(x, y).to_rgba());
				}
			}
		}
		ColorType::Rgb8 => {
			let d = img.as_rgb8().unwrap();
			for y in 0..h {
				for x in 0..w {
					s[(x + y * w) as usize] = make(d.get_pixel(x, y).to_rgba());
				}
			}
		}
		ColorType::Rgba8 => {
			let d = img.as_rgba8().unwrap();
			for y in 0..h {
				for x in 0..w {
					s[(x + y * w) as usize] = make(d.get_pixel(x, y).to_rgba());
				}
			}
		}
		ColorType::L16 => {
			let d = img.as_luma16().unwrap();
			for y in 0..h {
				for x in 0..w {
					s[(x + y * w) as usize] = make(p16to8(d.get_pixel(x, y).to_rgba()));
				}
			}
		}
		ColorType::La16 => {
			let d = img.as_luma_alpha16().unwrap();
			for y in 0..h {
				for x in 0..w {
					s[(x + y * w) as usize] = make(p16to8(d.get_pixel(x, y).to_rgba()));
				}
			}
		}
		ColorType::Rgb16 => {
			let d = img.as_rgb16().unwrap();
			for y in 0..h {
				for x in 0..w {
					s[(x + y * w) as usize] = make(p16to8(d.get_pixel(x, y).to_rgba()));
				}
			}
		}
		ColorType::Rgba16 => {
			let d = img.as_rgba16().unwrap();
			for y in 0..h {
				for x in 0..w {
					s[(x + y * w) as usize] = make(p16to8(d.get_pixel(x, y).to_rgba()));
				}
			}
		}
		ColorType::Rgb32F => {
			let d = img.as_rgb32f().unwrap();
			for y in 0..h {
				for x in 0..w {
					s[(x + y * w) as usize] = make(pf32to8(d.get_pixel(x, y).to_rgba()));
				}
			}
		}
		ColorType::Rgba32F => {
			let d = img.as_rgba32f().unwrap();
			for y in 0..h {
				for x in 0..w {
					s[(x + y * w) as usize] = make(pf32to8(d.get_pixel(x, y).to_rgba()));
				}
			}
		}
		_ => todo!("Unimplemented format {:?}", img.color())
	};
	
	RgbImg {
		pixels: s,
		width: w,
		height: h,
		isDefault: path.eq(">>default.png"),
	}
}

fn ceilDiv(a: usize, b: usize) -> usize {
	(a + (b - 1)) / b
}

fn imgFrom<R: BufRead + Seek>(data: R) -> Result<DynamicImage, String> {
	ImageReader::new(data)
		.with_guessed_format()
		.map_err(|err| format!("{}", err))
		.and_then(|i| i.decode().map_err(|err| format!("{}", err)))
}

fn make(p: Rgba<u8>) -> Rgba8Pixel {
	Rgba8Pixel::from([p[0], p[1], p[2], p[3]])
}


fn p16to8(p: Rgba<u16>) -> Rgba<u8> {
	Rgba::from([p[0] as u8, p[1] as u8, p[2] as u8, p[3] as u8])
}

fn pf32to8(p: Rgba<f32>) -> Rgba<u8> {
	Rgba::from([p[0] as u8, p[1] as u8, p[2] as u8, p[3] as u8])
}

