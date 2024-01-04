use colorful::{Color, Colorful, RGB as ColorfulRgb};
use tiny_gradient::{GradientDisplay, GradientStr, RGB};

/// Utility for completely clearing the console when called
pub fn clean_console() {
    print!("{esc}c", esc = 27 as char);
}

/// Prints a colored log to the console (defaults to `use tui::colors::salmon`)
pub fn print_color(log: &str, color: Option<ColorfulRgb>) {
    let color = color.unwrap_or(colors::sql_u());
    println!("{}", log.color(color));
}

pub fn print_gradient(log: &str) {
    let gradient = GradientStr::gradient(
        log,
        [
            RGB::new(255, 198, 217),
            RGB::new(124, 207, 225),
            RGB::new(137, 203, 166),
            RGB::new(165, 213, 113),
        ],
    );
    println!("{}", gradient);
}

pub fn label(log: &str) {
    println!("{} {}", "➜".bold(), colors::gradient_rainbow(log));
}

pub fn confirmation(log: &str) {
    println!(
        "{} {}",
        "✓".color(colors::indicator_good()).bold(),
        colors::gradient_rainbow(log)
    );
}

pub fn white_confirmation(log: &str) {
    println!(
        "{} {}",
        "✓".color(colors::indicator_good()).bold(),
        log.color(Color::White).bold()
    );
}

/// Tembo branded gradient chevrons for printing singular output
pub fn chevrons<'a>() -> GradientDisplay<'a, [RGB; 4]> {
    GradientStr::gradient(
        &">>>>",
        [
            RGB::new(255, 198, 217),
            RGB::new(124, 207, 225),
            RGB::new(137, 203, 166),
            RGB::new(165, 213, 113),
        ],
    )
}

pub fn logo<'a>() -> GradientDisplay<'a, [RGB; 3]> {
    colors::gradient_rainbow(">>> T E M B O")
}

pub fn instance_started(server_url: &str, stack: &str, instance_type: &str) {
    let bar = "┃".color(colors::sql_u()).bold();
    println!(
        "\n{bar} {} {instance_type} instance {}: \n\n ➜ {}\n ➜ {}",
        logo(),
        "started".bg_rgb(255, 125, 127).color(Color::White).bold(),
        format!(
            "{} {}",
            "Url:".color(Color::White).bold(),
            server_url.bold()
        ),
        stack.color(colors::grey()).bold()
    );
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
	use spinoff::Color as SpinnerColor;

    pub fn sql_u() -> ColorfulRgb {
        ColorfulRgb::new(255, 125, 127)
    }

    pub fn warning_light() -> ColorfulRgb {
        ColorfulRgb::new(255, 244, 228)
    }

    pub fn gradient_p<'a>(log: &'a str) -> GradientDisplay<'a, [RGB; 4]> {
        GradientStr::gradient(
            log,
            [
                RGB::new(255, 198, 217),
                RGB::new(124, 207, 225),
                RGB::new(137, 203, 166),
                RGB::new(165, 213, 113),
            ],
        )
    }

    pub fn gradient_rainbow<'a>(log: &'a str) -> GradientDisplay<'a, [RGB; 3]> {
        GradientStr::gradient(
            log,
            [
                RGB::new(247, 117, 119),
                RGB::new(219, 57, 203),
                RGB::new(202, 111, 229),
            ],
        )
    }

    pub fn indicator_good() -> ColorfulRgb {
        ColorfulRgb::new(132, 234, 189)
    }

    pub fn grey() -> ColorfulRgb {
        ColorfulRgb::new(158, 162, 166)
    }

	pub const SPINNER_COLOR: SpinnerColor = SpinnerColor::TrueColor {
		r: 255,
		g: 125,
		b: 127,
	};
}
