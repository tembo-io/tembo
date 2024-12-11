use colorful::{Color, Colorful, RGB as ColorfulRgb};
use std::fmt;
use tiny_gradient::{GradientDisplay, GradientStr, RGB};

pub enum TemboCliLog<'a> {
    Gradient(GradientDisplay<'a, [RGB; 3]>),
    GradientLarge(GradientDisplay<'a, [RGB; 4]>),
    Default(String),
}

impl fmt::Display for TemboCliLog<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TemboCliLog::Gradient(gradient) => write!(f, "{}", gradient),
            TemboCliLog::Default(message) => write!(f, "{}", message),
            TemboCliLog::GradientLarge(gradient) => write!(f, "{}", gradient),
        }
    }
}

/// Utility for completely clearing the console when called
pub fn clean_console() {
    print!("{esc}c", esc = 27 as char);
}

#[allow(dead_code)]
/// Prints a colored log to the console (defaults to `use tui::colors::salmon`)
pub fn print_color(log: &str, color: Option<ColorfulRgb>) {
    let color = color.unwrap_or(colors::sql_u());
    println!("{}", log.color(color));
}
#[allow(dead_code)]
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

#[allow(dead_code)]
pub fn label(log: &str) {
    println!("{} {}", "➜".bold(), colors::gradient_rainbow(log));
}

pub fn label_with_value(log: &str, value: &str) {
    println!(
        "{} {} {}",
        "➜".bold(),
        colors::gradient_rainbow(log),
        value.color(Color::White).bold()
    );
}

pub fn error(log: &str) {
    println!(
        "{} {}",
        "✗".color(colors::bad()).bold(),
        log.color(colors::bad())
    );
}

#[allow(dead_code)]
pub fn warning(log: &str) {
    println!(
        "{} {}",
        "⚠".color(colors::schema_y()).bold(),
        log.color(colors::schema_y())
    );
}

pub fn info(log: &str) {
    println!(
        "{} {}",
        "i".color(colors::schema_y()).bold(),
        log.color(colors::schema_y())
    );
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

#[allow(dead_code)]
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

pub fn logo<'a>() -> TemboCliLog<'a> {
    colors::gradient_rainbow(">>> T E M B O")
}

pub fn instance_started(server_url: &str, stack: &str) {
    let bar = "┃".color(colors::sql_u()).bold();
    println!(
        "\n{bar} {} instance {}: \n\n ➜ {}\n ➜ {}",
        logo(),
        "started".bg_rgb(255, 125, 127).color(Color::White).bold(),
        format_args!(
            "{} {}",
            "Connection String:".color(Color::White).bold(),
            server_url.bold()
        ),
        stack.color(colors::grey()).bold()
    );
    println!()
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
    use super::TemboCliLog;
    use colorful::RGB as ColorfulRgb;
    use spinoff::Color as SpinnerColor;
    use tiny_gradient::{GradientStr, RGB};

    pub fn sql_u() -> ColorfulRgb {
        ColorfulRgb::new(255, 125, 127)
    }

    #[allow(dead_code)]
    pub fn warning_light() -> ColorfulRgb {
        ColorfulRgb::new(255, 244, 228)
    }

    pub fn schema_y() -> ColorfulRgb {
        ColorfulRgb::new(233, 252, 135)
    }

    #[allow(dead_code)]
    #[allow(clippy::needless_lifetimes)]
    pub fn gradient_p<'a>(log: &'a str) -> TemboCliLog {
        let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();
        // aTerminal only supports 8 bit colors so gradients won't work
        if term_program == "Apple_Terminal" {
            return TemboCliLog::Default(log.to_string());
        }
        TemboCliLog::GradientLarge(GradientStr::gradient(
            log,
            [
                RGB::new(255, 198, 217),
                RGB::new(124, 207, 225),
                RGB::new(137, 203, 166),
                RGB::new(165, 213, 113),
            ],
        ))
    }

    pub fn gradient_rainbow(log: &str) -> TemboCliLog {
        let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();
        // aTerminal only supports 8 bit colors so gradients won't work
        if term_program == "Apple_Terminal" {
            return TemboCliLog::Default(log.to_string());
        }
        TemboCliLog::Gradient(GradientStr::gradient(
            log,
            [
                RGB::new(247, 117, 119),
                RGB::new(219, 57, 203),
                RGB::new(202, 111, 229),
            ],
        ))
    }

    pub fn indicator_good() -> ColorfulRgb {
        ColorfulRgb::new(132, 234, 189)
    }

    pub fn grey() -> ColorfulRgb {
        ColorfulRgb::new(158, 162, 166)
    }

    pub fn bad() -> ColorfulRgb {
        ColorfulRgb::new(250, 70, 102)
    }

    pub const SPINNER_COLOR: SpinnerColor = SpinnerColor::TrueColor {
        r: 255,
        g: 125,
        b: 127,
    };
}
