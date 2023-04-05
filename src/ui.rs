use std::ffi::c_void;
use std::fmt::format;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use nwg::{Button, ControlHandle, FlexboxLayout, FlexboxLayoutItem, Frame, GridLayout, NwgError, TextInput, TextInputBuilder, Window, WindowFlags};
use nwg::stretch::geometry::{Rect, Size};
use nwg::stretch::style::{AlignItems, AlignSelf, Dimension as D, Dimension, Direction, FlexDirection, JustifyContent, Overflow};
use nwg::stretch::style::FlexDirection::{Column, Row};
use nwg::stretch::style::FlexWrap::Wrap;

use crate::cursed;
use crate::cursed::PlacementMode;

pub struct MWindow {
	pub window: Window,
	TextInput: Vec<TextInput>,
	Button: Vec<Button>,
	Frame: Vec<Frame>,
}

impl MWindow {
	pub fn getWindowPlacementMode(&self) -> Result<PlacementMode, String> { cursed::getWindowPlacementMode(&self.window) }
	pub fn position(&self) -> (i32, i32) {
		return self.window.position();
	}
	pub fn size(&self) -> (u32, u32) {
		return self.window.size();
	}
	pub fn setSize(&self, width: u32, height: u32) {
		self.window.set_size(width, height);
	}
	pub fn setPosition(&self, x: i32, y: i32) {
		self.window.set_position(x, y);
	}
	pub fn setVisible(&self, visible: bool) {
		self.window.set_visible(visible);
	}
}

pub fn makeMain<'a>() -> Result<MWindow, NwgError> {
	let mut flags = WindowFlags::empty();
	for x in vec![WindowFlags::WINDOW, WindowFlags::MINIMIZE_BOX, WindowFlags::MAXIMIZE_BOX, WindowFlags::RESIZABLE] {
		flags = flags.union(x);
	}
	let mut ui = MWindow {
		window: Default::default(),
		TextInput: vec![],
		Button: vec![],
		Frame: vec![],
	};
	
	Window::builder()
		.flags(flags)
		.title("Rexplorer")
		.build(&mut ui.window)?;
	
	let mut name_edit = Default::default();
	
	TextInput::builder()
		.text("Heisenberg")
		.focus(true)
		.parent(&mut ui.window)
		.build(&mut name_edit)?;
	
	let mut bf = Default::default();
	Frame::builder()
		.parent(&mut ui.window)
		.build(&mut bf)?;
	
	let mut b = FlexboxLayout::builder()
		.parent(&mut bf)
		.flex_wrap(Wrap)
		.flex_direction(Row)
		.overflow(Overflow::Visible)
		.justify_content(JustifyContent::FlexStart)
		.align_items(AlignItems::FlexStart);
	
	for i in 0..15 {
		let mut btn = Default::default();
		
		Button::builder()
			.text(format!("Penis {}", i + 1).as_ref())
			.parent(&mut bf)
			.build(&mut btn)?;
		
		b = b.child(&btn).child_size(size(Dimension::Points(100.0))).child_margin(rect(D::Points(0.0)));
		ui.Button.push(btn);
	}
	let fuckYou = Default::default();
	b.build(&fuckYou)?;
	
	fixedFlexy(&mut ui, Dimension::Points(60.0), Column, &name_edit, &bf)?;
	let a = fuckYou;
	
	ui.TextInput.push(name_edit);
	ui.Frame.push(bf);
	return Ok(ui);
}

fn fixedFlexy<A: Into<ControlHandle>, B: Into<ControlHandle>>(ui: &mut MWindow, fixed: Dimension, dir: FlexDirection, name_edit: A, hello_button: B) -> Result<FlexboxLayout, NwgError> {
	let layout = Default::default();
	let r = rect(Dimension::Points(0.0));
	FlexboxLayout::builder()
		.parent(&mut ui.window)
		.flex_direction(dir)
		.child(name_edit)
		.child_margin(r)
		.child_flex_basis(fixed)
		.child(hello_button)
		.child_margin(r)
		.child_flex_grow(1.0)
		.child_flex_shrink(1.0)
		.build(&layout)?;
	
	Ok(layout)
}

fn size(d: Dimension) -> Size<Dimension> {
	Size {
		width: d,
		height: d,
	}
}

fn rect(d: Dimension) -> Rect<Dimension> {
	Rect {
		start: d,
		end: d,
		top: d,
		bottom: d,
	}
}

pub fn centerWindow(window: &MWindow) {
	let (w, h) = window.window.size();
	
	let [top, left, width, height] = nwg::Monitor::monitor_rect_from_window(&window.window);
	
	window.window.set_position(top + ((width - w as i32) / 2), left + ((height - h as i32) / 2));
}
