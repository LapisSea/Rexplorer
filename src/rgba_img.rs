use std::cmp::max;
use std::io::Cursor;
use std::ops::Deref;

use image::{ColorType, Pixel, Rgba};
use image::imageops::FilterType;
use rust_embed::RustEmbed;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

#[derive(RustEmbed)]
#[folder = "src/ui/"]
#[include = "*.png"]
#[include = "*.jpg"]
struct BuiltInAssets;


pub struct RgbImg {
	pixels: Vec<Rgba8Pixel>,
	width: u32,
	height: u32,
}

impl RgbImg {
	pub fn asImage(&self) -> Image {
		let mut buffer = SharedPixelBuffer::new(self.width, self.height);
		let buff = buffer.make_mut_slice();
		let px = &self.pixels;
		for i in 0..px.len() {
			buff[i] = px[i]
		}
		return Image::from_rgba8(buffer);
	}
	
	pub fn read(path: &str) -> Result<RgbImg, String> {
		use image::io::Reader as ImageReader;
		
		let mut img = None;
		
		if path.starts_with(">>") {
			let p = &path[2..];
			println!("Loading embeded: {:?}", p);
			if let Some(asset) = BuiltInAssets::get(p) {
				let reader = Cursor::new(asset.data.deref());
				img = Some(ImageReader::new(reader)
					.with_guessed_format()
					.map_err(|err| format!("{}", err))
					.and_then(|i| i.decode().map_err(|err| format!("{}", err)))?);
			}
		}
		
		if img.is_none() {
			println!("Loading: {:?}", path);
			
			img = Some(ImageReader::open(path)
				.and_then(|i| i.with_guessed_format())
				.map_err(|err| format!("{}", err))
				.and_then(|i| i.decode().map_err(|err| format!("{}", err)))?);
		}
		
		
		let mut img = img.unwrap();
		
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
		
		let w = img.width();
		let h = img.height();
		let mut s = vec![Rgba8Pixel::new(0, 0, 0, 0); (w * h) as usize];
		
		match img.color() {
			ColorType::L8 => {
				let d = img.as_luma8().unwrap();
				for x in 0..w {
					for y in 0..h {
						s[(x + y * w) as usize] = make(d.get_pixel(x, y).to_rgba());
					}
				}
			}
			ColorType::La8 => {
				let d = img.as_luma_alpha8().unwrap();
				for x in 0..w {
					for y in 0..h {
						s[(x + y * w) as usize] = make(d.get_pixel(x, y).to_rgba());
					}
				}
			}
			ColorType::Rgb8 => {
				let d = img.as_rgb8().unwrap();
				for x in 0..w {
					for y in 0..h {
						s[(x + y * w) as usize] = make(d.get_pixel(x, y).to_rgba());
					}
				}
			}
			ColorType::Rgba8 => {
				let d = img.as_rgba8().unwrap();
				for x in 0..w {
					for y in 0..h {
						s[(x + y * w) as usize] = make(d.get_pixel(x, y).to_rgba());
					}
				}
			}
			ColorType::L16 => {
				let d = img.as_luma16().unwrap();
				for x in 0..w {
					for y in 0..h {
						s[(x + y * w) as usize] = make(p16to8(d.get_pixel(x, y).to_rgba()));
					}
				}
			}
			ColorType::La16 => {
				let d = img.as_luma_alpha16().unwrap();
				for x in 0..w {
					for y in 0..h {
						s[(x + y * w) as usize] = make(p16to8(d.get_pixel(x, y).to_rgba()));
					}
				}
			}
			ColorType::Rgb16 => {
				let d = img.as_rgb16().unwrap();
				for x in 0..w {
					for y in 0..h {
						s[(x + y * w) as usize] = make(p16to8(d.get_pixel(x, y).to_rgba()));
					}
				}
			}
			ColorType::Rgba16 => {
				let d = img.as_rgba16().unwrap();
				for x in 0..w {
					for y in 0..h {
						s[(x + y * w) as usize] = make(p16to8(d.get_pixel(x, y).to_rgba()));
					}
				}
			}
			ColorType::Rgb32F => {
				let d = img.as_rgb32f().unwrap();
				for x in 0..w {
					for y in 0..h {
						let p = pf32to8(d.get_pixel(x, y).to_rgba());
						s[(x + y * w) as usize] = make(p);
					}
				}
			}
			ColorType::Rgba32F => {
				let d = img.as_rgba32f().unwrap();
				for x in 0..w {
					for y in 0..h {
						s[(x + y * w) as usize] = make(pf32to8(d.get_pixel(x, y).to_rgba()));
					}
				}
			}
			_ => todo!("Unimplemented format {:?}", img.color())
		};
		
		Ok(RgbImg {
			pixels: s,
			width: w,
			height: h,
		})
	}
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

