use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct StackError {
    pub details: String,
}

impl StackError {
    pub fn new(msg: &str) -> StackError {
        StackError {
            details: msg.to_string(),
        }
    }
}

impl fmt::Display for StackError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

impl Error for StackError {
    fn description(&self) -> &str {
        &self.details
    }
}
