use lazy_static::lazy_static;
use std::collections::HashMap;

lazy_static! {
    static ref HASHMAP: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert(
            "meta-llama/Meta-Llama-3-8B-Instruct",
            "meta-llama/Llama-3.1-8B-Instruct",
        );
        m
    };
}

pub fn map_model(requested: &str) -> Option<&'static str> {
    HASHMAP.get(requested).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_model() {
        let model = "meta-llama/Meta-Llama-3-8B-Instruct";
        let mapped = map_model(model);
        assert_eq!(mapped, Some("meta-llama/Llama-3.1-8B-Instruct"));
    }
}
