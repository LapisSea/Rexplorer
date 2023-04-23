use std::{env, fs, io};
use std::path::PathBuf;

use image::imageops::FilterType;
use image::io::Reader;
use image::Pixel;
use slint_build::*;
#[cfg(windows)]
use winres::WindowsResource;

fn main() -> Result<(), String> {
	let path = "src/ui/start.slint";
	
	let conf = CompilerConfiguration::new()
		// .with_style("native".into())
		// .with_style("fluent".into())
		;
	
	// println!("AAAAAAAAAAAAAAAAAAAAAAAAAAAA, {:?}", std::env::var_os("SLINT_STYLE"));
	// panic!();
	
	let ico_src = "src/ui/icon.png";
	
	let mut icon_path = PathBuf::from(env::var("OUT_DIR").unwrap());
	icon_path.push("exec_icon.ico");
	let icon_path = icon_path.to_str().unwrap();
	
	
	compile_with_config(path, conf).map_err(|err| err.to_string())?;
	
	
	if cfg!(target_os = "windows") {
		png_to_icon(ico_src, icon_path);
		
		WindowsResource::new()
			// This path can be absolute, or relative to your crate root.
			.set_icon(icon_path)
			.compile().map_err(|err| err.to_string())?;
	}
	
	Ok(())
}

fn png_to_icon(ico_src: &str, icon_path: &str) {
	if fs::metadata(icon_path).map(|meta| {
		fs::metadata(ico_src).unwrap().modified().unwrap() > meta.modified().unwrap()
	}).unwrap_or(true) {
		let mut icon = Reader::open(ico_src)
			.and_then(|i| i.with_guessed_format())
			.map_err(|err| format!("{}", err))
			.and_then(|i| i.decode().map_err(|err| format!("{}", err)))
			.unwrap();
		
		let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);
		let file = fs::File::open(ico_src).unwrap();
		let image = ico::IconImage::read_png(file).unwrap();
		icon_dir.add_entry(ico::IconDirEntry::encode(&image).unwrap());
		
		while icon.width() > 4 {
			let mut rgba = vec![255; (4 * icon.width() * icon.height()) as usize];
			let img = icon.as_rgba8().unwrap();
			
			for y in 0..img.height() {
				for x in 0..img.width() {
					let px = img.get_pixel(x, y).to_rgba();
					let start = ((x + (y * img.width())) * 4) as usize;
					rgba[start..(4 + start)].copy_from_slice(&px.0[..4]);
				}
			}
			
			let image = ico::IconImage::from_rgba_data(img.width(), img.height(), rgba);
			icon_dir.add_entry(ico::IconDirEntry::encode(&image).unwrap());
			
			icon = icon.resize(icon.width() / 2, icon.height() / 2, FilterType::Gaussian);
		}
		
		let file = fs::File::create(icon_path).unwrap();
		icon_dir.write(file).unwrap();
	}
}
