use google_cloud_storage::http::buckets::{Binding, Condition};
use std::collections::HashSet;

#[derive(Default)]
pub struct IamBindingBuilder {
    role: Option<String>,
    members: HashSet<String>,
    condition: Option<Condition>,
}

impl IamBindingBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn role(mut self, role: impl AsRef<str>) -> Self {
        self.role = Some(role.as_ref().to_string());
        self
    }

    pub fn add_member(mut self, member: impl AsRef<str>) -> Self {
        self.members.insert(member.as_ref().to_string());
        self
    }

    pub fn add_members<I, S>(mut self, members: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.members
            .extend(members.into_iter().map(|s| s.as_ref().to_string()));
        self
    }

    pub fn condition(mut self, condition: Condition) -> Self {
        self.condition = Some(condition);
        self
    }

    pub fn build(self) -> Result<Binding, &'static str> {
        let role = self.role.ok_or("Role must be specified")?;
        if self.members.is_empty() {
            return Err("At least one member must be specified");
        }
        Ok(Binding {
            role,
            members: self.members.into_iter().collect(),
            condition: self.condition,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_successful_build() {
        let binding = IamBindingBuilder::new()
            .role("admin")
            .add_member("user1@example.com")
            .add_member("user2@example.com")
            .build();
        assert!(binding.is_ok());
        let binding = binding.unwrap();
        assert_eq!(binding.role, "admin");
        assert_eq!(binding.members.len(), 2);
        assert!(binding.members.contains(&"user1@example.com".to_string()));
        assert!(binding.members.contains(&"user2@example.com".to_string()));
    }

    #[test]
    fn test_build_without_role() {
        let binding = IamBindingBuilder::new()
            .add_member("user1@example.com")
            .build();
        assert!(binding.is_err());
        assert_eq!(binding.unwrap_err(), "Role must be specified");
    }

    #[test]
    fn test_build_without_members() {
        let binding = IamBindingBuilder::new().role("admin").build();
        assert!(binding.is_err());
        assert_eq!(
            binding.unwrap_err(),
            "At least one member must be specified"
        );
    }

    // This test is testing that adding a duplicate member doesn't result in a duplicate member in the binding.
    #[test]
    fn test_add_duplicate_members() {
        let binding = IamBindingBuilder::new()
            .role("admin")
            .add_member("user1@example.com")
            .add_member("user1@example.com")
            .build();
        assert!(binding.is_ok());
        let binding = binding.unwrap();
        assert_eq!(binding.members.len(), 1);
    }

    // Test adding multiple members at once.  While not something we support yet, it's a good idea
    // to support it in the future.
    #[test]
    fn test_add_multiple_members() {
        let binding = IamBindingBuilder::new()
            .role("admin")
            .add_members(vec!["user1@example.com", "user2@example.com"])
            .build();
        assert!(binding.is_ok());
        let binding = binding.unwrap();
        assert_eq!(binding.members.len(), 2);
        assert!(binding.members.contains(&"user1@example.com".to_string()));
        assert!(binding.members.contains(&"user2@example.com".to_string()));
    }

    #[test]
    fn test_add_condition() {
        let condition = Condition {
            title: "test".to_string(),
            description: Some("test condition".to_string()),
            expression: "resource.type == \"storage.googleapis.com/Bucket\") || (resource.type == \"storage.googleapis.com/Object\"".to_string(),
        };
        let binding = IamBindingBuilder::new()
            .role("admin")
            .add_member("user1@example.com")
            .condition(condition.clone())
            .build();
        assert!(binding.is_ok());
        let binding = binding.unwrap();
        assert_eq!(binding.condition, Some(condition));
    }

    #[test]
    fn test_build_with_all_options() {
        let condition = Condition {
            title: "test".to_string(),
            description: Some("test condition".to_string()),
            expression: "resource.type == \"storage.googleapis.com/Bucket\") || (resource.type == \"storage.googleapis.com/Object\"".to_string(),
        };
        let binding = IamBindingBuilder::new()
            .role("admin")
            .add_member("user1@example.com")
            .add_members(vec!["user2@example.com", "user3@example.com"])
            .condition(condition.clone())
            .build();
        assert!(binding.is_ok());
        let binding = binding.unwrap();
        assert_eq!(binding.role, "admin");
        assert_eq!(binding.members.len(), 3);
        assert!(binding.members.contains(&"user1@example.com".to_string()));
        assert!(binding.members.contains(&"user2@example.com".to_string()));
        assert!(binding.members.contains(&"user3@example.com".to_string()));
        assert_eq!(binding.condition, Some(condition));
    }

    #[test]
    fn test_builder_method_order() {
        let binding1 = IamBindingBuilder::new()
            .role("admin")
            .add_member("user1@example.com")
            .build();
        let binding2 = IamBindingBuilder::new()
            .add_member("user1@example.com")
            .role("admin")
            .build();
        assert!(binding1.is_ok());
        assert!(binding2.is_ok());
        assert_eq!(binding1.unwrap(), binding2.unwrap());
    }
}
