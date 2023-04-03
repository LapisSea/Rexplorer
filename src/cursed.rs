use nwg::Window;

#[derive(Debug)]
pub(crate) enum PlacementMode {
	REGULAR,
	MINIMIZED,
	MAXIMIZED,
}
pub(crate) fn getWindowPlacementMode(window: &Window) -> Result<PlacementMode, String> {
	let handle = window.handle.hwnd();
	if handle.is_none() {
		return Err("Window is destroyed".to_string());
	}
	use winapi::um::winuser;
	let cmd;
	unsafe {
		use std::mem;
		let mut placement = mem::zeroed();
		winuser::GetWindowPlacement(handle.unwrap(), &mut placement);
		cmd = placement.showCmd as i32;
	}
	match cmd {
		winuser::SW_SHOWNORMAL => Ok(PlacementMode::REGULAR),
		winuser::SW_SHOWMAXIMIZED => Ok(PlacementMode::MAXIMIZED),
		winuser::SW_SHOWMINIMIZED => Ok(PlacementMode::MINIMIZED),
		_ => Err(format!("Illegal value: {cmd}"))
	}
}
