use slint_build::*;

fn main() {
	// println!("cargo:rustc-env=SLINT_BACKEND=Qt");
	// println!("cargo:rustc-env=SLINT_STYLE=native");
	// println!("cargo:rustc-env=qmake=C:\\Programming\\Qt\\6.5.0\\mingw_64\\bin\\");
	
	let path = "src/ui/start.slint";
	
	let conf = CompilerConfiguration::new()
		// .with_style("native".into())
		// .with_style("fluent".into())
		;
	
	// println!("AAAAAAAAAAAAAAAAAAAAAAAAAAAA, {:?}", std::env::var_os("SLINT_STYLE"));
	// panic!();
	
	compile_with_config(path, conf).unwrap();
}
