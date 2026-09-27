//! Contact snapshots belong to the recommendation and inherit its audience.
//! Only the attached number and its saved label cross this boundary.
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Debug, Default, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    #[default]
    Keep,
    Set {
        phone: String,
        #[serde(default)]
        saved_name: Option<String>,
    },
    None,
}

pub fn normalize(phone: &str) -> Result<String, &'static str> {
    if phone.len() > 80
        || phone
            .chars()
            .any(|c| !c.is_ascii_digit() && !"+ ()-.".contains(c))
    {
        return Err("Use an international phone number, including + and country code");
    }
    let compact: String = phone
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '+')
        .collect();
    let digits = compact.strip_prefix('+').unwrap_or("");
    if !(8..=15).contains(&digits.len())
        || digits.starts_with('0')
        || !digits.bytes().all(|b| b.is_ascii_digit())
    {
        return Err("Use an international phone number, including + and country code");
    }
    Ok(compact)
}

pub fn snapshot(
    phone: &str,
    saved_name: Option<&str>,
    origin: &str,
) -> Result<Value, &'static str> {
    let mut value = json!({"phone":normalize(phone)?,"origin":origin});
    if let Some(name) = saved_name {
        let name = name.trim();
        if name.chars().count() > 200 || name.chars().any(char::is_control) {
            return Err("Contact name must be at most 200 characters without control characters");
        }
        // An explicitly cleared label is a choice, not missing metadata for
        // automatic backfill to restore. Older clients omit the field entirely.
        value["saved_name"] = json!(name);
    }
    Ok(value)
}

impl Edit {
    pub fn check(&self) -> Result<(), &'static str> {
        if let Self::Set { phone, saved_name } = self {
            snapshot(phone, saved_name.as_deref(), "user")?;
        }
        Ok(())
    }
    pub fn apply(
        &self,
        old_subject: &str,
        new_subject: &str,
        previous: &Value,
        next: &mut Value,
    ) -> Result<(), &'static str> {
        match self {
            Self::Keep => {
                // A renamed/retyped subject needs a deliberate new attachment.
                if old_subject == new_subject && previous["entity_kind"] == next["entity_kind"] {
                    for field in ["contact", "contact_matching"] {
                        if let Some(value) = previous.get(field) {
                            next[field] = value.clone();
                        }
                    }
                } else if previous.get("contact").is_some()
                    || previous.get("contact_matching").is_some()
                {
                    next["contact_matching"] = json!("off");
                }
            }
            Self::Set { phone, saved_name } => {
                if next["entity_kind"] != "person_service" {
                    return Err("Contact numbers are supported for people and services");
                }
                next["contact"] = snapshot(phone, saved_name.as_deref(), "user")?;
                next["contact_matching"] = json!("off");
            }
            Self::None => {
                if let Some(object) = next.as_object_mut() {
                    object.remove("contact");
                }
                next["contact_matching"] = json!("off");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_name_is_bounded_and_optional() {
        assert_eq!(
            snapshot("+12025550123", Some(" Maya Rao "), "contacts").unwrap()["saved_name"],
            "Maya Rao"
        );
        assert!(snapshot("+12025550123", Some("Maya\nRao"), "contacts").is_err());
        assert_eq!(
            snapshot("+12025550123", Some(""), "user").unwrap()["saved_name"],
            ""
        );
        assert!(snapshot("+12025550123", Some(&"a".repeat(201)), "contacts").is_err());
        assert!(
            snapshot("+12025550123", None, "user")
                .unwrap()
                .get("saved_name")
                .is_none()
        );
    }
    #[test]
    fn international_numbers_only() {
        assert_eq!(normalize("+91 (98765) 43210").unwrap(), "+919876543210");
        for bad in [
            "9876543210",
            "+0123456789",
            "+919876543210 ext 2",
            "++919876543210",
            "+123",
            "+1234567890123456",
        ] {
            assert!(normalize(bad).is_err());
        }
    }
    #[test]
    fn edits_preserve_snapshots_but_not_across_identity_changes() {
        let previous = json!({"entity_kind":"person_service","contact":{"phone":"+919876543210","origin":"contacts"}});
        let mut next = json!({"entity_kind":"person_service"});
        Edit::Keep
            .apply("A B", "A B", &previous, &mut next)
            .unwrap();
        assert_eq!(next["contact"], previous["contact"]);
        let mut renamed = json!({"entity_kind":"person_service"});
        Edit::Keep
            .apply("A B", "C D", &previous, &mut renamed)
            .unwrap();
        assert!(renamed.get("contact").is_none());
        assert_eq!(renamed["contact_matching"], "off");
        Edit::None
            .apply("A B", "A B", &previous, &mut next)
            .unwrap();
        assert!(next.get("contact").is_none());
    }
}
