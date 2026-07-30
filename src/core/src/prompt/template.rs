use std::collections::HashMap;

/// Renders a template string by replacing `{{KEY}}` placeholders with values.
/// Unknown placeholders are left as-is.
pub fn render_template(template: &str, vars: &HashMap<String, String>) -> String {
    let mut result = template.to_string();

    for (key, value) in vars {
        let placeholder = format!("{{{{{key}}}}}");
        result = result.replace(&placeholder, value);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_replacement() {
        let mut vars = HashMap::new();
        vars.insert("NAME".to_string(), "Bimo".to_string());
        vars.insert("VERSION".to_string(), "1.0".to_string());

        let template = "Hello {{NAME}}, version {{VERSION}}";
        assert_eq!(render_template(template, &vars), "Hello Bimo, version 1.0");
    }

    #[test]
    fn test_unknown_placeholder_left_as_is() {
        let vars = HashMap::new();
        let template = "Hello {{NAME}}";
        assert_eq!(render_template(template, &vars), "Hello {{NAME}}");
    }

    #[test]
    fn test_multiple_occurrences() {
        let mut vars = HashMap::new();
        vars.insert("X".to_string(), "foo".to_string());
        let template = "{{X}} {{X}} {{X}}";
        assert_eq!(render_template(template, &vars), "foo foo foo");
    }
}
