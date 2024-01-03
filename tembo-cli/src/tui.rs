use tiny_gradient::{GradientDisplay, GradientStr, RGB};
use colorful::{RGB as ColorfulRgb, Colorful};

/// Clears the console when called
pub fn clean_console() {
	print!("{esc}c", esc = 27 as char);
}

/// Prints a colored log to the console (defaults to `use tui::colors::salmon`)
pub fn print_color(log: &str, color: Option<ColorfulRgb>) {
	let color = color.unwrap_or(colors::salmon());
	println!("{}", log.color(color));
}

pub fn print_gradient(log: &str) {
	let gradient = GradientStr::gradient(
		log,
		[RGB::new(255, 198, 217), RGB::new(124, 207, 225), RGB::new(137, 203, 166), RGB::new(165, 213, 113)],
	);
	println!("{}", gradient);
}

/// Tembo branded gradient chevrons for printing singular output
pub fn chevrons<'a>() -> GradientDisplay<'a, [RGB; 4]> {
	GradientStr::gradient(
		&">>>>",
		[RGB::new(255, 198, 217), RGB::new(124, 207, 225), RGB::new(137, 203, 166), RGB::new(165, 213, 113)],
	)
}

/// Helper function for printing indentations to the console
pub fn indent(amount: u32) -> String {
	let mut new_amount = String::new();

	for _ in 0..amount {
		new_amount.push('\n');
	}
	new_amount
}

pub mod colors {
	use colorful::RGB as ColorfulRgb;
	use tiny_gradient::{GradientDisplay, GradientStr, RGB};

	pub fn salmon() -> ColorfulRgb {
		ColorfulRgb::new(255, 125, 127)
	}

	pub fn sand() -> ColorfulRgb {
		ColorfulRgb::new(255, 244, 228)
	}

	pub fn gradient<'a>(log: &'a str) -> GradientDisplay<'a, [RGB; 4]> {
		GradientStr::gradient(
			log,
			[RGB::new(255, 198, 217), RGB::new(124, 207, 225), RGB::new(137, 203, 166), RGB::new(165, 213, 113)],
		)
	}
}
