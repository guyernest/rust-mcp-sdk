//! Shared utility functions.

/// Convert a `snake_case` or `kebab-case` string to `PascalCase`.
pub(crate) fn to_pascal_case(s: &str) -> String {
    s.split(['_', '-'])
        .filter(|seg| !seg.is_empty())
        .map(|seg| {
            let mut chars = seg.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    upper + chars.as_str()
                },
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case() {
        assert_eq!(to_pascal_case("hello_world"), "HelloWorld");
        assert_eq!(to_pascal_case("get_user_data"), "GetUserData");
    }

    #[test]
    fn kebab_case() {
        assert_eq!(to_pascal_case("my-tool"), "MyTool");
        assert_eq!(to_pascal_case("add-user-data"), "AddUserData");
    }

    #[test]
    fn single_word() {
        assert_eq!(to_pascal_case("search"), "Search");
    }

    #[test]
    fn mixed_separators() {
        assert_eq!(to_pascal_case("my-tool_name"), "MyToolName");
    }
}
